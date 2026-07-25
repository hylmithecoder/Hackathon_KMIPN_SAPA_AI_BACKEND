use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
pub struct QuoteTemplateItemDto {
    pub product_id: Option<u64>,
    pub description: String,
    pub quantity: f64,
    pub unit_price: f64,
    pub discount: Option<f64>,
}

#[derive(Debug, Deserialize)]
pub struct CreateQuoteTemplateDto {
    pub name: String,
    pub description: Option<String>,
    pub currency: Option<String>,
    pub tax_rate: Option<f64>,
    pub notes: Option<String>,
    pub terms: Option<String>,
    pub items: Vec<QuoteTemplateItemDto>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateQuoteTemplateDto {
    pub name: Option<String>,
    pub description: Option<String>,
    pub currency: Option<String>,
    pub tax_rate: Option<f64>,
    pub notes: Option<String>,
    pub terms: Option<String>,
    pub is_active: Option<bool>,
    pub items: Option<Vec<QuoteTemplateItemDto>>,
}

#[derive(Debug, Deserialize)]
pub struct InstantiateQuoteTemplateDto {
    pub deal_id: u64,
    pub quote_number: Option<String>,
    pub issue_date: Option<String>,
    pub expiry_date: Option<String>,
    pub notes: Option<String>,
}
