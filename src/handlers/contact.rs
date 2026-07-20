use crate::database::scheme::{Contact, Tag};
use crate::error::AppError;
use crate::models::contact::{CreateContactDto, TagContactDto, UpdateContactDto};
use crate::response::ApiResponse;
use crate::state::AppState;
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use mysql::params;
use mysql::prelude::*;

const CONTACT_COLUMNS: &str = "c.id, c.first_name, c.last_name, c.email, c.phone, c.job_title, \
    c.company_id, co.name AS company_name, c.source, c.status, c.assigned_to, c.description, c.created_at, c.updated_at";

fn row_to_contact(row: &mut mysql::Row) -> Contact {
    Contact {
        id: row.take("id").unwrap_or_default(),
        first_name: row.take("first_name").unwrap_or_default(),
        last_name: row.take("last_name"),
        email: row.take("email"),
        phone: row.take("phone"),
        job_title: row.take("job_title"),
        company_id: row.take("company_id"),
        company_name: row.take("company_name"),
        source: row.take("source"),
        status: row.take("status").unwrap_or_default(),
        assigned_to: row.take("assigned_to"),
        description: row.take("description"),
        created_at: row.take("created_at"),
        updated_at: row.take("updated_at"),
    }
}

fn apply_tags(
    conn: &mut mysql::PooledConn,
    contact_id: u64,
    tag_ids: &[u64],
) -> Result<(), mysql::Error> {
    conn.exec_drop(
        "DELETE FROM contact_tags WHERE contact_id = :contact_id",
        params! { "contact_id" => contact_id },
    )?;
    for tag_id in tag_ids {
        conn.exec_drop(
            "INSERT IGNORE INTO contact_tags (contact_id, tag_id) VALUES (:contact_id, :tag_id)",
            params! { "contact_id" => contact_id, "tag_id" => tag_id },
        )?;
    }
    Ok(())
}

pub async fn list_contacts(
    State(state): State<AppState>,
) -> Result<ApiResponse<Vec<Contact>>, AppError> {
    let mut conn = state
        .pool
        .get_conn()
        .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

    let contacts: Vec<Contact> = conn
        .query_map(
            format!("SELECT {CONTACT_COLUMNS} FROM contacts c LEFT JOIN companies co ON c.company_id = co.id ORDER BY c.id DESC"),
            |mut row: mysql::Row| row_to_contact(&mut row),
        )
        .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

    Ok(ApiResponse::success(contacts))
}

pub async fn create_contact(
    State(state): State<AppState>,
    Json(payload): Json<CreateContactDto>,
) -> Result<(StatusCode, ApiResponse<Contact>), AppError> {
    if payload.first_name.trim().is_empty() {
        return Err(AppError::Validation("first_name is required".into()));
    }

    let status = payload.status.unwrap_or_else(|| "lead".to_string());
    let source = payload.source.unwrap_or_else(|| "manual".to_string());

    let mut conn = state
        .pool
        .get_conn()
        .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

    conn.exec_drop(
        "INSERT INTO contacts (first_name, last_name, email, phone, job_title, company_id, source, status, assigned_to, description) \
         VALUES (:first_name, :last_name, :email, :phone, :job_title, :company_id, :source, :status, :assigned_to, :description)",
        params! {
            "first_name" => payload.first_name.trim(),
            "last_name" => payload.last_name.as_deref(),
            "email" => payload.email.as_deref(),
            "phone" => payload.phone.as_deref(),
            "job_title" => payload.job_title.as_deref(),
            "company_id" => payload.company_id,
            "source" => &source,
            "status" => &status,
            "assigned_to" => payload.assigned_to,
            "description" => payload.description.as_deref(),
        },
    )
    .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

    let last_id = conn.last_insert_id();

    if let Some(tag_ids) = payload.tag_ids {
        apply_tags(&mut conn, last_id, &tag_ids)
            .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;
    }

    let contact = Contact {
        id: last_id,
        first_name: payload.first_name,
        last_name: payload.last_name,
        email: payload.email,
        phone: payload.phone,
        job_title: payload.job_title,
        company_id: payload.company_id,
        company_name: None,
        source: Some(source),
        status,
        assigned_to: payload.assigned_to,
        description: payload.description,
        created_at: None,
        updated_at: None,
    };

    Ok((StatusCode::CREATED, ApiResponse::success(contact)))
}

pub async fn get_contact(
    Path(id): Path<u64>,
    State(state): State<AppState>,
) -> Result<ApiResponse<Contact>, AppError> {
    let mut conn = state
        .pool
        .get_conn()
        .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

    let contact: Option<Contact> = conn
        .exec_first(
            format!("SELECT {CONTACT_COLUMNS} FROM contacts c LEFT JOIN companies co ON c.company_id = co.id WHERE c.id = :id"),
            params! { "id" => id },
        )
        .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?
        .map(|mut row: mysql::Row| row_to_contact(&mut row));

    match contact {
        Some(c) => Ok(ApiResponse::success(c)),
        None => Err(AppError::NotFound),
    }
}

pub async fn update_contact(
    Path(id): Path<u64>,
    State(state): State<AppState>,
    Json(payload): Json<UpdateContactDto>,
) -> Result<ApiResponse<Contact>, AppError> {
    let mut conn = state
        .pool
        .get_conn()
        .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

    let existing: Option<Contact> = conn
        .exec_first(
            format!("SELECT {CONTACT_COLUMNS} FROM contacts c LEFT JOIN companies co ON c.company_id = co.id WHERE c.id = :id"),
            params! { "id" => id },
        )
        .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?
        .map(|mut row: mysql::Row| row_to_contact(&mut row));

    let Some(mut contact) = existing else {
        return Err(AppError::NotFound);
    };

    if let Some(first_name) = payload.first_name {
        if first_name.trim().is_empty() {
            return Err(AppError::Validation("first_name is required".into()));
        }
        contact.first_name = first_name;
    }
    if payload.last_name.is_some() {
        contact.last_name = payload.last_name;
    }
    if payload.email.is_some() {
        contact.email = payload.email;
    }
    if payload.phone.is_some() {
        contact.phone = payload.phone;
    }
    if payload.job_title.is_some() {
        contact.job_title = payload.job_title;
    }
    if payload.company_id.is_some() {
        contact.company_id = payload.company_id;
    }
    if payload.source.is_some() {
        contact.source = payload.source;
    }
    if let Some(status) = payload.status {
        contact.status = status;
    }
    if payload.assigned_to.is_some() {
        contact.assigned_to = payload.assigned_to;
    }
    if payload.description.is_some() {
        contact.description = payload.description;
    }

    conn.exec_drop(
        "UPDATE contacts SET first_name = :first_name, last_name = :last_name, email = :email, \
         phone = :phone, job_title = :job_title, company_id = :company_id, source = :source, \
         status = :status, assigned_to = :assigned_to, description = :description WHERE id = :id",
        params! {
            "id" => id,
            "first_name" => &contact.first_name,
            "last_name" => &contact.last_name,
            "email" => &contact.email,
            "phone" => &contact.phone,
            "job_title" => &contact.job_title,
            "company_id" => contact.company_id,
            "source" => &contact.source,
            "status" => &contact.status,
            "assigned_to" => contact.assigned_to,
            "description" => &contact.description,
        },
    )
    .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

    if let Some(tag_ids) = payload.tag_ids {
        apply_tags(&mut conn, id, &tag_ids).map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;
    }

    Ok(ApiResponse::success(contact))
}

pub async fn delete_contact(
    Path(id): Path<u64>,
    State(state): State<AppState>,
) -> Result<StatusCode, AppError> {
    let mut conn = state
        .pool
        .get_conn()
        .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

    conn.exec_drop(
        "DELETE FROM contacts WHERE id = :id",
        params! { "id" => id },
    )
    .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

    if conn.affected_rows() > 0 {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(AppError::NotFound)
    }
}

pub async fn list_contact_tags(
    Path(id): Path<u64>,
    State(state): State<AppState>,
) -> Result<ApiResponse<Vec<Tag>>, AppError> {
    let mut conn = state
        .pool
        .get_conn()
        .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

    let tags: Vec<Tag> = conn
        .exec_map(
            "SELECT t.id, t.name, t.color, t.created_at FROM tags t \
             JOIN contact_tags ct ON t.id = ct.tag_id WHERE ct.contact_id = :id",
            params! { "id" => id },
            |mut row: mysql::Row| Tag {
                id: row.take("id").unwrap_or_default(),
                name: row.take("name").unwrap_or_default(),
                color: row.take("color"),
                created_at: row.take("created_at"),
            },
        )
        .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

    Ok(ApiResponse::success(tags))
}

pub async fn add_tag_to_contact(
    Path(id): Path<u64>,
    State(state): State<AppState>,
    Json(payload): Json<TagContactDto>,
) -> Result<ApiResponse<()>, AppError> {
    let mut conn = state
        .pool
        .get_conn()
        .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

    conn.exec_drop(
        "INSERT IGNORE INTO contact_tags (contact_id, tag_id) VALUES (:contact_id, :tag_id)",
        params! { "contact_id" => id, "tag_id" => payload.tag_id },
    )
    .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

    Ok(ApiResponse::message("Tag added"))
}

pub async fn remove_tag_from_contact(
    Path((id, tag_id)): Path<(u64, u64)>,
    State(state): State<AppState>,
) -> Result<StatusCode, AppError> {
    let mut conn = state
        .pool
        .get_conn()
        .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

    conn.exec_drop(
        "DELETE FROM contact_tags WHERE contact_id = :contact_id AND tag_id = :tag_id",
        params! { "contact_id" => id, "tag_id" => tag_id },
    )
    .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

    Ok(StatusCode::NO_CONTENT)
}
