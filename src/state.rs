//! Shared application state.

use crate::whatsapp::WaRegistry;
use crate::ws::Broadcaster;
use mysql::Pool;

/// State shared across all requests.
#[derive(Clone)]
pub struct AppState {
    pub pool: Pool,
    pub wa: WaRegistry,
    pub broadcaster: Broadcaster,
}

impl AppState {
    /// Create the initial state from a database pool.
    pub fn new(pool: Pool) -> Self {
        Self {
            wa: WaRegistry::new(pool.clone()),
            broadcaster: Broadcaster::new(),
            pool,
        }
    }
}
