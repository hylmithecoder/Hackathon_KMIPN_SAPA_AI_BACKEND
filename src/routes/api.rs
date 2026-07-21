//! API v1 routes.

use axum::{
    Router,
    response::IntoResponse,
    routing::{get, post, put},
};
use std::time::Duration;
use tower::{ServiceBuilder, timeout::TimeoutLayer};
use tower_http::cors::{Any, CorsLayer};

use crate::{
    handlers::{
        activity, auth, campaign, company, contact, deal, deal_stage, health, note, notification,
        product, quote, tag, ticket, whatsapp,
    },
    middleware,
    state::AppState,
    ws,
};

async fn fallback(
    uri: axum::http::Uri,
    method: axum::http::Method,
) -> (axum::http::StatusCode, axum::Json<serde_json::Value>) {
    (
        axum::http::StatusCode::NOT_FOUND,
        axum::Json(serde_json::json!({
            "success": false,
            "message": format!("{} {} not found", method, uri.path())
        })),
    )
}

async fn method_not_allowed_layer(
    req: axum::http::Request<axum::body::Body>,
    next: axum::middleware::Next,
) -> axum::response::Response {
    let response = next.run(req).await;
    if response.status() == axum::http::StatusCode::METHOD_NOT_ALLOWED {
        (
            axum::http::StatusCode::METHOD_NOT_ALLOWED,
            axum::Json(serde_json::json!({
                "success": false,
                "message": "Method not allowed"
            })),
        )
            .into_response()
    } else {
        response
    }
}

async fn handle_middleware_error(err: axum::BoxError) -> impl axum::response::IntoResponse {
    (
        axum::http::StatusCode::INTERNAL_SERVER_ERROR,
        axum::Json(serde_json::json!({
            "success": false,
            "message": format!("Unhandled error: {err}")
        })),
    )
}

/// Router for the `/api/v1` namespace.
pub fn router() -> Router<AppState> {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let api = Router::new()
        // Health (also exposed at /api/v1/health for convenience)
        .route("/api/v1/health", get(health::health_check))
        .route("/api/v1/health/ready", get(health::readiness))
        // Real-time WebSocket endpoint
        .route("/api/v1/ws", get(ws::ws_handler))
        // Auth
        .route("/api/v1/auth/login", post(auth::login))
        .route("/api/v1/auth/register", post(auth::register))
        .route("/api/v1/auth/logout", post(auth::logout))
        .route("/api/v1/users", get(auth::list_users))
        .route(
            "/api/v1/users/{id}",
            put(auth::update_user).delete(auth::delete_user),
        )
        // Companies
        .route(
            "/api/v1/companies",
            get(company::list_companies).post(company::create_company),
        )
        .route(
            "/api/v1/companies/{id}",
            get(company::get_company)
                .put(company::update_company)
                .delete(company::delete_company),
        )
        // Contacts
        .route(
            "/api/v1/contacts",
            get(contact::list_contacts).post(contact::create_contact),
        )
        .route(
            "/api/v1/contacts/{id}",
            get(contact::get_contact)
                .put(contact::update_contact)
                .delete(contact::delete_contact),
        )
        .route(
            "/api/v1/contacts/{id}/tags",
            get(contact::list_contact_tags).post(contact::add_tag_to_contact),
        )
        .route(
            "/api/v1/contacts/{id}/tags/{tag_id}",
            axum::routing::delete(contact::remove_tag_from_contact),
        )
        // Deal stages
        .route(
            "/api/v1/deal-stages",
            get(deal_stage::list_stages).post(deal_stage::create_stage),
        )
        .route(
            "/api/v1/deal-stages/{id}",
            get(deal_stage::get_stage)
                .put(deal_stage::update_stage)
                .delete(deal_stage::delete_stage),
        )
        .route(
            "/api/v1/deal-stages/reorder",
            put(deal_stage::reorder_stages),
        )
        // Deals
        .route(
            "/api/v1/deals",
            get(deal::list_deals).post(deal::create_deal),
        )
        .route(
            "/api/v1/deals/{id}",
            get(deal::get_deal)
                .put(deal::update_deal)
                .delete(deal::delete_deal),
        )
        .route("/api/v1/deals/{id}/move-stage", put(deal::move_deal_stage))
        // Activities
        .route(
            "/api/v1/activities",
            get(activity::list_activities).post(activity::create_activity),
        )
        .route(
            "/api/v1/activities/{id}",
            get(activity::get_activity)
                .put(activity::update_activity)
                .delete(activity::delete_activity),
        )
        .route(
            "/api/v1/activities/{id}/done",
            put(activity::mark_activity_done),
        )
        // Notes
        .route(
            "/api/v1/notes",
            get(note::list_notes).post(note::create_note),
        )
        .route(
            "/api/v1/notes/{id}",
            get(note::get_note)
                .put(note::update_note)
                .delete(note::delete_note),
        )
        // Products
        .route(
            "/api/v1/products",
            get(product::list_products).post(product::create_product),
        )
        .route(
            "/api/v1/products/{id}",
            get(product::get_product)
                .put(product::update_product)
                .delete(product::delete_product),
        )
        // Quotes
        .route(
            "/api/v1/quotes",
            get(quote::list_quotes).post(quote::create_quote),
        )
        .route(
            "/api/v1/quotes/{id}",
            get(quote::get_quote)
                .put(quote::update_quote)
                .delete(quote::delete_quote),
        )
        .route(
            "/api/v1/quotes/{id}/status",
            put(quote::update_quote_status),
        )
        .route("/api/v1/quotes/{id}/items", get(quote::list_quote_items))
        // Tickets
        .route(
            "/api/v1/tickets",
            get(ticket::list_tickets).post(ticket::create_ticket),
        )
        .route(
            "/api/v1/tickets/{id}",
            get(ticket::get_ticket)
                .put(ticket::update_ticket)
                .delete(ticket::delete_ticket),
        )
        .route(
            "/api/v1/tickets/{id}/status",
            put(ticket::update_ticket_status),
        )
        // Campaigns
        .route(
            "/api/v1/campaigns",
            get(campaign::list_campaigns).post(campaign::create_campaign),
        )
        .route(
            "/api/v1/campaigns/{id}",
            get(campaign::get_campaign)
                .put(campaign::update_campaign)
                .delete(campaign::delete_campaign),
        )
        .route(
            "/api/v1/campaigns/{id}/status",
            put(campaign::update_campaign_status),
        )
        // Tags
        .route("/api/v1/tags", get(tag::list_tags).post(tag::create_tag))
        .route(
            "/api/v1/tags/{id}",
            get(tag::get_tag)
                .put(tag::update_tag)
                .delete(tag::delete_tag),
        )
        // Notifications
        .route(
            "/api/v1/notifications",
            get(notification::list_notifications).post(notification::create_notification),
        )
        .route(
            "/api/v1/notifications/unread-count",
            get(notification::unread_count),
        )
        .route(
            "/api/v1/notifications/read-all",
            put(notification::mark_all_read),
        )
        .route(
            "/api/v1/notifications/{id}/read",
            put(notification::mark_read),
        )
        .route(
            "/api/v1/notifications/{id}",
            axum::routing::delete(notification::delete_notification),
        )
        // WhatsApp
        .route("/api/v1/whatsapp/status", get(whatsapp::get_status))
        .route("/api/v1/whatsapp/qr", get(whatsapp::wa_qr))
        .route("/api/v1/whatsapp/connect", post(whatsapp::wa_connect))
        .route("/api/v1/whatsapp/send", post(whatsapp::wa_send))
        .route("/api/v1/whatsapp/logout", post(whatsapp::wa_logout))
        .route("/api/v1/whatsapp/messages", get(whatsapp::list_messages))
        .layer(
            ServiceBuilder::new()
                .layer(axum::error_handling::HandleErrorLayer::new(
                    handle_middleware_error,
                ))
                .layer(TimeoutLayer::new(Duration::from_secs(30))),
        );

    Router::new()
        .merge(api)
        .fallback_service(axum::routing::any(fallback))
        .layer(axum::middleware::from_fn(method_not_allowed_layer))
        .layer(cors)
        .layer(axum::middleware::from_fn(middleware::request_id))
        .layer(axum::middleware::from_fn(middleware::access_log))
}
