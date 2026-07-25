use axum::{
    extract::Multipart,
    response::IntoResponse,
    Json,
};
use serde_json::json;
use std::path::Path;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use uuid::Uuid;

use crate::error::AppError;

/// Handles POST /api/v1/upload for uploading files (images, documents, stickers).
pub async fn upload_file(mut multipart: Multipart) -> Result<impl IntoResponse, AppError> {
    let upload_dir = "storage/uploads";
    if let Err(e) = tokio::fs::create_dir_all(upload_dir).await {
        return Err(AppError::Internal(anyhow::anyhow!("failed to create uploads directory: {e}")));
    }

    let mut saved_url: Option<String> = None;
    let mut original_filename: Option<String> = None;

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| AppError::BadRequest(format!("multipart error: {e}")))?
    {
        let file_name = field.file_name().unwrap_or("file").to_string();
        original_filename = Some(file_name.clone());

        let ext = Path::new(&file_name)
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("bin");

        let unique_name = format!("{}.{}", Uuid::new_v4(), ext);
        let file_path = format!("{upload_dir}/{unique_name}");

        let data = field
            .bytes()
            .await
            .map_err(|e| AppError::BadRequest(format!("failed to read file bytes: {e}")))?;

        let mut file = File::create(&file_path)
            .await
            .map_err(|e| AppError::Internal(anyhow::anyhow!("failed to create file: {e}")))?;

        file.write_all(&data)
            .await
            .map_err(|e| AppError::Internal(anyhow::anyhow!("failed to write file: {e}")))?;

        saved_url = Some(format!("/uploads/{unique_name}"));
        break;
    }

    let url = saved_url.ok_or_else(|| AppError::BadRequest("No file attached in request".into()))?;

    Ok(Json(json!({
        "success": true,
        "data": {
            "url": url,
            "filename": original_filename
        }
    })))
}
