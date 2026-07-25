use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct DraftQuoteAiDto {
    pub deal_id: u64,
    pub quote_id: Option<u64>,
    pub template_id: Option<u64>,
    pub instruction: Option<String>,
    pub language: Option<String>,
}
