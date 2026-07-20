//! User resource handlers.
//!
//! These handlers demonstrate CRUD patterns. They return `Result<T, AppError>` so that
//! errors are automatically converted to the standard JSON error response.

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};

use crate::{
    error::AppError,
    models::user::{CreateUser, User},
    response::ApiResponse,
    state::AppState,
};

/// List all users.
///
/// In a real project this would fetch rows from the database pool stored in `AppState`.
/// The template returns a small static list so the endpoint works immediately.
pub async fn list_users(
    State(_state): State<AppState>,
) -> Result<ApiResponse<Vec<User>>, AppError> {
    let users = vec![User::new("alice"), User::new("bob")];
    Ok(ApiResponse::success(users))
}

/// Create a new user.
///
/// Demonstrates request body deserialization, validation, and returning the newly created resource.
pub async fn create_user(
    Json(payload): Json<CreateUser>,
) -> Result<(StatusCode, ApiResponse<User>), AppError> {
    if payload.name.trim().is_empty() {
        return Err(AppError::Validation("name is required".to_string()));
    }

    let user = User::new(payload.name.trim());
    Ok((StatusCode::CREATED, ApiResponse::success(user)))
}

/// Get a single user by id.
pub async fn get_user(Path(id): Path<String>) -> Result<ApiResponse<User>, AppError> {
    if id == "1" {
        Ok(ApiResponse::success(User::new("alice")))
    } else {
        Err(AppError::NotFound)
    }
}
