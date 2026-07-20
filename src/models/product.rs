use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct CreateProductDto {
    pub name: String,
    pub sku: Option<String>,
    pub description: Option<String>,
    pub category: Option<String>,
    pub unit_price: f64,
    pub currency: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateProductDto {
    pub name: Option<String>,
    pub sku: Option<String>,
    pub description: Option<String>,
    pub category: Option<String>,
    pub unit_price: Option<f64>,
    pub currency: Option<String>,
    pub is_active: Option<bool>,
}
