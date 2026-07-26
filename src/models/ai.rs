use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct DraftQuoteAiDto {
    pub deal_id: u64,
    pub quote_id: Option<u64>,
    pub template_id: Option<u64>,
    pub instruction: Option<String>,
    pub language: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct DraftNoteAiDto {
    pub instruction: Option<String>,
    pub existing_content: Option<String>,
    pub contact_id: Option<u64>,
    pub deal_id: Option<u64>,
    pub company_id: Option<u64>,
    pub language: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SummarizeDealAiDto {
    pub deal_id: u64,
    pub instruction: Option<String>,
    pub language: Option<String>,
}
