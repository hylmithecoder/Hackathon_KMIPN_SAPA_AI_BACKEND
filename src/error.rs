//! Centralized error handling.
//!
//! Every handler can return `Result<T, AppError>`. Because `AppError` implements Axum's
//! `IntoResponse`, Axum automatically converts errors into a consistent JSON body.
//!
//! Response shape (per project rule):
//! ```json
//! {
//!   "success": false,
//!   "message": "Detailed error context"
//! }
//! ```

use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde_json::json;
use thiserror::Error;

/// Application-wide error type.
#[derive(Debug, Error)]
pub enum AppError {
    /// Catch-all for unexpected internal failures. The original error is logged but not exposed.
    #[error("Internal server error")]
    Internal(#[from] anyhow::Error),

    /// Validation failures that originate from application logic (not just JSON schema).
    #[error("{0}")]
    Validation(String),

    /// Resource not found.
    #[error("Not found")]
    NotFound,

    /// Generic bad request with a human-readable message.
    #[error("{0}")]
    BadRequest(String),

    /// Authentication failure.
    #[error("Unauthorized")]
    Unauthorized,

    /// Forbidden action.
    #[error("Forbidden")]
    Forbidden,

    /// Conflict, e.g. duplicate unique key.
    #[error("Conflict: {0}")]
    Conflict(String),
}

impl AppError {
    pub fn message(&self) -> String {
        match self {
            AppError::Internal(err) => {
                tracing::error!(error = ?err, "internal error");
                "Internal server error".to_string()
            }
            AppError::Validation(msg) | AppError::BadRequest(msg) | AppError::Conflict(msg) => {
                msg.clone()
            }
            AppError::NotFound => "Not found".to_string(),
            AppError::Unauthorized => "Unauthorized".to_string(),
            AppError::Forbidden => "Forbidden".to_string(),
        }
    }

    pub fn status(&self) -> StatusCode {
        match self {
            AppError::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
            AppError::Validation(_) | AppError::BadRequest(_) => StatusCode::BAD_REQUEST,
            AppError::NotFound => StatusCode::NOT_FOUND,
            AppError::Unauthorized => StatusCode::UNAUTHORIZED,
            AppError::Forbidden => StatusCode::FORBIDDEN,
            AppError::Conflict(_) => StatusCode::CONFLICT,
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let status = self.status();
        let message = self.message();

        let body = Json(json!({
            "success": false,
            "message": message,
        }));

        (status, body).into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_status_codes() {
        assert_eq!(AppError::NotFound.status(), StatusCode::NOT_FOUND);
        assert_eq!(AppError::Unauthorized.status(), StatusCode::UNAUTHORIZED);
        assert_eq!(AppError::Forbidden.status(), StatusCode::FORBIDDEN);
        assert_eq!(
            AppError::Validation("bad".into()).status(),
            StatusCode::BAD_REQUEST
        );
        assert_eq!(
            AppError::Conflict("dup".into()).status(),
            StatusCode::CONFLICT
        );
    }

    #[test]
    fn error_messages() {
        assert_eq!(AppError::NotFound.message(), "Not found");
        assert_eq!(AppError::Validation("invalid".into()).message(), "invalid");
    }
}
