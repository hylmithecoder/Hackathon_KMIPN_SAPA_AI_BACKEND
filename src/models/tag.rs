use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct CreateTagDto {
    pub name: String,
    pub color: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateTagDto {
    pub name: Option<String>,
    pub color: Option<String>,
}
