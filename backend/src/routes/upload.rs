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

    while let Some(field) = multipart.next_field().await.map_err(|e| AppError::Bad(e.to_string()))? {
        let original_name = field
            .file_name()
            .unwrap_or("unknown")
            .to_string();

        let data = field.bytes().await.map_err(|e| AppError::Bad(e.to_string()))?;
        let size = data.len() as u64;
        let file_id = Uuid::new_v4().to_string();
        let stored_path = format!("{}/{}_{}", state.config.upload_dir, file_id, original_name);

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
