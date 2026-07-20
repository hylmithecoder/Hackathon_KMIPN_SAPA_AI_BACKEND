//! Binary entry point.

use anyhow::Result;
use api_sapaai::{config, database::connection::init_db, log_info, server};

fn init_logging() {
    use tracing_subscriber::EnvFilter;

    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        EnvFilter::new("info,api_sapaai=debug,whatsapp_rust=debug,wacore=debug")
    });

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(true)
        .init();
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    init_logging();

    config::init();

    log_info!("starting {}", config::app_name());

    let pool = match init_db() {
        Ok(p) => p,
        Err(e) => {
            api_sapaai::log_err!("Failed to connect to MySQL database: {:?}", e);
            std::process::exit(1);
        }
    };
    log_info!("DB connection success");

    // Restore any previously paired WhatsApp session in the background.
    let state = api_sapaai::state::AppState::new(pool.clone());
    let wa = state.wa.clone();
    tokio::spawn(async move {
        wa.restore_all().await;
    });

    server::run(pool).await
}
