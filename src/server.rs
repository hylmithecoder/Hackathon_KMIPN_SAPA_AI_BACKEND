//! Server lifecycle management.

use crate::log_info;
use crate::{config, routes::create_router, state::AppState};
use anyhow::{Context, Result};
use axum::serve;
use tokio::net::TcpListener;
use tokio::signal::unix::{self, SignalKind};

/// Start the HTTP server and block until shutdown.
pub async fn run(pool: mysql::Pool) -> Result<()> {
    let bind_addr = config::bind_address();
    let listener = TcpListener::bind(&bind_addr).await.with_context(|| {
        format!(
            "cannot bind API server to {bind_addr}; another backend instance may already be running"
        )
    })?;

    // Build one shared state for both HTTP handlers and the background
    // WhatsApp restore task. Binding first avoids opening/restoring a WA
    // session when startup cannot claim the configured HTTP port.
    let state = AppState::new(pool);
    let wa = state.wa.clone();
    let app = create_router(state);
    tokio::spawn(async move {
        wa.restore_all().await;
    });

    log_info!("{} listening on http://{}", config::app_name(), bind_addr);

    serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    log_info!("server stopped");
    Ok(())
}

/// Wait for a shutdown signal or reload request.
async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        unix::signal(SignalKind::terminate())
            .expect("failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => log_info!("received Ctrl+C, shutting down"),
        _ = terminate => log_info!("received SIGTERM, shutting down"),
    }
}
