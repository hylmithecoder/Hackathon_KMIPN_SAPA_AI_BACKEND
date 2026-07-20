use crate::database::scheme::Ticket;
use crate::error::AppError;
use crate::models::ticket::{CreateTicketDto, TicketStatusDto, UpdateTicketDto};
use crate::response::ApiResponse;
use crate::state::AppState;
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use mysql::params;
use mysql::prelude::*;

const TICKET_COLUMNS: &str = "t.id, t.ticket_number, t.subject, t.description, t.contact_id, \
    CONCAT(c.first_name, ' ', IFNULL(c.last_name, '')) AS contact_name, t.company_id, co.name AS company_name, \
    t.assigned_to, u.full_name AS assigned_name, t.priority, t.status, t.source, t.resolved_at, t.closed_at, t.created_at, t.updated_at";

fn row_to_ticket(row: &mut mysql::Row) -> Ticket {
    Ticket {
        id: row.take("id").unwrap_or_default(),
        ticket_number: row.take("ticket_number").unwrap_or_default(),
        subject: row.take("subject").unwrap_or_default(),
        description: row.take("description").unwrap_or_default(),
        contact_id: row.take("contact_id"),
        contact_name: row.take("contact_name"),
        company_id: row.take("company_id"),
        company_name: row.take("company_name"),
        assigned_to: row.take("assigned_to"),
        assigned_name: row.take("assigned_name"),
        priority: row.take("priority").unwrap_or_default(),
        status: row.take("status").unwrap_or_default(),
        source: row.take("source"),
        resolved_at: row.take("resolved_at"),
        closed_at: row.take("closed_at"),
        created_at: row.take("created_at"),
        updated_at: row.take("updated_at"),
    }
}

pub async fn list_tickets(
    State(state): State<AppState>,
) -> Result<ApiResponse<Vec<Ticket>>, AppError> {
    let mut conn = state
        .pool
        .get_conn()
        .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

    let tickets: Vec<Ticket> = conn
        .query_map(
            format!(
                "SELECT {TICKET_COLUMNS} FROM tickets t \
                 LEFT JOIN contacts c ON t.contact_id = c.id \
                 LEFT JOIN companies co ON t.company_id = co.id \
                 LEFT JOIN users u ON t.assigned_to = u.id \
                 ORDER BY t.id DESC"
            ),
            |mut row: mysql::Row| row_to_ticket(&mut row),
        )
        .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

    Ok(ApiResponse::success(tickets))
}

pub async fn create_ticket(
    State(state): State<AppState>,
    Json(payload): Json<CreateTicketDto>,
) -> Result<(StatusCode, ApiResponse<Ticket>), AppError> {
    if payload.ticket_number.trim().is_empty() {
        return Err(AppError::Validation("ticket_number is required".into()));
    }
    if payload.subject.trim().is_empty() {
        return Err(AppError::Validation("subject is required".into()));
    }

    let priority = payload.priority.unwrap_or_else(|| "medium".to_string());
    let source = payload.source.unwrap_or_else(|| "email".to_string());

    let mut conn = state
        .pool
        .get_conn()
        .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

    conn.exec_drop(
        "INSERT INTO tickets (ticket_number, subject, description, contact_id, company_id, assigned_to, priority, source) \
         VALUES (:ticket_number, :subject, :description, :contact_id, :company_id, :assigned_to, :priority, :source)",
        params! {
            "ticket_number" => payload.ticket_number.trim(),
            "subject" => payload.subject.trim(),
            "description" => payload.description.trim(),
            "contact_id" => payload.contact_id,
            "company_id" => payload.company_id,
            "assigned_to" => payload.assigned_to,
            "priority" => &priority,
            "source" => &source,
        },
    )
    .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

    let last_id = conn.last_insert_id();
    let ticket = Ticket {
        id: last_id,
        ticket_number: payload.ticket_number,
        subject: payload.subject,
        description: payload.description,
        contact_id: payload.contact_id,
        contact_name: None,
        company_id: payload.company_id,
        company_name: None,
        assigned_to: payload.assigned_to,
        assigned_name: None,
        priority,
        status: "open".to_string(),
        source: Some(source),
        resolved_at: None,
        closed_at: None,
        created_at: None,
        updated_at: None,
    };

    Ok((StatusCode::CREATED, ApiResponse::success(ticket)))
}

pub async fn get_ticket(
    Path(id): Path<u64>,
    State(state): State<AppState>,
) -> Result<ApiResponse<Ticket>, AppError> {
    let mut conn = state
        .pool
        .get_conn()
        .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

    let ticket: Option<Ticket> = conn
        .exec_first(
            format!(
                "SELECT {TICKET_COLUMNS} FROM tickets t \
                 LEFT JOIN contacts c ON t.contact_id = c.id \
                 LEFT JOIN companies co ON t.company_id = co.id \
                 LEFT JOIN users u ON t.assigned_to = u.id \
                 WHERE t.id = :id"
            ),
            params! { "id" => id },
        )
        .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?
        .map(|mut row: mysql::Row| row_to_ticket(&mut row));

    match ticket {
        Some(t) => Ok(ApiResponse::success(t)),
        None => Err(AppError::NotFound),
    }
}

pub async fn update_ticket(
    Path(id): Path<u64>,
    State(state): State<AppState>,
    Json(payload): Json<UpdateTicketDto>,
) -> Result<ApiResponse<Ticket>, AppError> {
    let mut conn = state
        .pool
        .get_conn()
        .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

    let existing: Option<Ticket> = conn
        .exec_first(
            format!(
                "SELECT {TICKET_COLUMNS} FROM tickets t \
                 LEFT JOIN contacts c ON t.contact_id = c.id \
                 LEFT JOIN companies co ON t.company_id = co.id \
                 LEFT JOIN users u ON t.assigned_to = u.id \
                 WHERE t.id = :id"
            ),
            params! { "id" => id },
        )
        .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?
        .map(|mut row: mysql::Row| row_to_ticket(&mut row));

    let Some(mut ticket) = existing else {
        return Err(AppError::NotFound);
    };

    if let Some(subject) = payload.subject {
        if subject.trim().is_empty() {
            return Err(AppError::Validation("subject is required".into()));
        }
        ticket.subject = subject;
    }
    if let Some(description) = payload.description {
        ticket.description = description;
    }
    if payload.contact_id.is_some() {
        ticket.contact_id = payload.contact_id;
    }
    if payload.company_id.is_some() {
        ticket.company_id = payload.company_id;
    }
    if payload.assigned_to.is_some() {
        ticket.assigned_to = payload.assigned_to;
    }
    if let Some(priority) = payload.priority {
        ticket.priority = priority;
    }
    if let Some(source) = payload.source {
        ticket.source = Some(source);
    }

    conn.exec_drop(
        "UPDATE tickets SET subject = :subject, description = :description, contact_id = :contact_id, \
         company_id = :company_id, assigned_to = :assigned_to, priority = :priority, source = :source WHERE id = :id",
        params! {
            "id" => id,
            "subject" => &ticket.subject,
            "description" => &ticket.description,
            "contact_id" => ticket.contact_id,
            "company_id" => ticket.company_id,
            "assigned_to" => ticket.assigned_to,
            "priority" => &ticket.priority,
            "source" => &ticket.source,
        },
    )
    .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

    Ok(ApiResponse::success(ticket))
}

pub async fn update_ticket_status(
    Path(id): Path<u64>,
    State(state): State<AppState>,
    Json(payload): Json<TicketStatusDto>,
) -> Result<ApiResponse<Ticket>, AppError> {
    if payload.status.trim().is_empty() {
        return Err(AppError::Validation("status is required".into()));
    }

    let mut conn = state
        .pool
        .get_conn()
        .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

    let status = payload.status.trim();
    let sql = match status {
        "resolved" => "UPDATE tickets SET status = :status, resolved_at = NOW() WHERE id = :id",
        "closed" => "UPDATE tickets SET status = :status, closed_at = NOW() WHERE id = :id",
        _ => {
            "UPDATE tickets SET status = :status, resolved_at = NULL, closed_at = NULL WHERE id = :id"
        }
    };

    conn.exec_drop(sql, params! { "id" => id, "status" => status })
        .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

    if conn.affected_rows() == 0 {
        return Err(AppError::NotFound);
    }

    get_ticket(Path(id), State(state)).await
}

pub async fn delete_ticket(
    Path(id): Path<u64>,
    State(state): State<AppState>,
) -> Result<StatusCode, AppError> {
    let mut conn = state
        .pool
        .get_conn()
        .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

    conn.exec_drop("DELETE FROM tickets WHERE id = :id", params! { "id" => id })
        .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

    if conn.affected_rows() > 0 {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(AppError::NotFound)
    }
}
