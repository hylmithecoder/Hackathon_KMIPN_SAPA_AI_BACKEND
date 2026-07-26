use axum::{
    Json,
    extract::{Multipart, multipart::MultipartError},
    http::StatusCode,
    response::IntoResponse,
};
use serde_json::json;
use std::path::Path;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use uuid::Uuid;

use crate::error::AppError;

/// Maximum size of one uploaded file (10 MiB).
pub const MAX_UPLOAD_FILE_SIZE: usize = 10 * 1024 * 1024;

/// Maximum multipart request size, including headers and boundary overhead.
pub const MAX_UPLOAD_REQUEST_SIZE: usize = MAX_UPLOAD_FILE_SIZE + 64 * 1024;

fn upload_size_error() -> AppError {
    AppError::PayloadTooLarge("File exceeds the 10 MB upload limit".into())
}

fn map_multipart_error(error: MultipartError) -> AppError {
    if error.status() == StatusCode::PAYLOAD_TOO_LARGE {
        upload_size_error()
    } else {
        AppError::BadRequest(format!("multipart error: {}", error.body_text()))
    }
}

fn validate_upload_size(size: usize) -> Result<(), AppError> {
    if size > MAX_UPLOAD_FILE_SIZE {
        Err(upload_size_error())
    } else {
        Ok(())
    }
}

/// Handles POST /api/v1/upload for uploading files (images, documents, stickers).
pub async fn upload_file(mut multipart: Multipart) -> Result<impl IntoResponse, AppError> {
    let upload_dir = "storage/uploads";
    if let Err(e) = tokio::fs::create_dir_all(upload_dir).await {
        return Err(AppError::Internal(anyhow::anyhow!(
            "failed to create uploads directory: {e}"
        )));
    }

    let mut saved_url: Option<String> = None;
    let mut original_filename: Option<String> = None;

    while let Some(field) = multipart.next_field().await.map_err(map_multipart_error)? {
        if field.name() != Some("file") {
            continue;
        }

        let file_name = field.file_name().unwrap_or("file").to_string();
        original_filename = Some(file_name.clone());

        let ext = Path::new(&file_name)
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("bin");

        let unique_name = format!("{}.{}", Uuid::new_v4(), ext);
        let file_path = format!("{upload_dir}/{unique_name}");

        let data = field.bytes().await.map_err(map_multipart_error)?;

        validate_upload_size(data.len())?;

        let mut file = File::create(&file_path)
            .await
            .map_err(|e| AppError::Internal(anyhow::anyhow!("failed to create file: {e}")))?;

        file.write_all(&data)
            .await
            .map_err(|e| AppError::Internal(anyhow::anyhow!("failed to write file: {e}")))?;

        saved_url = Some(format!("/uploads/{unique_name}"));
        break;
    }

    let url =
        saved_url.ok_or_else(|| AppError::BadRequest("No file attached in request".into()))?;

    Ok(Json(json!({
        "success": true,
        "data": {
            "url": url,
            "filename": original_filename
        }
    })))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upload_size_accepts_ten_mebibytes() {
        assert!(validate_upload_size(MAX_UPLOAD_FILE_SIZE).is_ok());
    }

    #[test]
    fn upload_size_rejects_more_than_ten_mebibytes() {
        assert!(matches!(
            validate_upload_size(MAX_UPLOAD_FILE_SIZE + 1),
            Err(AppError::PayloadTooLarge(_))
        ));
    }
}
