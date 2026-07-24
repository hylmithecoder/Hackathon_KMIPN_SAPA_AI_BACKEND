use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct SendWhatsappDto {
    pub phone: String,
    pub message: String,
}

#[derive(Debug, Deserialize)]
pub struct SendDealWhatsappDto {
    pub message: String,
}
