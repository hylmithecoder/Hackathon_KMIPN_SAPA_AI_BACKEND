use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct CreatePriceBookDto {
    pub name: String,
    pub currency: Option<String>,
    pub description: Option<String>,
    pub is_default: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct UpdatePriceBookDto {
    pub name: Option<String>,
    pub currency: Option<String>,
    pub description: Option<String>,
    pub is_default: Option<bool>,
    pub is_active: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct CreatePriceBookItemDto {
    pub product_id: u64,
    pub min_quantity: Option<f64>,
    pub unit_price: f64,
}

#[derive(Debug, Deserialize)]
pub struct UpdatePriceBookItemDto {
    pub min_quantity: Option<f64>,
    pub unit_price: Option<f64>,
}

#[derive(Debug, Deserialize)]
pub struct PriceResolutionQuery {
    pub quantity: Option<f64>,
}
