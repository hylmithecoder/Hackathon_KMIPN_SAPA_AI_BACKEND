use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct CreateActivityDto {
    pub activity_type: String,
    pub subject: String,
    pub description: Option<String>,
    pub contact_id: Option<u64>,
    pub deal_id: Option<u64>,
    pub company_id: Option<u64>,
    pub assigned_to: Option<u64>,
    pub due_date: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateActivityDto {
    pub activity_type: Option<String>,
    pub subject: Option<String>,
    pub description: Option<String>,
    pub contact_id: Option<u64>,
    pub deal_id: Option<u64>,
    pub company_id: Option<u64>,
    pub assigned_to: Option<u64>,
    pub due_date: Option<String>,
    pub status: Option<String>,
    pub completed_at: Option<String>,
}
