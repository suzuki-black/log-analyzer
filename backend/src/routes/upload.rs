use axum::{extract::{Multipart, State}, Json};
use std::sync::Arc;
use uuid::Uuid;
use tokio::io::AsyncWriteExt;
use crate::{AppState, error::AppError, models::UploadResponse};

pub async fn upload_file(
    State(state): State<Arc<AppState>>,
    mut multipart: Multipart,
) -> Result<Json<Vec<UploadResponse>>, AppError> {
    let mut results = Vec::new();

    eprintln!("[upload] request received");
    while let Some(field) = multipart.next_field().await.map_err(|e| {
        eprintln!("[upload] multipart field error: {}", e);
        AppError::Bad(e.to_string())
    })? {
        let raw_name = field.file_name().unwrap_or("unknown").to_string();
        // The frontend sends a directory-relative path (e.g. "A/B/a.gz") as the multipart
        // filename so files with the same basename in different sub-folders stay distinct.
        // Preserve that path as `original_name` for display, but flatten to a basename for
        // on-disk storage (the file_id prefix already guarantees uniqueness on disk).
        let original_name = raw_name.trim_start_matches(['/', '\\']).to_string();
        let basename = original_name
            .rsplit(['/', '\\'])
            .next()
            .filter(|s| !s.is_empty())
            .unwrap_or("unknown")
            .to_string();
        eprintln!("[upload] receiving file: {} (raw: {})", original_name, raw_name);

        let data = field.bytes().await.map_err(|e| {
            eprintln!("[upload] bytes read error for {}: {}", original_name, e);
            AppError::Bad(e.to_string())
        })?;
        let size = data.len() as u64;
        eprintln!("[upload] received {} bytes for {}", size, original_name);
        let file_id = Uuid::new_v4().to_string();
        let stored_path = format!("{}/{}_{}", state.config.upload_dir, file_id, basename);

        tokio::fs::create_dir_all(&state.config.upload_dir).await?;
        let mut file = tokio::fs::File::create(&stored_path).await?;
        file.write_all(&data).await?;

        sqlx::query(
            "INSERT INTO _la_files (id, original_name, stored_path, size) VALUES (?, ?, ?, ?)",
        )
        .bind(&file_id)
        .bind(&original_name)
        .bind(&stored_path)
        .bind(size as i64)
        .execute(&state.pool)
        .await?;

        results.push(UploadResponse { file_id, filename: original_name, size });
    }

    Ok(Json(results))
}
