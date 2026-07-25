//! Route composition.

use crate::state::AppState;
use axum::Router;
use tower_http::services::ServeDir;

pub mod api;
pub mod health;

/// Create the root router, combining health and API routers.
pub fn create_router(state: AppState) -> Router {
    let _ = std::fs::create_dir_all("storage/uploads");
    Router::new()
        .nest_service("/uploads", ServeDir::new("storage/uploads"))
        .merge(health::router())
        .merge(api::router())
        .with_state(state)
}
