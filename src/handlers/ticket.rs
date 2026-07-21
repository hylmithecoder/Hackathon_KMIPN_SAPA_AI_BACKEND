use crate::database::scheme::Ticket;
use crate::error::AppError;
use crate::models::ticket::{CreateTicketDto, TicketStatusDto, UpdateTicketDto};
use crate::response::ApiResponse;
use crate::state::AppState;
use crate::utils::db::{
    map_mysql_err, opt_str, opt_u64, req_str, req_u64, validate_company, validate_contact,
    validate_user,
};
use crate::ws::event::ChangeAction;
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

fn row_to_ticket(row: &mut mysql::Row) -> Result<Ticket, AppError> {
    Ok(Ticket {
        id: req_u64(row, "id")?,
        ticket_number: req_str(row, "ticket_number")?,
        subject: req_str(row, "subject")?,
        description: req_str(row, "description")?,
        contact_id: opt_u64(row, "contact_id"),
        contact_name: opt_str(row, "contact_name"),
        company_id: opt_u64(row, "company_id"),
        company_name: opt_str(row, "company_name"),
        assigned_to: opt_u64(row, "assigned_to"),
        assigned_name: opt_str(row, "assigned_name"),
        priority: req_str(row, "priority")?,
        status: req_str(row, "status")?,
        source: opt_str(row, "source"),
        resolved_at: opt_str(row, "resolved_at"),
        closed_at: opt_str(row, "closed_at"),
        created_at: opt_str(row, "created_at"),
        updated_at: opt_str(row, "updated_at"),
    })
}

pub async fn list_tickets(
    State(state): State<AppState>,
) -> Result<ApiResponse<Vec<Ticket>>, AppError> {
    let mut conn = state
        .pool
        .get_conn()
        .map_err(map_mysql_err)?;

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
        .map_err(map_mysql_err)?
        .into_iter()
        .collect::<Result<Vec<_>, _>>()?;

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
        .map_err(map_mysql_err)?;

    if let Some(contact_id) = payload.contact_id {
        validate_contact(&mut conn, contact_id, "contact_id")?;
    }
    if let Some(company_id) = payload.company_id {
        validate_company(&mut conn, company_id, "company_id")?;
    }
    if let Some(assigned_to) = payload.assigned_to {
        validate_user(&mut conn, assigned_to, "assigned_to")?;
    }

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
    .map_err(map_mysql_err)?;

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

    state
        .broadcaster
        .notify("ticket", ChangeAction::Created, Some(last_id));

    Ok((StatusCode::CREATED, ApiResponse::success(ticket)))
}

pub async fn get_ticket(
    Path(id): Path<u64>,
    State(state): State<AppState>,
) -> Result<ApiResponse<Ticket>, AppError> {
    let mut conn = state
        .pool
        .get_conn()
        .map_err(map_mysql_err)?;

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
        .map_err(map_mysql_err)?
        .map(|mut row: mysql::Row| row_to_ticket(&mut row))
        .transpose()?;

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
        .map_err(map_mysql_err)?;

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
        .map_err(map_mysql_err)?
        .map(|mut row: mysql::Row| row_to_ticket(&mut row))
        .transpose()?;

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
    if let Some(contact_id) = payload.contact_id {
        validate_contact(&mut conn, contact_id, "contact_id")?;
        ticket.contact_id = Some(contact_id);
    }
    if let Some(company_id) = payload.company_id {
        validate_company(&mut conn, company_id, "company_id")?;
        ticket.company_id = Some(company_id);
    }
    if let Some(assigned_to) = payload.assigned_to {
        validate_user(&mut conn, assigned_to, "assigned_to")?;
        ticket.assigned_to = Some(assigned_to);
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
    .map_err(map_mysql_err)?;

    state
        .broadcaster
        .notify("ticket", ChangeAction::Updated, Some(id));

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
        .map_err(map_mysql_err)?;

    let status = payload.status.trim();
    let sql = match status {
        "resolved" => "UPDATE tickets SET status = :status, resolved_at = NOW() WHERE id = :id",
        "closed" => "UPDATE tickets SET status = :status, closed_at = NOW() WHERE id = :id",
        _ => {
            "UPDATE tickets SET status = :status, resolved_at = NULL, closed_at = NULL WHERE id = :id"
        }
    };

    conn.exec_drop(sql, params! { "id" => id, "status" => status })
        .map_err(map_mysql_err)?;

    if conn.affected_rows() == 0 {
        return Err(AppError::NotFound);
    }

    state
        .broadcaster
        .notify("ticket", ChangeAction::Updated, Some(id));

    get_ticket(Path(id), State(state)).await
}

pub async fn delete_ticket(
    Path(id): Path<u64>,
    State(state): State<AppState>,
) -> Result<StatusCode, AppError> {
    let mut conn = state
        .pool
        .get_conn()
        .map_err(map_mysql_err)?;

    conn.exec_drop("DELETE FROM tickets WHERE id = :id", params! { "id" => id })
        .map_err(map_mysql_err)?;

    if conn.affected_rows() > 0 {
        state
            .broadcaster
            .notify("ticket", ChangeAction::Deleted, Some(id));
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(AppError::NotFound)
    }
}
