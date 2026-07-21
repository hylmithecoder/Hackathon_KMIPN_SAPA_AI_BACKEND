use crate::database::scheme::DealStage;
use crate::error::AppError;
use crate::models::deal_stage::{CreateDealStageDto, ReorderDealStageDto, UpdateDealStageDto};
use crate::response::ApiResponse;
use crate::state::AppState;
use crate::utils::db::{map_mysql_err, opt_str, req_bool, req_f64, req_i32, req_str, req_u64};
use crate::ws::event::ChangeAction;
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use mysql::params;
use mysql::prelude::*;

const STAGE_COLUMNS: &str = "id, name, position, probability, color, is_active, created_at";

fn row_to_stage(row: &mut mysql::Row) -> Result<DealStage, AppError> {
    Ok(DealStage {
        id: req_u64(row, "id")?,
        name: req_str(row, "name")?,
        position: req_i32(row, "position")?,
        probability: req_f64(row, "probability")?,
        color: opt_str(row, "color"),
        is_active: req_bool(row, "is_active")?,
        created_at: opt_str(row, "created_at"),
    })
}

pub async fn list_stages(
    State(state): State<AppState>,
) -> Result<ApiResponse<Vec<DealStage>>, AppError> {
    let mut conn = state
        .pool
        .get_conn()
        .map_err(map_mysql_err)?;

    let stages: Vec<DealStage> = conn
        .query_map(
            format!("SELECT {STAGE_COLUMNS} FROM deal_stages ORDER BY position, id"),
            |mut row: mysql::Row| row_to_stage(&mut row),
        )
        .map_err(map_mysql_err)?
        .into_iter()
        .collect::<Result<Vec<_>, _>>()?;

    Ok(ApiResponse::success(stages))
}

pub async fn create_stage(
    State(state): State<AppState>,
    Json(payload): Json<CreateDealStageDto>,
) -> Result<(StatusCode, ApiResponse<DealStage>), AppError> {
    if payload.name.trim().is_empty() {
        return Err(AppError::Validation("name is required".into()));
    }

    let mut conn = state
        .pool
        .get_conn()
        .map_err(map_mysql_err)?;

    conn.exec_drop(
        "INSERT INTO deal_stages (name, position, probability, color) VALUES (:name, :position, :probability, :color)",
        params! {
            "name" => payload.name.trim(),
            "position" => payload.position,
            "probability" => payload.probability.unwrap_or(0.0),
            "color" => payload.color.as_deref(),
        },
    )
    .map_err(map_mysql_err)?;

    let last_id = conn.last_insert_id();
    let stage = DealStage {
        id: last_id,
        name: payload.name,
        position: payload.position,
        probability: payload.probability.unwrap_or(0.0),
        color: payload.color,
        is_active: true,
        created_at: None,
    };

    state
        .broadcaster
        .notify("deal_stage", ChangeAction::Created, Some(last_id));

    Ok((StatusCode::CREATED, ApiResponse::success(stage)))
}

pub async fn get_stage(
    Path(id): Path<u64>,
    State(state): State<AppState>,
) -> Result<ApiResponse<DealStage>, AppError> {
    let mut conn = state
        .pool
        .get_conn()
        .map_err(map_mysql_err)?;

    let stage: Option<DealStage> = conn
        .exec_first(
            format!("SELECT {STAGE_COLUMNS} FROM deal_stages WHERE id = :id"),
            params! { "id" => id },
        )
        .map_err(map_mysql_err)?
        .map(|mut row: mysql::Row| row_to_stage(&mut row))
        .transpose()?;

    match stage {
        Some(s) => Ok(ApiResponse::success(s)),
        None => Err(AppError::NotFound),
    }
}

pub async fn update_stage(
    Path(id): Path<u64>,
    State(state): State<AppState>,
    Json(payload): Json<UpdateDealStageDto>,
) -> Result<ApiResponse<DealStage>, AppError> {
    let mut conn = state
        .pool
        .get_conn()
        .map_err(map_mysql_err)?;

    let existing: Option<DealStage> = conn
        .exec_first(
            format!("SELECT {STAGE_COLUMNS} FROM deal_stages WHERE id = :id"),
            params! { "id" => id },
        )
        .map_err(map_mysql_err)?
        .map(|mut row: mysql::Row| row_to_stage(&mut row))
        .transpose()?;

    let Some(mut stage) = existing else {
        return Err(AppError::NotFound);
    };

    if let Some(name) = payload.name {
        if name.trim().is_empty() {
            return Err(AppError::Validation("name is required".into()));
        }
        stage.name = name;
    }
    if let Some(position) = payload.position {
        stage.position = position;
    }
    if let Some(probability) = payload.probability {
        stage.probability = probability;
    }
    if payload.color.is_some() {
        stage.color = payload.color;
    }
    if let Some(is_active) = payload.is_active {
        stage.is_active = is_active;
    }

    conn.exec_drop(
        "UPDATE deal_stages SET name = :name, position = :position, probability = :probability, \
         color = :color, is_active = :is_active WHERE id = :id",
        params! {
            "id" => id,
            "name" => &stage.name,
            "position" => stage.position,
            "probability" => stage.probability,
            "color" => &stage.color,
            "is_active" => stage.is_active as i8,
        },
    )
    .map_err(map_mysql_err)?;

    state
        .broadcaster
        .notify("deal_stage", ChangeAction::Updated, Some(id));

    Ok(ApiResponse::success(stage))
}

pub async fn delete_stage(
    Path(id): Path<u64>,
    State(state): State<AppState>,
) -> Result<StatusCode, AppError> {
    let mut conn = state
        .pool
        .get_conn()
        .map_err(map_mysql_err)?;

    conn.exec_drop(
        "DELETE FROM deal_stages WHERE id = :id",
        params! { "id" => id },
    )
    .map_err(map_mysql_err)?;

    if conn.affected_rows() > 0 {
        state
            .broadcaster
            .notify("deal_stage", ChangeAction::Deleted, Some(id));
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(AppError::NotFound)
    }
}

pub async fn reorder_stages(
    State(state): State<AppState>,
    Json(payload): Json<ReorderDealStageDto>,
) -> Result<ApiResponse<()>, AppError> {
    let mut conn = state
        .pool
        .get_conn()
        .map_err(map_mysql_err)?;

    for (position, id) in payload.ordered_ids.iter().enumerate() {
        conn.exec_drop(
            "UPDATE deal_stages SET position = :position WHERE id = :id",
            params! { "position" => position as i32, "id" => id },
        )
        .map_err(map_mysql_err)?;
    }

    state
        .broadcaster
        .notify("deal_stage", ChangeAction::Updated, None);

    Ok(ApiResponse::message("Stages reordered"))
}
