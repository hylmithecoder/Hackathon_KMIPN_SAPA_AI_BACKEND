use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct CreateNotificationDto {
    pub user_id: u64,
    pub title: String,
    pub body: String,
    pub category: Option<String>,
    pub entity_type: Option<String>,
    pub entity_id: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateNotificationDto {
    pub is_read: bool,
}
