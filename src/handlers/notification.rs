use crate::database::scheme::Notification;
use crate::error::AppError;
use crate::models::notification::CreateNotificationDto;
use crate::response::ApiResponse;
use crate::state::AppState;
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use mysql::params;
use mysql::prelude::*;

const NOTIFICATION_COLUMNS: &str =
    "id, user_id, title, body, category, entity_type, entity_id, is_read, created_at";

type NotificationRow = (
    u64,
    u64,
    String,
    String,
    String,
    Option<String>,
    Option<u64>,
    i8,
    Option<String>,
);

fn map_notification(row: NotificationRow) -> Notification {
    let (id, user_id, title, body, category, entity_type, entity_id, is_read, created_at) = row;
    Notification {
        id,
        user_id,
        title,
        body,
        category,
        entity_type,
        entity_id,
        is_read: is_read != 0,
        created_at,
    }
}

pub async fn list_notifications(
    State(state): State<AppState>,
) -> Result<ApiResponse<Vec<Notification>>, AppError> {
    let mut conn = state
        .pool
        .get_conn()
        .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

    let notifications = conn
        .query_map(
            format!("SELECT {NOTIFICATION_COLUMNS} FROM notifications ORDER BY id DESC"),
            map_notification,
        )
        .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

    Ok(ApiResponse::success(notifications))
}

pub async fn create_notification(
    State(state): State<AppState>,
    Json(payload): Json<CreateNotificationDto>,
) -> Result<(StatusCode, ApiResponse<Notification>), AppError> {
    if payload.title.trim().is_empty() {
        return Err(AppError::Validation("title is required".into()));
    }
    if payload.body.trim().is_empty() {
        return Err(AppError::Validation("body is required".into()));
    }

    let category = payload.category.unwrap_or_else(|| "general".to_string());

    let mut conn = state
        .pool
        .get_conn()
        .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

    conn.exec_drop(
        "INSERT INTO notifications (user_id, title, body, category, entity_type, entity_id) \
         VALUES (:user_id, :title, :body, :category, :entity_type, :entity_id)",
        params! {
            "user_id" => payload.user_id,
            "title" => payload.title.trim(),
            "body" => payload.body.trim(),
            "category" => &category,
            "entity_type" => payload.entity_type.as_deref(),
            "entity_id" => payload.entity_id,
        },
    )
    .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

    let last_id = conn.last_insert_id();
    let notification = Notification {
        id: last_id,
        user_id: payload.user_id,
        title: payload.title,
        body: payload.body,
        category,
        entity_type: payload.entity_type,
        entity_id: payload.entity_id,
        is_read: false,
        created_at: None,
    };

    Ok((StatusCode::CREATED, ApiResponse::success(notification)))
}

pub async fn unread_count(State(state): State<AppState>) -> Result<ApiResponse<u64>, AppError> {
    let mut conn = state
        .pool
        .get_conn()
        .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

    let count: Option<u64> = conn
        .query_first("SELECT COUNT(*) FROM notifications WHERE is_read = 0")
        .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

    Ok(ApiResponse::success(count.unwrap_or(0)))
}

pub async fn mark_read(
    Path(id): Path<u64>,
    State(state): State<AppState>,
) -> Result<ApiResponse<Notification>, AppError> {
    let mut conn = state
        .pool
        .get_conn()
        .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

    conn.exec_drop(
        "UPDATE notifications SET is_read = 1 WHERE id = :id",
        params! { "id" => id },
    )
    .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

    if conn.affected_rows() == 0 {
        return Err(AppError::NotFound);
    }

    let notification: Option<Notification> = conn
        .exec_first(
            format!("SELECT {NOTIFICATION_COLUMNS} FROM notifications WHERE id = :id"),
            params! { "id" => id },
        )
        .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?
        .map(map_notification);

    match notification {
        Some(n) => Ok(ApiResponse::success(n)),
        None => Err(AppError::NotFound),
    }
}

pub async fn mark_all_read(State(state): State<AppState>) -> Result<ApiResponse<()>, AppError> {
    let mut conn = state
        .pool
        .get_conn()
        .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

    conn.exec_drop("UPDATE notifications SET is_read = 1 WHERE is_read = 0", ())
        .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

    Ok(ApiResponse::message("All notifications marked read"))
}

pub async fn delete_notification(
    Path(id): Path<u64>,
    State(state): State<AppState>,
) -> Result<StatusCode, AppError> {
    let mut conn = state
        .pool
        .get_conn()
        .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

    conn.exec_drop(
        "DELETE FROM notifications WHERE id = :id",
        params! { "id" => id },
    )
    .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

    if conn.affected_rows() > 0 {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(AppError::NotFound)
    }
}
