use crate::database::scheme::Note;
use crate::error::AppError;
use crate::models::note::{CreateNoteDto, UpdateNoteDto};
use crate::response::ApiResponse;
use crate::state::AppState;
use crate::utils::db::{map_mysql_err, validate_company, validate_contact, validate_deal};
use crate::ws::event::ChangeAction;
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use mysql::params;
use mysql::prelude::*;

const NOTE_COLUMNS: &str = "n.id, n.content, n.contact_id, n.deal_id, n.company_id, n.created_by, \
    DATE_FORMAT(n.created_at, '%Y-%m-%d %H:%i:%s') AS created_at, \
    DATE_FORMAT(n.updated_at, '%Y-%m-%d %H:%i:%s') AS updated_at";

type NoteRow = (
    u64,
    String,
    Option<u64>,
    Option<u64>,
    Option<u64>,
    Option<u64>,
    Option<String>,
    Option<String>,
);

fn map_note(row: NoteRow) -> Note {
    let (id, content, contact_id, deal_id, company_id, created_by, created_at, updated_at) = row;
    Note {
        id,
        content,
        contact_id,
        deal_id,
        company_id,
        created_by,
        created_at,
        updated_at,
    }
}

pub async fn list_notes(State(state): State<AppState>) -> Result<ApiResponse<Vec<Note>>, AppError> {
    let mut conn = state
        .pool
        .get_conn()
        .map_err(map_mysql_err)?;

    let notes = conn
        .query_map(
            format!("SELECT {NOTE_COLUMNS} FROM notes n ORDER BY n.id DESC"),
            map_note,
        )
        .map_err(map_mysql_err)?;

    Ok(ApiResponse::success(notes))
}

pub async fn create_note(
    State(state): State<AppState>,
    Json(payload): Json<CreateNoteDto>,
) -> Result<(StatusCode, ApiResponse<Note>), AppError> {
    if payload.content.trim().is_empty() {
        return Err(AppError::Validation("content is required".into()));
    }

    let mut conn = state
        .pool
        .get_conn()
        .map_err(map_mysql_err)?;

    if let Some(contact_id) = payload.contact_id {
        validate_contact(&mut conn, contact_id, "contact_id")?;
    }
    if let Some(deal_id) = payload.deal_id {
        validate_deal(&mut conn, deal_id, "deal_id")?;
    }
    if let Some(company_id) = payload.company_id {
        validate_company(&mut conn, company_id, "company_id")?;
    }

    conn.exec_drop(
        "INSERT INTO notes (content, contact_id, deal_id, company_id) \
         VALUES (:content, :contact_id, :deal_id, :company_id)",
        params! {
            "content" => payload.content.trim(),
            "contact_id" => payload.contact_id,
            "deal_id" => payload.deal_id,
            "company_id" => payload.company_id,
        },
    )
    .map_err(map_mysql_err)?;

    let last_id = conn.last_insert_id();
    let note = Note {
        id: last_id,
        content: payload.content,
        contact_id: payload.contact_id,
        deal_id: payload.deal_id,
        company_id: payload.company_id,
        created_by: None,
        created_at: None,
        updated_at: None,
    };

    state
        .broadcaster
        .notify("note", ChangeAction::Created, Some(last_id));

    Ok((StatusCode::CREATED, ApiResponse::success(note)))
}

pub async fn get_note(
    Path(id): Path<u64>,
    State(state): State<AppState>,
) -> Result<ApiResponse<Note>, AppError> {
    let mut conn = state
        .pool
        .get_conn()
        .map_err(map_mysql_err)?;

    let note: Option<Note> = conn
        .exec_first(
            format!("SELECT {NOTE_COLUMNS} FROM notes n WHERE n.id = :id"),
            params! { "id" => id },
        )
        .map_err(map_mysql_err)?
        .map(map_note);

    match note {
        Some(n) => Ok(ApiResponse::success(n)),
        None => Err(AppError::NotFound),
    }
}

pub async fn update_note(
    Path(id): Path<u64>,
    State(state): State<AppState>,
    Json(payload): Json<UpdateNoteDto>,
) -> Result<ApiResponse<Note>, AppError> {
    let mut conn = state
        .pool
        .get_conn()
        .map_err(map_mysql_err)?;

    let existing: Option<Note> = conn
        .exec_first(
            format!("SELECT {NOTE_COLUMNS} FROM notes n WHERE n.id = :id"),
            params! { "id" => id },
        )
        .map_err(map_mysql_err)?
        .map(map_note);

    let Some(mut note) = existing else {
        return Err(AppError::NotFound);
    };

    if let Some(content) = payload.content {
        if content.trim().is_empty() {
            return Err(AppError::Validation("content is required".into()));
        }
        note.content = content;
    }

    conn.exec_drop(
        "UPDATE notes SET content = :content WHERE id = :id",
        params! { "id" => id, "content" => &note.content },
    )
    .map_err(map_mysql_err)?;

    state
        .broadcaster
        .notify("note", ChangeAction::Updated, Some(id));

    Ok(ApiResponse::success(note))
}

pub async fn delete_note(
    Path(id): Path<u64>,
    State(state): State<AppState>,
) -> Result<StatusCode, AppError> {
    let mut conn = state
        .pool
        .get_conn()
        .map_err(map_mysql_err)?;

    conn.exec_drop("DELETE FROM notes WHERE id = :id", params! { "id" => id })
        .map_err(map_mysql_err)?;

    if conn.affected_rows() > 0 {
        state
            .broadcaster
            .notify("note", ChangeAction::Deleted, Some(id));
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(AppError::NotFound)
    }
}
