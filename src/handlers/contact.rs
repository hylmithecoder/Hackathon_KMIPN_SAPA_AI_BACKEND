use crate::database::scheme::{Contact, Tag};
use crate::error::AppError;
use crate::models::contact::{CreateContactDto, TagContactDto, UpdateContactDto};
use crate::response::ApiResponse;
use crate::state::AppState;
use crate::utils::db::{
    map_mysql_err, opt_str, opt_u64, req_str, req_u64, validate_company, validate_tag,
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

const CONTACT_COLUMNS: &str = "c.id, c.first_name, c.last_name, c.email, c.phone, c.job_title, \
    c.company_id, co.name AS company_name, c.source, c.status, c.assigned_to, c.description, c.created_at, c.updated_at";

fn row_to_contact(row: &mut mysql::Row) -> Result<Contact, AppError> {
    Ok(Contact {
        id: req_u64(row, "id")?,
        first_name: req_str(row, "first_name")?,
        last_name: opt_str(row, "last_name"),
        email: opt_str(row, "email"),
        phone: opt_str(row, "phone"),
        job_title: opt_str(row, "job_title"),
        company_id: opt_u64(row, "company_id"),
        company_name: opt_str(row, "company_name"),
        source: opt_str(row, "source"),
        status: req_str(row, "status")?,
        assigned_to: opt_u64(row, "assigned_to"),
        description: opt_str(row, "description"),
        created_at: opt_str(row, "created_at"),
        updated_at: opt_str(row, "updated_at"),
    })
}

fn apply_tags(
    conn: &mut mysql::PooledConn,
    contact_id: u64,
    tag_ids: &[u64],
) -> Result<(), AppError> {
    conn.exec_drop(
        "DELETE FROM contact_tags WHERE contact_id = :contact_id",
        params! { "contact_id" => contact_id },
    )
    .map_err(map_mysql_err)?;
    for tag_id in tag_ids {
        validate_tag(conn, *tag_id, "tag_id")?;
        conn.exec_drop(
            "INSERT IGNORE INTO contact_tags (contact_id, tag_id) VALUES (:contact_id, :tag_id)",
            params! { "contact_id" => contact_id, "tag_id" => tag_id },
        )
        .map_err(map_mysql_err)?;
    }
    Ok(())
}

pub async fn list_contacts(
    State(state): State<AppState>,
) -> Result<ApiResponse<Vec<Contact>>, AppError> {
    let mut conn = state
        .pool
        .get_conn()
        .map_err(map_mysql_err)?;

    let contacts: Vec<Contact> = conn
        .query_map(
            format!("SELECT {CONTACT_COLUMNS} FROM contacts c LEFT JOIN companies co ON c.company_id = co.id ORDER BY c.id DESC"),
            |mut row: mysql::Row| row_to_contact(&mut row),
        )
        .map_err(map_mysql_err)?
        .into_iter()
        .collect::<Result<Vec<_>, _>>()?;

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
        .map_err(map_mysql_err)?;

    if let Some(company_id) = payload.company_id {
        validate_company(&mut conn, company_id, "company_id")?;
    }
    if let Some(assigned_to) = payload.assigned_to {
        validate_user(&mut conn, assigned_to, "assigned_to")?;
    }

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
    .map_err(map_mysql_err)?;

    let last_id = conn.last_insert_id();

    if let Some(tag_ids) = payload.tag_ids {
        apply_tags(&mut conn, last_id, &tag_ids)?;
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

    state
        .broadcaster
        .notify("contact", ChangeAction::Created, Some(last_id));

    Ok((StatusCode::CREATED, ApiResponse::success(contact)))
}

pub async fn get_contact(
    Path(id): Path<u64>,
    State(state): State<AppState>,
) -> Result<ApiResponse<Contact>, AppError> {
    let mut conn = state
        .pool
        .get_conn()
        .map_err(map_mysql_err)?;

    let contact: Option<Contact> = conn
        .exec_first(
            format!("SELECT {CONTACT_COLUMNS} FROM contacts c LEFT JOIN companies co ON c.company_id = co.id WHERE c.id = :id"),
            params! { "id" => id },
        )
        .map_err(map_mysql_err)?
        .map(|mut row: mysql::Row| row_to_contact(&mut row))
        .transpose()?;

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
        .map_err(map_mysql_err)?;

    let existing: Option<Contact> = conn
        .exec_first(
            format!("SELECT {CONTACT_COLUMNS} FROM contacts c LEFT JOIN companies co ON c.company_id = co.id WHERE c.id = :id"),
            params! { "id" => id },
        )
        .map_err(map_mysql_err)?
        .map(|mut row: mysql::Row| row_to_contact(&mut row))
        .transpose()?;

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
    if let Some(company_id) = payload.company_id {
        validate_company(&mut conn, company_id, "company_id")?;
        contact.company_id = Some(company_id);
    }
    if payload.source.is_some() {
        contact.source = payload.source;
    }
    if let Some(status) = payload.status {
        contact.status = status;
    }
    if let Some(assigned_to) = payload.assigned_to {
        validate_user(&mut conn, assigned_to, "assigned_to")?;
        contact.assigned_to = Some(assigned_to);
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
    .map_err(map_mysql_err)?;

    if let Some(tag_ids) = payload.tag_ids {
        apply_tags(&mut conn, id, &tag_ids)?;
    }

    state
        .broadcaster
        .notify("contact", ChangeAction::Updated, Some(id));

    Ok(ApiResponse::success(contact))
}

pub async fn delete_contact(
    Path(id): Path<u64>,
    State(state): State<AppState>,
) -> Result<StatusCode, AppError> {
    let mut conn = state
        .pool
        .get_conn()
        .map_err(map_mysql_err)?;

    conn.exec_drop(
        "DELETE FROM contacts WHERE id = :id",
        params! { "id" => id },
    )
    .map_err(map_mysql_err)?;

    if conn.affected_rows() > 0 {
        state
            .broadcaster
            .notify("contact", ChangeAction::Deleted, Some(id));
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
        .map_err(map_mysql_err)?;

    let tags: Vec<Tag> = conn
        .exec_map(
            "SELECT t.id, t.name, t.color, t.created_at FROM tags t \
             JOIN contact_tags ct ON t.id = ct.tag_id WHERE ct.contact_id = :id",
            params! { "id" => id },
            |mut row: mysql::Row| -> Result<Tag, AppError> {
                Ok(Tag {
                    id: req_u64(&mut row, "id")?,
                    name: req_str(&mut row, "name")?,
                    color: opt_str(&mut row, "color"),
                    created_at: opt_str(&mut row, "created_at"),
                })
            },
        )
        .map_err(map_mysql_err)?
        .into_iter()
        .collect::<Result<Vec<_>, _>>()?;

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
        .map_err(map_mysql_err)?;

    validate_tag(&mut conn, payload.tag_id, "tag_id")?;

    conn.exec_drop(
        "INSERT IGNORE INTO contact_tags (contact_id, tag_id) VALUES (:contact_id, :tag_id)",
        params! { "contact_id" => id, "tag_id" => payload.tag_id },
    )
    .map_err(map_mysql_err)?;

    state
        .broadcaster
        .notify("contact", ChangeAction::Updated, Some(id));

    Ok(ApiResponse::message("Tag added"))
}

pub async fn remove_tag_from_contact(
    Path((id, tag_id)): Path<(u64, u64)>,
    State(state): State<AppState>,
) -> Result<StatusCode, AppError> {
    let mut conn = state
        .pool
        .get_conn()
        .map_err(map_mysql_err)?;

    conn.exec_drop(
        "DELETE FROM contact_tags WHERE contact_id = :contact_id AND tag_id = :tag_id",
        params! { "contact_id" => id, "tag_id" => tag_id },
    )
    .map_err(map_mysql_err)?;

    state
        .broadcaster
        .notify("contact", ChangeAction::Updated, Some(id));

    Ok(StatusCode::NO_CONTENT)
}
