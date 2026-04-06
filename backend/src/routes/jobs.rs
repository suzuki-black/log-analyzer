use axum::{extract::{Path, State}, Json};
use std::sync::Arc;
use uuid::Uuid;
use tokio::sync::broadcast;
use crate::{AppState, error::AppError, models::{Job, CreateJobRequest, SseEvent, TableInfo}};

pub async fn create_job(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateJobRequest>,
) -> Result<Json<Job>, AppError> {
    // validate file_ids exist
    for fid in &req.file_ids {
        let row: Option<(String,)> = sqlx::query_as("SELECT id FROM _la_files WHERE id = ?")
            .bind(fid)
            .fetch_optional(&state.pool)
            .await?;
        if row.is_none() {
            return Err(AppError::NotFound(format!("file_id {} not found", fid)));
        }
    }

    let full_table = format!("{}{}", req.table_name, state.config.table_suffix);
    let job_id = Uuid::new_v4().to_string();
    let dup_mode_str = serde_json::to_string(&req.dup_mode).unwrap().trim_matches('"').to_string();

    sqlx::query(
        "INSERT INTO _la_jobs (id, table_name, dup_mode, status, total_files) VALUES (?, ?, ?, 'pending', ?)",
    )
    .bind(&job_id)
    .bind(&full_table)
    .bind(&dup_mode_str)
    .bind(req.file_ids.len() as i64)
    .execute(&state.pool)
    .await?;

    let job = Job::new(job_id.clone(), full_table, req.file_ids, req.dup_mode);

    let (tx, _) = broadcast::channel::<SseEvent>(256);
    {
        let mut senders = state.senders.write().await;
        senders.insert(job_id.clone(), tx);
    }
    {
        let mut jobs = state.jobs.write().await;
        jobs.insert(job_id.clone(), job.clone());
    }

    // spawn worker
    let state2 = Arc::clone(&state);
    let job2 = job.clone();
    tokio::spawn(async move {
        crate::ingest::worker::run(state2, job2).await;
    });

    Ok(Json(job))
}

pub async fn list_jobs(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<Job>>, AppError> {
    let jobs = state.jobs.read().await;
    let mut list: Vec<Job> = jobs.values().cloned().collect();
    list.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    Ok(Json(list))
}

pub async fn get_job(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<Job>, AppError> {
    let jobs = state.jobs.read().await;
    jobs.get(&id)
        .cloned()
        .map(Json)
        .ok_or_else(|| AppError::NotFound(format!("job {} not found", id)))
}

pub async fn list_tables(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<TableInfo>>, AppError> {
    let suffix = &state.config.table_suffix;
    let pattern = format!("%{}", suffix);
    let rows: Vec<(String,)> = sqlx::query_as(
        "SELECT table_name FROM information_schema.tables WHERE table_schema = DATABASE() AND table_name LIKE ? AND table_name NOT LIKE '\\_la\\_%' ORDER BY table_name"
    )
    .bind(&pattern)
    .fetch_all(&state.pool)
    .await?;

    let tables = rows.into_iter().map(|(name,)| TableInfo { name, created_at: None }).collect();
    Ok(Json(tables))
}

pub async fn truncate_table(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    validate_managed_table(&state, &name)?;
    sqlx::query(&format!("TRUNCATE TABLE `{}`", name))
        .execute(&state.pool)
        .await?;
    Ok(Json(serde_json::json!({"ok": true})))
}

pub async fn drop_table(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    validate_managed_table(&state, &name)?;
    sqlx::query(&format!("DROP TABLE IF EXISTS `{}`", name))
        .execute(&state.pool)
        .await?;
    Ok(Json(serde_json::json!({"ok": true})))
}

/// Guard: only allow operations on tables ending with the configured suffix
fn validate_managed_table(state: &crate::AppState, name: &str) -> Result<(), AppError> {
    if !name.ends_with(&state.config.table_suffix) {
        return Err(AppError::Bad(format!(
            "テーブル '{}' は本ツール管理外のテーブルです",
            name
        )));
    }
    Ok(())
}
