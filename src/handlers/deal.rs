use crate::database::scheme::Deal;
use crate::error::AppError;
use crate::models::deal::{CreateDealDto, DealStageMoveDto, UpdateDealDto};
use crate::response::ApiResponse;
use crate::state::AppState;
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use mysql::params;
use mysql::prelude::*;

const DEAL_COLUMNS: &str = "d.id, d.title, d.contact_id, \
    CONCAT(c.first_name, ' ', IFNULL(c.last_name, '')) AS contact_name, \
    d.company_id, co.name AS company_name, d.stage_id, s.name AS stage_name, \
    d.owner_id, u.full_name AS owner_name, d.value, d.currency, d.expected_close_date, \
    d.actual_close_date, d.status, d.description, d.created_at, d.updated_at";

fn row_to_deal(row: &mut mysql::Row) -> Deal {
    Deal {
        id: row.take("id").unwrap_or_default(),
        title: row.take("title").unwrap_or_default(),
        contact_id: row.take("contact_id").unwrap_or_default(),
        contact_name: row.take("contact_name"),
        company_id: row.take("company_id"),
        company_name: row.take("company_name"),
        stage_id: row.take("stage_id").unwrap_or_default(),
        stage_name: row.take("stage_name"),
        owner_id: row.take("owner_id"),
        owner_name: row.take("owner_name"),
        value: row.take("value").unwrap_or_default(),
        currency: row.take("currency").unwrap_or_default(),
        expected_close_date: row.take("expected_close_date"),
        actual_close_date: row.take("actual_close_date"),
        status: row.take("status").unwrap_or_default(),
        description: row.take("description"),
        created_at: row.take("created_at"),
        updated_at: row.take("updated_at"),
    }
}

pub async fn list_deals(State(state): State<AppState>) -> Result<ApiResponse<Vec<Deal>>, AppError> {
    let mut conn = state
        .pool
        .get_conn()
        .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

    let deals: Vec<Deal> = conn
        .query_map(
            format!(
                "SELECT {DEAL_COLUMNS} FROM deals d \
                 JOIN contacts c ON d.contact_id = c.id \
                 LEFT JOIN companies co ON d.company_id = co.id \
                 LEFT JOIN deal_stages s ON d.stage_id = s.id \
                 LEFT JOIN users u ON d.owner_id = u.id \
                 ORDER BY d.id DESC"
            ),
            |mut row: mysql::Row| row_to_deal(&mut row),
        )
        .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

    Ok(ApiResponse::success(deals))
}

pub async fn create_deal(
    State(state): State<AppState>,
    Json(payload): Json<CreateDealDto>,
) -> Result<(StatusCode, ApiResponse<Deal>), AppError> {
    if payload.title.trim().is_empty() {
        return Err(AppError::Validation("title is required".into()));
    }

    let status = payload.status.unwrap_or_else(|| "open".to_string());
    let currency = payload.currency.unwrap_or_else(|| "IDR".to_string());
    let value = payload.value.unwrap_or(0.0);

    let mut conn = state
        .pool
        .get_conn()
        .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

    conn.exec_drop(
        "INSERT INTO deals (title, contact_id, company_id, stage_id, owner_id, value, currency, expected_close_date, status, description) \
         VALUES (:title, :contact_id, :company_id, :stage_id, :owner_id, :value, :currency, :expected_close_date, :status, :description)",
        params! {
            "title" => payload.title.trim(),
            "contact_id" => payload.contact_id,
            "company_id" => payload.company_id,
            "stage_id" => payload.stage_id,
            "owner_id" => payload.owner_id,
            "value" => value,
            "currency" => &currency,
            "expected_close_date" => payload.expected_close_date.as_deref(),
            "status" => &status,
            "description" => payload.description.as_deref(),
        },
    )
    .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

    let last_id = conn.last_insert_id();
    let deal = Deal {
        id: last_id,
        title: payload.title,
        contact_id: payload.contact_id,
        contact_name: None,
        company_id: payload.company_id,
        company_name: None,
        stage_id: payload.stage_id,
        stage_name: None,
        owner_id: payload.owner_id,
        owner_name: None,
        value,
        currency,
        expected_close_date: payload.expected_close_date,
        actual_close_date: None,
        status,
        description: payload.description,
        created_at: None,
        updated_at: None,
    };

    Ok((StatusCode::CREATED, ApiResponse::success(deal)))
}

pub async fn get_deal(
    Path(id): Path<u64>,
    State(state): State<AppState>,
) -> Result<ApiResponse<Deal>, AppError> {
    let mut conn = state
        .pool
        .get_conn()
        .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

    let deal: Option<Deal> = conn
        .exec_first(
            format!(
                "SELECT {DEAL_COLUMNS} FROM deals d \
                 JOIN contacts c ON d.contact_id = c.id \
                 LEFT JOIN companies co ON d.company_id = co.id \
                 LEFT JOIN deal_stages s ON d.stage_id = s.id \
                 LEFT JOIN users u ON d.owner_id = u.id \
                 WHERE d.id = :id"
            ),
            params! { "id" => id },
        )
        .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?
        .map(|mut row: mysql::Row| row_to_deal(&mut row));

    match deal {
        Some(d) => Ok(ApiResponse::success(d)),
        None => Err(AppError::NotFound),
    }
}

pub async fn update_deal(
    Path(id): Path<u64>,
    State(state): State<AppState>,
    Json(payload): Json<UpdateDealDto>,
) -> Result<ApiResponse<Deal>, AppError> {
    let mut conn = state
        .pool
        .get_conn()
        .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

    let existing: Option<Deal> = conn
        .exec_first(
            format!(
                "SELECT {DEAL_COLUMNS} FROM deals d \
                 JOIN contacts c ON d.contact_id = c.id \
                 LEFT JOIN companies co ON d.company_id = co.id \
                 LEFT JOIN deal_stages s ON d.stage_id = s.id \
                 LEFT JOIN users u ON d.owner_id = u.id \
                 WHERE d.id = :id"
            ),
            params! { "id" => id },
        )
        .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?
        .map(|mut row: mysql::Row| row_to_deal(&mut row));

    let Some(mut deal) = existing else {
        return Err(AppError::NotFound);
    };

    if let Some(title) = payload.title {
        if title.trim().is_empty() {
            return Err(AppError::Validation("title is required".into()));
        }
        deal.title = title;
    }
    if let Some(contact_id) = payload.contact_id {
        deal.contact_id = contact_id;
    }
    if payload.company_id.is_some() {
        deal.company_id = payload.company_id;
    }
    if let Some(stage_id) = payload.stage_id {
        deal.stage_id = stage_id;
    }
    if payload.owner_id.is_some() {
        deal.owner_id = payload.owner_id;
    }
    if let Some(value) = payload.value {
        deal.value = value;
    }
    if let Some(currency) = payload.currency {
        deal.currency = currency;
    }
    if payload.expected_close_date.is_some() {
        deal.expected_close_date = payload.expected_close_date;
    }
    if payload.actual_close_date.is_some() {
        deal.actual_close_date = payload.actual_close_date;
    }
    if let Some(status) = payload.status {
        deal.status = status;
    }
    if payload.description.is_some() {
        deal.description = payload.description;
    }

    conn.exec_drop(
        "UPDATE deals SET title = :title, contact_id = :contact_id, company_id = :company_id, \
         stage_id = :stage_id, owner_id = :owner_id, value = :value, currency = :currency, \
         expected_close_date = :expected_close_date, actual_close_date = :actual_close_date, \
         status = :status, description = :description WHERE id = :id",
        params! {
            "id" => id,
            "title" => &deal.title,
            "contact_id" => deal.contact_id,
            "company_id" => deal.company_id,
            "stage_id" => deal.stage_id,
            "owner_id" => deal.owner_id,
            "value" => deal.value,
            "currency" => &deal.currency,
            "expected_close_date" => &deal.expected_close_date,
            "actual_close_date" => &deal.actual_close_date,
            "status" => &deal.status,
            "description" => &deal.description,
        },
    )
    .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

    Ok(ApiResponse::success(deal))
}

pub async fn delete_deal(
    Path(id): Path<u64>,
    State(state): State<AppState>,
) -> Result<StatusCode, AppError> {
    let mut conn = state
        .pool
        .get_conn()
        .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

    conn.exec_drop("DELETE FROM deals WHERE id = :id", params! { "id" => id })
        .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

    if conn.affected_rows() > 0 {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(AppError::NotFound)
    }
}

pub async fn move_deal_stage(
    Path(id): Path<u64>,
    State(state): State<AppState>,
    Json(payload): Json<DealStageMoveDto>,
) -> Result<ApiResponse<Deal>, AppError> {
    let mut conn = state
        .pool
        .get_conn()
        .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

    let stage_exists: Option<u8> = conn
        .exec_first(
            "SELECT 1 FROM deal_stages WHERE id = :id",
            params! { "id" => payload.stage_id },
        )
        .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;
    if stage_exists.is_none() {
        return Err(AppError::NotFound);
    }

    conn.exec_drop(
        "UPDATE deals SET stage_id = :stage_id WHERE id = :id",
        params! { "id" => id, "stage_id" => payload.stage_id },
    )
    .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

    get_deal(Path(id), State(state)).await
}
