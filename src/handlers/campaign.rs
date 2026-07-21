use crate::database::scheme::Campaign;
use crate::error::AppError;
use crate::models::campaign::{CampaignStatusDto, CreateCampaignDto, UpdateCampaignDto};
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

const CAMPAIGN_COLUMNS: &str = "id, name, campaign_type, status, start_date, end_date, budget, currency, \
    target_audience, message_template, sent_count, delivered_count, responded_count, created_by, created_at, updated_at";

fn row_to_campaign(row: &mut mysql::Row) -> Campaign {
    Campaign {
        id: row.take("id").unwrap_or_default(),
        name: row.take("name").unwrap_or_default(),
        campaign_type: row.take("campaign_type").unwrap_or_default(),
        status: row.take("status").unwrap_or_default(),
        start_date: row.take("start_date"),
        end_date: row.take("end_date"),
        budget: row.take("budget"),
        currency: row.take("currency").unwrap_or_default(),
        target_audience: row.take("target_audience"),
        message_template: row.take("message_template"),
        sent_count: row.take("sent_count").unwrap_or_default(),
        delivered_count: row.take("delivered_count").unwrap_or_default(),
        responded_count: row.take("responded_count").unwrap_or_default(),
        created_by: row.take("created_by"),
        created_at: row.take("created_at"),
        updated_at: row.take("updated_at"),
    }
}

pub async fn list_campaigns(
    State(state): State<AppState>,
) -> Result<ApiResponse<Vec<Campaign>>, AppError> {
    let mut conn = state
        .pool
        .get_conn()
        .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

    let campaigns: Vec<Campaign> = conn
        .query_map(
            format!("SELECT {CAMPAIGN_COLUMNS} FROM campaigns ORDER BY id DESC"),
            |mut row: mysql::Row| row_to_campaign(&mut row),
        )
        .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

    Ok(ApiResponse::success(campaigns))
}

pub async fn create_campaign(
    State(state): State<AppState>,
    Json(payload): Json<CreateCampaignDto>,
) -> Result<(StatusCode, ApiResponse<Campaign>), AppError> {
    if payload.name.trim().is_empty() {
        return Err(AppError::Validation("name is required".into()));
    }
    if payload.campaign_type.trim().is_empty() {
        return Err(AppError::Validation("campaign_type is required".into()));
    }

    let currency = payload.currency.unwrap_or_else(|| "IDR".to_string());

    let mut conn = state
        .pool
        .get_conn()
        .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

    conn.exec_drop(
        "INSERT INTO campaigns (name, campaign_type, start_date, end_date, budget, currency, target_audience, message_template) \
         VALUES (:name, :campaign_type, :start_date, :end_date, :budget, :currency, :target_audience, :message_template)",
        params! {
            "name" => payload.name.trim(),
            "campaign_type" => payload.campaign_type.trim(),
            "start_date" => payload.start_date.as_deref(),
            "end_date" => payload.end_date.as_deref(),
            "budget" => payload.budget,
            "currency" => &currency,
            "target_audience" => payload.target_audience.as_deref(),
            "message_template" => payload.message_template.as_deref(),
        },
    )
    .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

    let last_id = conn.last_insert_id();
    let campaign = Campaign {
        id: last_id,
        name: payload.name,
        campaign_type: payload.campaign_type,
        status: "draft".to_string(),
        start_date: payload.start_date,
        end_date: payload.end_date,
        budget: payload.budget,
        currency,
        target_audience: payload.target_audience,
        message_template: payload.message_template,
        sent_count: 0,
        delivered_count: 0,
        responded_count: 0,
        created_by: None,
        created_at: None,
        updated_at: None,
    };

    state
        .broadcaster
        .notify("campaign", ChangeAction::Created, Some(last_id));

    Ok((StatusCode::CREATED, ApiResponse::success(campaign)))
}

pub async fn get_campaign(
    Path(id): Path<u64>,
    State(state): State<AppState>,
) -> Result<ApiResponse<Campaign>, AppError> {
    let mut conn = state
        .pool
        .get_conn()
        .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

    let campaign: Option<Campaign> = conn
        .exec_first(
            format!("SELECT {CAMPAIGN_COLUMNS} FROM campaigns WHERE id = :id"),
            params! { "id" => id },
        )
        .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?
        .map(|mut row: mysql::Row| row_to_campaign(&mut row));

    match campaign {
        Some(c) => Ok(ApiResponse::success(c)),
        None => Err(AppError::NotFound),
    }
}

pub async fn update_campaign(
    Path(id): Path<u64>,
    State(state): State<AppState>,
    Json(payload): Json<UpdateCampaignDto>,
) -> Result<ApiResponse<Campaign>, AppError> {
    let mut conn = state
        .pool
        .get_conn()
        .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

    let existing: Option<Campaign> = conn
        .exec_first(
            format!("SELECT {CAMPAIGN_COLUMNS} FROM campaigns WHERE id = :id"),
            params! { "id" => id },
        )
        .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?
        .map(|mut row: mysql::Row| row_to_campaign(&mut row));

    let Some(mut campaign) = existing else {
        return Err(AppError::NotFound);
    };

    if let Some(name) = payload.name {
        if name.trim().is_empty() {
            return Err(AppError::Validation("name is required".into()));
        }
        campaign.name = name;
    }
    if let Some(campaign_type) = payload.campaign_type {
        campaign.campaign_type = campaign_type;
    }
    if let Some(status) = payload.status {
        campaign.status = status;
    }
    if payload.start_date.is_some() {
        campaign.start_date = payload.start_date;
    }
    if payload.end_date.is_some() {
        campaign.end_date = payload.end_date;
    }
    if payload.budget.is_some() {
        campaign.budget = payload.budget;
    }
    if let Some(currency) = payload.currency {
        campaign.currency = currency;
    }
    if payload.target_audience.is_some() {
        campaign.target_audience = payload.target_audience;
    }
    if payload.message_template.is_some() {
        campaign.message_template = payload.message_template;
    }

    conn.exec_drop(
        "UPDATE campaigns SET name = :name, campaign_type = :campaign_type, status = :status, \
         start_date = :start_date, end_date = :end_date, budget = :budget, currency = :currency, \
         target_audience = :target_audience, message_template = :message_template WHERE id = :id",
        params! {
            "id" => id,
            "name" => &campaign.name,
            "campaign_type" => &campaign.campaign_type,
            "status" => &campaign.status,
            "start_date" => &campaign.start_date,
            "end_date" => &campaign.end_date,
            "budget" => campaign.budget,
            "currency" => &campaign.currency,
            "target_audience" => &campaign.target_audience,
            "message_template" => &campaign.message_template,
        },
    )
    .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

    state
        .broadcaster
        .notify("campaign", ChangeAction::Updated, Some(id));

    Ok(ApiResponse::success(campaign))
}

pub async fn update_campaign_status(
    Path(id): Path<u64>,
    State(state): State<AppState>,
    Json(payload): Json<CampaignStatusDto>,
) -> Result<ApiResponse<Campaign>, AppError> {
    if payload.status.trim().is_empty() {
        return Err(AppError::Validation("status is required".into()));
    }

    let mut conn = state
        .pool
        .get_conn()
        .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

    conn.exec_drop(
        "UPDATE campaigns SET status = :status WHERE id = :id",
        params! { "id" => id, "status" => payload.status.trim() },
    )
    .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

    if conn.affected_rows() == 0 {
        return Err(AppError::NotFound);
    }

    state
        .broadcaster
        .notify("campaign", ChangeAction::Updated, Some(id));

    get_campaign(Path(id), State(state)).await
}

pub async fn delete_campaign(
    Path(id): Path<u64>,
    State(state): State<AppState>,
) -> Result<StatusCode, AppError> {
    let mut conn = state
        .pool
        .get_conn()
        .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

    conn.exec_drop(
        "DELETE FROM campaigns WHERE id = :id",
        params! { "id" => id },
    )
    .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

    if conn.affected_rows() > 0 {
        state
            .broadcaster
            .notify("campaign", ChangeAction::Deleted, Some(id));
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(AppError::NotFound)
    }
}
