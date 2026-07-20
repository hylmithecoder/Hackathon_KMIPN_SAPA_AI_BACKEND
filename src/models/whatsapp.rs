use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct SendWhatsappDto {
    pub phone: String,
    pub message: String,
}
