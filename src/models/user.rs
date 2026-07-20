//! User domain model and API DTOs.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// User returned by the API.
#[derive(Debug, Clone, Serialize)]
pub struct User {
    pub id: String,
    pub name: String,
    pub created_at: DateTime<Utc>,
}

impl User {
    /// Convenience constructor for examples and tests.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            name: name.into(),
            created_at: Utc::now(),
        }
    }
}

/// Request body for creating a user.
#[derive(Debug, Deserialize)]
pub struct CreateUser {
    pub name: String,
}
