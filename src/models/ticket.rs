use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct CreateTicketDto {
    pub ticket_number: String,
    pub subject: String,
    pub description: String,
    pub contact_id: Option<u64>,
    pub company_id: Option<u64>,
    pub assigned_to: Option<u64>,
    pub priority: Option<String>,
    pub source: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateTicketDto {
    pub subject: Option<String>,
    pub description: Option<String>,
    pub contact_id: Option<u64>,
    pub company_id: Option<u64>,
    pub assigned_to: Option<u64>,
    pub priority: Option<String>,
    pub source: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct TicketStatusDto {
    pub status: String,
}
