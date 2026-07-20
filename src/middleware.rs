//! Reusable middleware.

use axum::{
    body::Body,
    extract::Request,
    http::{HeaderName, header::HeaderValue},
    middleware::Next,
    response::Response,
};
use std::time::Instant;
use uuid::Uuid;

/// Header used to expose the per-request correlation id to clients.
const REQUEST_ID_HEADER: &str = "x-request-id";

/// Attach a unique request id to every incoming request.
///
/// The id is stored as a response header so clients can quote it in support tickets.
pub async fn request_id(req: Request<Body>, next: Next) -> Response {
    let request_id = req
        .headers()
        .get(REQUEST_ID_HEADER)
        .and_then(|value| value.to_str().ok())
        .map(|s| s.to_string())
        .unwrap_or_else(|| Uuid::new_v4().to_string());

    let mut response = next.run(req).await;

    if let Ok(value) = HeaderValue::from_str(&request_id) {
        response
            .headers_mut()
            .insert(HeaderName::from_static(REQUEST_ID_HEADER), value);
    }

    response
}

/// Verbose access log of every request and response.
pub async fn access_log(req: Request<Body>, next: Next) -> Response {
    let method = req.method().clone();
    let uri = req.uri().clone();
    let start = Instant::now();

    let response = next.run(req).await;
    let status = response.status();
    let duration = start.elapsed();

    crate::log_info!(
        "{} {} -> {} in {:?}",
        method,
        uri.path(),
        status.as_u16(),
        duration
    );

    response
}
