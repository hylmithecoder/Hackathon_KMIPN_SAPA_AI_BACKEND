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

/// ANSI color for the status code based on its class.
///
/// * `2xx` → green (`\x1b[32m`)
/// * `3xx` → cyan (`\x1b[36m`)
/// * `4xx` / `5xx` → red (`\x1b[31m`)
fn status_color(code: u16) -> &'static str {
    match code {
        200..=299 => "\x1b[32m",
        300..=399 => "\x1b[36m",
        _ => "\x1b[31m",
    }
}

/// Format a [`Duration`] into a human-readable string (e.g. `3ms`, `1.2s`).
fn format_duration(d: std::time::Duration) -> String {
    if d.as_secs() > 0 {
        format!("{}.{:03}s", d.as_secs(), d.subsec_millis())
    } else if d.as_micros() >= 1000 {
        format!("{}ms", d.as_millis())
    } else {
        format!("{}µs", d.as_micros())
    }
}

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

/// Access log of every request and response.
///
/// Request and response bodies remain untouched so logging cannot reject large
/// uploads, consume streaming bodies, or replace an otherwise valid response.
pub async fn access_log(req: Request<Body>, next: Next) -> Response {
    let method = req.method().clone();
    let uri = req.uri().clone();
    let path = uri.path().to_string();
    let query_str = uri.query().unwrap_or("").to_string();

    let start = Instant::now();
    let response = next.run(req).await;
    let duration = start.elapsed();
    let status = response.status();

    let (method_str, status_code, duration_fmt) =
        (method.as_str(), status.as_u16(), format_duration(duration));
    let color = status_color(status_code);

    if query_str.is_empty() {
        println!(
            "{}[{}] \x1b[0m{} {} -> {}{} \x1b[0min {}",
            color,
            crate::utils::debugger::timestamp(),
            method_str,
            path,
            color,
            status_code,
            duration_fmt,
        );
    } else {
        println!(
            "{}[{}] \x1b[0m{} {}?{} -> {}{} \x1b[0min {}",
            color,
            crate::utils::debugger::timestamp(),
            method_str,
            path,
            query_str,
            color,
            status_code,
            duration_fmt,
        );
    }

    response
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        Router,
        body::{Body, to_bytes},
        http::{Request, StatusCode},
        middleware::from_fn,
        routing::{get, post},
    };
    use tower::ServiceExt;

    #[tokio::test]
    async fn test_request_id_middleware() {
        let app = Router::new()
            .route("/test", get(|| async { "ok" }))
            .layer(from_fn(request_id));

        let response = app
            .oneshot(Request::builder().uri("/test").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert!(response.headers().contains_key(REQUEST_ID_HEADER));
    }

    #[tokio::test]
    async fn test_access_log_middleware() {
        let app = Router::new()
            .route(
                "/echo",
                post(|body: String| async move { format!("echo: {}", body) }),
            )
            .layer(from_fn(access_log));

        let req = Request::builder()
            .method("POST")
            .uri("/echo?param1=val1")
            .header("content-type", "text/plain")
            .body(Body::from("hello world"))
            .unwrap();

        let response = app.oneshot(req).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let body_bytes = to_bytes(response.into_body(), 1024)
            .await
            .unwrap();
        assert_eq!(&body_bytes[..], b"echo: hello world");
    }

    #[tokio::test]
    async fn test_access_log_preserves_large_request_and_response_bodies() {
        const LARGE_BODY_SIZE: usize = 3 * 1024 * 1024;

        let app = Router::new()
            .route(
                "/echo",
                post(|request: Request<Body>| async move {
                    to_bytes(request.into_body(), LARGE_BODY_SIZE)
                        .await
                        .unwrap()
                }),
            )
            .layer(from_fn(access_log));

        let request = Request::builder()
            .method("POST")
            .uri("/echo")
            .body(Body::from(vec![b'x'; LARGE_BODY_SIZE]))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let body = to_bytes(response.into_body(), LARGE_BODY_SIZE)
            .await
            .unwrap();
        assert_eq!(body.len(), LARGE_BODY_SIZE);
    }
}
