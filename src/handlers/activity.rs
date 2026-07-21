use crate::database::scheme::Activity;
use crate::error::AppError;
use crate::models::activity::{CreateActivityDto, UpdateActivityDto};
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

const ACTIVITY_COLUMNS: &str = "a.id, a.activity_type, a.subject, a.description, \
    a.contact_id, CONCAT(c.first_name, ' ', IFNULL(c.last_name, '')) AS contact_name, \
    a.deal_id, d.title AS deal_title, a.company_id, co.name AS company_name, \
    a.assigned_to, u.full_name AS assigned_name, a.due_date, a.completed_at, a.status, \
    a.created_by, a.created_at, a.updated_at";

fn row_to_activity(row: &mut mysql::Row) -> Activity {
    Activity {
        id: row.take("id").unwrap_or_default(),
        activity_type: row.take("activity_type").unwrap_or_default(),
        subject: row.take("subject").unwrap_or_default(),
        description: row.take("description"),
        contact_id: row.take("contact_id"),
        contact_name: row.take("contact_name"),
        deal_id: row.take("deal_id"),
        deal_title: row.take("deal_title"),
        company_id: row.take("company_id"),
        company_name: row.take("company_name"),
        assigned_to: row.take("assigned_to"),
        assigned_name: row.take("assigned_name"),
        due_date: row.take("due_date"),
        completed_at: row.take("completed_at"),
        status: row.take("status").unwrap_or_default(),
        created_by: row.take("created_by"),
        created_at: row.take("created_at"),
        updated_at: row.take("updated_at"),
    }
}

pub async fn list_activities(
    State(state): State<AppState>,
) -> Result<ApiResponse<Vec<Activity>>, AppError> {
    let mut conn = state
        .pool
        .get_conn()
        .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

    let activities: Vec<Activity> = conn
        .query_map(
            format!(
                "SELECT {ACTIVITY_COLUMNS} FROM activities a \
                 LEFT JOIN contacts c ON a.contact_id = c.id \
                 LEFT JOIN deals d ON a.deal_id = d.id \
                 LEFT JOIN companies co ON a.company_id = co.id \
                 LEFT JOIN users u ON a.assigned_to = u.id \
                 ORDER BY a.id DESC"
            ),
            |mut row: mysql::Row| row_to_activity(&mut row),
        )
        .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

    Ok(ApiResponse::success(activities))
}

pub async fn create_activity(
    State(state): State<AppState>,
    Json(payload): Json<CreateActivityDto>,
) -> Result<(StatusCode, ApiResponse<Activity>), AppError> {
    if payload.subject.trim().is_empty() {
        return Err(AppError::Validation("subject is required".into()));
    }
    if payload.activity_type.trim().is_empty() {
        return Err(AppError::Validation("activity_type is required".into()));
    }

    let mut conn = state
        .pool
        .get_conn()
        .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

    conn.exec_drop(
        "INSERT INTO activities (activity_type, subject, description, contact_id, deal_id, company_id, assigned_to, due_date) \
         VALUES (:activity_type, :subject, :description, :contact_id, :deal_id, :company_id, :assigned_to, :due_date)",
        params! {
            "activity_type" => payload.activity_type.trim(),
            "subject" => payload.subject.trim(),
            "description" => payload.description.as_deref(),
            "contact_id" => payload.contact_id,
            "deal_id" => payload.deal_id,
            "company_id" => payload.company_id,
            "assigned_to" => payload.assigned_to,
            "due_date" => payload.due_date.as_deref(),
        },
    )
    .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

    let last_id = conn.last_insert_id();
    let activity = Activity {
        id: last_id,
        activity_type: payload.activity_type,
        subject: payload.subject,
        description: payload.description,
        contact_id: payload.contact_id,
        contact_name: None,
        deal_id: payload.deal_id,
        deal_title: None,
        company_id: payload.company_id,
        company_name: None,
        assigned_to: payload.assigned_to,
        assigned_name: None,
        due_date: payload.due_date,
        completed_at: None,
        status: "pending".to_string(),
        created_by: None,
        created_at: None,
        updated_at: None,
    };

    state
        .broadcaster
        .notify("activity", ChangeAction::Created, Some(last_id));

    Ok((StatusCode::CREATED, ApiResponse::success(activity)))
}

pub async fn get_activity(
    Path(id): Path<u64>,
    State(state): State<AppState>,
) -> Result<ApiResponse<Activity>, AppError> {
    let mut conn = state
        .pool
        .get_conn()
        .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

    let activity: Option<Activity> = conn
        .exec_first(
            format!(
                "SELECT {ACTIVITY_COLUMNS} FROM activities a \
                 LEFT JOIN contacts c ON a.contact_id = c.id \
                 LEFT JOIN deals d ON a.deal_id = d.id \
                 LEFT JOIN companies co ON a.company_id = co.id \
                 LEFT JOIN users u ON a.assigned_to = u.id \
                 WHERE a.id = :id"
            ),
            params! { "id" => id },
        )
        .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?
        .map(|mut row: mysql::Row| row_to_activity(&mut row));

    match activity {
        Some(a) => Ok(ApiResponse::success(a)),
        None => Err(AppError::NotFound),
    }
}

pub async fn update_activity(
    Path(id): Path<u64>,
    State(state): State<AppState>,
    Json(payload): Json<UpdateActivityDto>,
) -> Result<ApiResponse<Activity>, AppError> {
    let mut conn = state
        .pool
        .get_conn()
        .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

    let existing: Option<Activity> = conn
        .exec_first(
            format!(
                "SELECT {ACTIVITY_COLUMNS} FROM activities a \
                 LEFT JOIN contacts c ON a.contact_id = c.id \
                 LEFT JOIN deals d ON a.deal_id = d.id \
                 LEFT JOIN companies co ON a.company_id = co.id \
                 LEFT JOIN users u ON a.assigned_to = u.id \
                 WHERE a.id = :id"
            ),
            params! { "id" => id },
        )
        .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?
        .map(|mut row: mysql::Row| row_to_activity(&mut row));

    let Some(mut activity) = existing else {
        return Err(AppError::NotFound);
    };

    if let Some(activity_type) = payload.activity_type {
        activity.activity_type = activity_type;
    }
    if let Some(subject) = payload.subject {
        if subject.trim().is_empty() {
            return Err(AppError::Validation("subject is required".into()));
        }
        activity.subject = subject;
    }
    if payload.description.is_some() {
        activity.description = payload.description;
    }
    if payload.contact_id.is_some() {
        activity.contact_id = payload.contact_id;
    }
    if payload.deal_id.is_some() {
        activity.deal_id = payload.deal_id;
    }
    if payload.company_id.is_some() {
        activity.company_id = payload.company_id;
    }
    if payload.assigned_to.is_some() {
        activity.assigned_to = payload.assigned_to;
    }
    if payload.due_date.is_some() {
        activity.due_date = payload.due_date;
    }
    if payload.completed_at.is_some() {
        activity.completed_at = payload.completed_at;
    }
    if let Some(status) = payload.status {
        activity.status = status;
    }

    conn.exec_drop(
        "UPDATE activities SET activity_type = :activity_type, subject = :subject, description = :description, \
         contact_id = :contact_id, deal_id = :deal_id, company_id = :company_id, assigned_to = :assigned_to, \
         due_date = :due_date, completed_at = :completed_at, status = :status WHERE id = :id",
        params! {
            "id" => id,
            "activity_type" => &activity.activity_type,
            "subject" => &activity.subject,
            "description" => &activity.description,
            "contact_id" => activity.contact_id,
            "deal_id" => activity.deal_id,
            "company_id" => activity.company_id,
            "assigned_to" => activity.assigned_to,
            "due_date" => &activity.due_date,
            "completed_at" => &activity.completed_at,
            "status" => &activity.status,
        },
    )
    .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

    state
        .broadcaster
        .notify("activity", ChangeAction::Updated, Some(id));

    Ok(ApiResponse::success(activity))
}

pub async fn delete_activity(
    Path(id): Path<u64>,
    State(state): State<AppState>,
) -> Result<StatusCode, AppError> {
    let mut conn = state
        .pool
        .get_conn()
        .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

    conn.exec_drop(
        "DELETE FROM activities WHERE id = :id",
        params! { "id" => id },
    )
    .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

    if conn.affected_rows() > 0 {
        state
            .broadcaster
            .notify("activity", ChangeAction::Deleted, Some(id));
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(AppError::NotFound)
    }
}

pub async fn mark_activity_done(
    Path(id): Path<u64>,
    State(state): State<AppState>,
) -> Result<ApiResponse<Activity>, AppError> {
    let mut conn = state
        .pool
        .get_conn()
        .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

    conn.exec_drop(
        "UPDATE activities SET status = 'completed', completed_at = NOW() WHERE id = :id",
        params! { "id" => id },
    )
    .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

    if conn.affected_rows() == 0 {
        return Err(AppError::NotFound);
    }

    state
        .broadcaster
        .notify("activity", ChangeAction::Updated, Some(id));

    get_activity(Path(id), State(state)).await
}
