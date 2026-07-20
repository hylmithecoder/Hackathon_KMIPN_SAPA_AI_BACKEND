use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct CreateNoteDto {
    pub content: String,
    pub contact_id: Option<u64>,
    pub deal_id: Option<u64>,
    pub company_id: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateNoteDto {
    pub content: Option<String>,
}
