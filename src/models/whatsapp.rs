use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct SendWhatsappDto {
    pub phone: String,
    pub message: String,
    pub media_url: Option<String>,
    pub media_filename: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SendDealWhatsappDto {
    pub phone: Option<String>,
    pub message: String,
    pub media_url: Option<String>,
    pub media_filename: Option<String>,
}
