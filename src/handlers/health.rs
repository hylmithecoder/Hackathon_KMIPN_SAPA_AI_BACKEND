//! Health check handlers.

use axum::{Json, response::IntoResponse};
use serde_json::json;

/// Liveness probe.
pub async fn health_check() -> &'static str {
    "OK"
}

/// Readiness probe.
pub async fn readiness() -> impl IntoResponse {
    Json(json!({
        "status": "ready",
        "timestamp": chrono::Utc::now().to_rfc3339(),
    }))
}
