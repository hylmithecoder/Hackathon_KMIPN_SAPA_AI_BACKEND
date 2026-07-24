//! Server lifecycle management.

use crate::log_info;
use crate::{config, routes::create_router, state::AppState};
use anyhow::Result;
use axum::serve;
use tokio::net::TcpListener;
use tokio::signal::unix::{self, SignalKind};

/// Start the HTTP server and block until shutdown.
pub async fn run(pool: mysql::Pool) -> Result<()> {
    let state = AppState::new(pool);
    let app = create_router(state);

    let bind_addr = config::bind_address();
    let listener = TcpListener::bind(&bind_addr).await?;

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

    let ctrl_r = async {
        #[cfg(unix)]
        let sighup = async {
            if let Ok(mut sig) = unix::signal(SignalKind::hangup()) {
                sig.recv().await;
            } else {
                std::future::pending::<()>().await;
            }
        };
        #[cfg(not(unix))]
        let sighup = std::future::pending::<()>();

        let stdin_ctrl_r = async {
            use tokio::io::AsyncReadExt;
            let mut stdin = tokio::io::stdin();
            let mut buf = [0u8; 1];
            loop {
                match stdin.read(&mut buf).await {
                    Ok(n) if n > 0 => {
                        // 0x12 is ASCII byte for Ctrl+R; also accept 'r' / 'R'
                        if buf[0] == 0x12 || buf[0] == b'r' || buf[0] == b'R' {
                            break;
                        }
                    }
                    _ => std::future::pending::<()>().await,
                }
            }
        };

        tokio::select! {
            _ = sighup => {},
            _ = stdin_ctrl_r => {},
        }
    };

    tokio::select! {
        _ = ctrl_r => reload_signal().await,
        _ = ctrl_c => log_info!("received Ctrl+C, shutting down"),
        _ = terminate => log_info!("received SIGTERM, shutting down"),
    }
}

async fn reload_signal() {
    log_info!("received Ctrl+R, reloading configuration...");
    config::init();
}


