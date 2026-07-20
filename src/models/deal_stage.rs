use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct CreateDealStageDto {
    pub name: String,
    pub position: i32,
    pub probability: Option<f64>,
    pub color: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateDealStageDto {
    pub name: Option<String>,
    pub position: Option<i32>,
    pub probability: Option<f64>,
    pub color: Option<String>,
    pub is_active: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct ReorderDealStageDto {
    pub ordered_ids: Vec<u64>,
}
