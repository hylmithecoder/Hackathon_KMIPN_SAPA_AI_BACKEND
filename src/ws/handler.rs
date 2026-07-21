//! WebSocket endpoint for streaming change events.

use axum::{
    Json,
    extract::{Query, State, WebSocketUpgrade},
    http::StatusCode,
    response::IntoResponse,
};
use futures::{SinkExt, StreamExt};
use mysql::params;
use mysql::prelude::*;
use serde::Deserialize;
use tokio::sync::broadcast;

use crate::state::AppState;

/// Optional subscription filter and authentication token supplied by the client.
///
/// Browser WebSocket clients cannot set custom headers, so the token must be
/// provided via the `token` query parameter. Non-browser clients may also send
/// a standard `Authorization: Bearer <token>` header.
///
/// Examples:
/// - `ws://localhost:5790/api/v1/ws?token=xxx` — receive all events.
/// - `ws://localhost:5790/api/v1/ws?token=xxx&entities=company,contact`
///   — only companies and contacts.
#[derive(Debug, Default, Deserialize)]
pub struct WsQuery {
    #[serde(default)]
    pub token: Option<String>,
    #[serde(default)]
    pub entities: Option<String>,
}

/// Upgrade an HTTP request to a WebSocket connection.
pub async fn ws_handler(
    ws: WebSocketUpgrade,
    Query(query): Query<WsQuery>,
    State(state): State<AppState>,
) -> axum::response::Response {
    let Some(token) = query.token else {
        return unauthorized("Missing token");
    };

    match validate_token(&state, &token).await {
        Ok(true) => {}
        Ok(false) => return unauthorized("Invalid or expired token"),
        Err(err) => {
            crate::log_err!("WebSocket token validation failed: {:?}", err);
            return internal_error();
        }
    }

    let filter = query.entities.map(|s| {
        s.split(',')
            .map(|part| part.trim().to_string())
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>()
    });

    ws.on_upgrade(move |socket| handle_socket(socket, state, filter))
}

/// Verify that a bearer token belongs to an active user.
async fn validate_token(state: &AppState, token: &str) -> Result<bool, mysql::Error> {
    let mut conn = state.pool.get_conn()?;
    let valid: Option<u8> = conn.exec_first(
        "SELECT 1 FROM users u \
         JOIN user_tokens t ON u.id = t.user_id \
         WHERE t.token = :token AND u.is_active = 1 \
         LIMIT 1",
        params! { "token" => token },
    )?;
    Ok(valid.is_some())
}

fn unauthorized(message: &str) -> axum::response::Response {
    (
        StatusCode::UNAUTHORIZED,
        Json(serde_json::json!({
            "success": false,
            "message": message,
        })),
    )
        .into_response()
}

fn internal_error() -> axum::response::Response {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(serde_json::json!({
            "success": false,
            "message": "Internal server error",
        })),
    )
        .into_response()
}

/// Maintain a single WebSocket connection: subscribe to the broadcast hub and
/// forward messages that match the requested entity filter.
async fn handle_socket(
    socket: axum::extract::ws::WebSocket,
    state: AppState,
    entity_filter: Option<Vec<String>>,
) {
    let mut rx = state.broadcaster.subscribe();
    let (mut sender, mut receiver) = socket.split();

    crate::log_info!("WebSocket client connected (filter: {:?})", entity_filter);

    let forward_task = tokio::spawn(async move {
        loop {
            match rx.recv().await {
                Ok(message) => {
                    if let Some(ref filter) = entity_filter {
                        if let Ok(event) = serde_json::from_str::<serde_json::Value>(&message) {
                            if let Some(serde_json::Value::String(entity)) = event.get("entity") {
                                if !filter.contains(entity) {
                                    continue;
                                }
                            }
                        }
                    }

                    if sender
                        .send(axum::extract::ws::Message::Text(message.into()))
                        .await
                        .is_err()
                    {
                        break;
                    }
                }
                Err(broadcast::error::RecvError::Closed) => break,
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
            }
        }
    });

    // Keep the connection alive by reading incoming messages. Clients may send
    // ping or subscribe messages, but we do not require any traffic.
    while let Some(Ok(msg)) = receiver.next().await {
        if matches!(msg, axum::extract::ws::Message::Close(_)) {
            break;
        }
    }

    let _ = forward_task.await;
    crate::log_info!("WebSocket client disconnected");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ws_query_parses_entities() {
        let query: WsQuery =
            serde_json::from_str(r#"{"token":"abc","entities":"company,contact"}"#).unwrap();
        assert_eq!(query.token, Some("abc".to_string()));
        assert_eq!(query.entities, Some("company,contact".to_string()));
    }

    #[test]
    fn ws_query_without_entities() {
        let query: WsQuery = serde_json::from_str(r#"{"token":"abc"}"#).unwrap();
        assert_eq!(query.token, Some("abc".to_string()));
        assert!(query.entities.is_none());
    }
}
