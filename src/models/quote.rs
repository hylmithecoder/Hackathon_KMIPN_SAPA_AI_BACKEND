use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct CreateQuoteDto {
    pub deal_id: u64,
    pub quote_number: String,
    pub issue_date: String,
    pub expiry_date: Option<String>,
    pub tax_rate: Option<f64>,
    pub currency: Option<String>,
    pub notes: Option<String>,
    pub items: Vec<CreateQuoteItemDto>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateQuoteDto {
    pub quote_number: Option<String>,
    pub issue_date: Option<String>,
    pub expiry_date: Option<String>,
    pub tax_rate: Option<f64>,
    pub currency: Option<String>,
    pub status: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateQuoteItemDto {
    pub product_id: Option<u64>,
    pub description: String,
    pub quantity: f64,
    pub unit_price: f64,
    pub discount: Option<f64>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateQuoteItemDto {
    pub product_id: Option<u64>,
    pub description: Option<String>,
    pub quantity: Option<f64>,
    pub unit_price: Option<f64>,
    pub discount: Option<f64>,
}

#[derive(Debug, Deserialize)]
pub struct QuoteStatusDto {
    pub status: String,
}
