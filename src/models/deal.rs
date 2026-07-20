use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct CreateDealDto {
    pub title: String,
    pub contact_id: u64,
    pub company_id: Option<u64>,
    pub stage_id: u64,
    pub owner_id: Option<u64>,
    pub value: Option<f64>,
    pub currency: Option<String>,
    pub expected_close_date: Option<String>,
    pub status: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateDealDto {
    pub title: Option<String>,
    pub contact_id: Option<u64>,
    pub company_id: Option<u64>,
    pub stage_id: Option<u64>,
    pub owner_id: Option<u64>,
    pub value: Option<f64>,
    pub currency: Option<String>,
    pub expected_close_date: Option<String>,
    pub actual_close_date: Option<String>,
    pub status: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct DealStageMoveDto {
    pub stage_id: u64,
}
