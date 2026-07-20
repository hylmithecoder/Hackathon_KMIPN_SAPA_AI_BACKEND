use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct CreateCampaignDto {
    pub name: String,
    pub campaign_type: String,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub budget: Option<f64>,
    pub currency: Option<String>,
    pub target_audience: Option<String>,
    pub message_template: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateCampaignDto {
    pub name: Option<String>,
    pub campaign_type: Option<String>,
    pub status: Option<String>,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub budget: Option<f64>,
    pub currency: Option<String>,
    pub target_audience: Option<String>,
    pub message_template: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CampaignStatusDto {
    pub status: String,
}
