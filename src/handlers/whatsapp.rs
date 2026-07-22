use crate::database::scheme::{WhatsappMessage, WhatsappSession};
use crate::error::AppError;
use crate::models::whatsapp::SendWhatsappDto;
use crate::response::ApiResponse;
use crate::state::AppState;
use crate::utils::db::map_mysql_err;
use crate::ws::event::ChangeAction;
use axum::{Json, extract::State, http::StatusCode};
use mysql::params;
use mysql::prelude::*;

type WhatsappSessionRow = (
    u64,
    String,
    Option<String>,
    String,
    Option<String>,
    Option<String>,
    Option<String>,
);
type WhatsappMessageRow = (
    u64,
    u64,
    String,
    String,
    Option<String>,
    String,
    Option<String>,
    Option<String>,
    Option<String>,
);

pub async fn get_status(
    State(state): State<AppState>,
) -> Result<ApiResponse<WhatsappSession>, AppError> {
    let mut conn = state
        .pool
        .get_conn()
        .map_err(map_mysql_err)?;

    let session: Option<WhatsappSession> = conn
        .exec_first(
            "SELECT id, name, sender_number, wa_status, wa_qr, \
             DATE_FORMAT(wa_paired_at, '%Y-%m-%d %H:%i:%s') AS wa_paired_at, \
             DATE_FORMAT(updated_at, '%Y-%m-%d %H:%i:%s') AS updated_at \
             FROM whatsapp_sessions ORDER BY id LIMIT 1",
            (),
        )
        .map_err(map_mysql_err)?
        .map(
            |(id, name, sender_number, wa_status, wa_qr, wa_paired_at, updated_at): WhatsappSessionRow| WhatsappSession {
                id,
                name,
                sender_number,
                wa_status,
                wa_qr,
                wa_paired_at,
                updated_at,
            },
        );

    match session {
        Some(s) => Ok(ApiResponse::success(s)),
        None => Err(AppError::NotFound),
    }
}

pub async fn wa_qr(State(state): State<AppState>) -> Result<ApiResponse<Option<String>>, AppError> {
    let qr = state.wa.foundation().qr_code().await;
    Ok(ApiResponse::success(qr))
}

pub async fn wa_connect(State(state): State<AppState>) -> Result<ApiResponse<()>, AppError> {
    let session = state.wa.foundation();
    match session.connect().await {
        Ok(_) => {
            state
                .broadcaster
                .notify("whatsapp_session", ChangeAction::Updated, None);
            Ok(ApiResponse::message("WhatsApp pairing started"))
        }
        Err(e) => Err(AppError::BadRequest(e)),
    }
}

pub async fn wa_send(
    State(state): State<AppState>,
    Json(payload): Json<SendWhatsappDto>,
) -> Result<ApiResponse<()>, AppError> {
    if payload.phone.trim().is_empty() {
        return Err(AppError::Validation("phone is required".into()));
    }
    if payload.message.trim().is_empty() {
        return Err(AppError::Validation("message is required".into()));
    }

    let mut conn = state
        .pool
        .get_conn()
        .map_err(map_mysql_err)?;

    let session_id: Option<u64> = conn
        .query_first("SELECT id FROM whatsapp_sessions ORDER BY id LIMIT 1")
        .map_err(map_mysql_err)?;

    let session_id = session_id.ok_or(AppError::NotFound)?;

    let phone = payload.phone.trim();
    let message = payload.message.trim();

    // Insert pending log row
    conn.exec_drop(
        "INSERT INTO whatsapp_messages (session_id, phone, message, status) \
         VALUES (:session_id, :phone, :message, 'pending')",
        params! {
            "session_id" => session_id,
            "phone" => phone,
            "message" => message,
        },
    )
    .map_err(map_mysql_err)?;

    let log_id = conn.last_insert_id();

    match state.wa.foundation().send_text(phone, message).await {
        Ok(wa_message_id) => {
            conn.exec_drop(
                "UPDATE whatsapp_messages SET wa_message_id = :wa_message_id, status = 'sent', sent_at = NOW() WHERE id = :id",
                params! { "wa_message_id" => wa_message_id, "id" => log_id },
            )
            .map_err(map_mysql_err)?;
            state
                .broadcaster
                .notify("whatsapp_message", ChangeAction::Created, Some(log_id));
            Ok(ApiResponse::message("Message sent"))
        }
        Err(e) => {
            conn.exec_drop(
                "UPDATE whatsapp_messages SET status = 'failed', error_message = :error WHERE id = :id",
                params! { "error" => &e, "id" => log_id },
            )
            .map_err(map_mysql_err)?;
            Err(AppError::BadRequest(e))
        }
    }
}

pub async fn wa_logout(State(state): State<AppState>) -> Result<StatusCode, AppError> {
    state.wa.foundation().logout().await;
    state
        .broadcaster
        .notify("whatsapp_session", ChangeAction::Updated, None);
    Ok(StatusCode::NO_CONTENT)
}

pub async fn list_messages(
    State(state): State<AppState>,
) -> Result<ApiResponse<Vec<crate::database::scheme::WhatsappMessage>>, AppError> {
    let mut conn = state
        .pool
        .get_conn()
        .map_err(map_mysql_err)?;

    let messages = conn
        .query_map(
            "SELECT id, session_id, phone, message, wa_message_id, status, error_message, \
             DATE_FORMAT(sent_at, '%Y-%m-%d %H:%i:%s') AS sent_at, \
             DATE_FORMAT(created_at, '%Y-%m-%d %H:%i:%s') AS created_at \
             FROM whatsapp_messages ORDER BY id DESC LIMIT 100",
            |(id, session_id, phone, message, wa_message_id, status, error_message, sent_at, created_at): WhatsappMessageRow| WhatsappMessage {
                id,
                session_id,
                phone,
                message,
                wa_message_id,
                status,
                error_message,
                sent_at,
                created_at,
            },
        )
        .map_err(map_mysql_err)?;

    Ok(ApiResponse::success(messages))
}
