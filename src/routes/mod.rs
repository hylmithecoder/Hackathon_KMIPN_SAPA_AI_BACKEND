//! Route composition.

use crate::state::AppState;
use axum::Router;

pub mod api;
pub mod health;

/// Create the root router, combining health and API routers.
pub fn create_router(state: AppState) -> Router {
    Router::new()
        .merge(health::router())
        .merge(api::router())
        .with_state(state)
}
