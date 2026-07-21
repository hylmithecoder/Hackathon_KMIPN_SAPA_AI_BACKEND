use crate::database::scheme::Tag;
use crate::error::AppError;
use crate::models::tag::{CreateTagDto, UpdateTagDto};
use crate::response::ApiResponse;
use crate::state::AppState;
use crate::ws::event::ChangeAction;
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use mysql::params;
use mysql::prelude::*;

const TAG_COLUMNS: &str = "id, name, color, created_at";

type TagRow = (u64, String, Option<String>, Option<String>);

fn map_tag(row: TagRow) -> Tag {
    let (id, name, color, created_at) = row;
    Tag {
        id,
        name,
        color,
        created_at,
    }
}

pub async fn list_tags(State(state): State<AppState>) -> Result<ApiResponse<Vec<Tag>>, AppError> {
    let mut conn = state
        .pool
        .get_conn()
        .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

    let tags = conn
        .query_map(
            format!("SELECT {TAG_COLUMNS} FROM tags ORDER BY id DESC"),
            map_tag,
        )
        .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

    Ok(ApiResponse::success(tags))
}

pub async fn create_tag(
    State(state): State<AppState>,
    Json(payload): Json<CreateTagDto>,
) -> Result<(StatusCode, ApiResponse<Tag>), AppError> {
    if payload.name.trim().is_empty() {
        return Err(AppError::Validation("name is required".into()));
    }

    let mut conn = state
        .pool
        .get_conn()
        .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

    conn.exec_drop(
        "INSERT INTO tags (name, color) VALUES (:name, :color)",
        params! {
            "name" => payload.name.trim(),
            "color" => payload.color.as_deref(),
        },
    )
    .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

    let last_id = conn.last_insert_id();
    let tag = Tag {
        id: last_id,
        name: payload.name,
        color: payload.color,
        created_at: None,
    };

    state
        .broadcaster
        .notify("tag", ChangeAction::Created, Some(last_id));

    Ok((StatusCode::CREATED, ApiResponse::success(tag)))
}

pub async fn get_tag(
    Path(id): Path<u64>,
    State(state): State<AppState>,
) -> Result<ApiResponse<Tag>, AppError> {
    let mut conn = state
        .pool
        .get_conn()
        .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

    let tag: Option<Tag> = conn
        .exec_first(
            format!("SELECT {TAG_COLUMNS} FROM tags WHERE id = :id"),
            params! { "id" => id },
        )
        .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?
        .map(map_tag);

    match tag {
        Some(t) => Ok(ApiResponse::success(t)),
        None => Err(AppError::NotFound),
    }
}

pub async fn update_tag(
    Path(id): Path<u64>,
    State(state): State<AppState>,
    Json(payload): Json<UpdateTagDto>,
) -> Result<ApiResponse<Tag>, AppError> {
    let mut conn = state
        .pool
        .get_conn()
        .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

    let existing: Option<Tag> = conn
        .exec_first(
            format!("SELECT {TAG_COLUMNS} FROM tags WHERE id = :id"),
            params! { "id" => id },
        )
        .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?
        .map(map_tag);

    let Some(mut tag) = existing else {
        return Err(AppError::NotFound);
    };

    if let Some(name) = payload.name {
        if name.trim().is_empty() {
            return Err(AppError::Validation("name is required".into()));
        }
        tag.name = name;
    }
    if payload.color.is_some() {
        tag.color = payload.color;
    }

    conn.exec_drop(
        "UPDATE tags SET name = :name, color = :color WHERE id = :id",
        params! {
            "id" => id,
            "name" => &tag.name,
            "color" => &tag.color,
        },
    )
    .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

    state
        .broadcaster
        .notify("tag", ChangeAction::Updated, Some(id));

    Ok(ApiResponse::success(tag))
}

pub async fn delete_tag(
    Path(id): Path<u64>,
    State(state): State<AppState>,
) -> Result<StatusCode, AppError> {
    let mut conn = state
        .pool
        .get_conn()
        .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

    conn.exec_drop("DELETE FROM tags WHERE id = :id", params! { "id" => id })
        .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

    if conn.affected_rows() > 0 {
        state
            .broadcaster
            .notify("tag", ChangeAction::Deleted, Some(id));
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(AppError::NotFound)
    }
}
