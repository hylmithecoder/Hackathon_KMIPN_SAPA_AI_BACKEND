//! Health/readiness routes.

use axum::{Router, routing::get};

use crate::{
    handlers::health::{health_check, readiness},
    state::AppState,
};

/// Router for health endpoints.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/health", get(health_check))
        .route("/health/ready", get(readiness))
}
