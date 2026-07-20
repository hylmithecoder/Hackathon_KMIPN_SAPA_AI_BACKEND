use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct CreateContactDto {
    pub first_name: String,
    pub last_name: Option<String>,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub job_title: Option<String>,
    pub company_id: Option<u64>,
    pub source: Option<String>,
    pub status: Option<String>,
    pub assigned_to: Option<u64>,
    pub description: Option<String>,
    pub tag_ids: Option<Vec<u64>>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateContactDto {
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub job_title: Option<String>,
    pub company_id: Option<u64>,
    pub source: Option<String>,
    pub status: Option<String>,
    pub assigned_to: Option<u64>,
    pub description: Option<String>,
    pub tag_ids: Option<Vec<u64>>,
}

#[derive(Debug, Deserialize)]
pub struct ContactQuery {
    pub search: Option<String>,
    pub status: Option<String>,
    pub company_id: Option<u64>,
    pub assigned_to: Option<u64>,
    pub tag_id: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub struct TagContactDto {
    pub tag_id: u64,
}
