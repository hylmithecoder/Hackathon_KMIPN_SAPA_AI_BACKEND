use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct CreateCompanyDto {
    pub name: String,
    pub industry: Option<String>,
    pub website: Option<String>,
    pub phone: Option<String>,
    pub email: Option<String>,
    pub address: Option<String>,
    pub city: Option<String>,
    pub country: Option<String>,
    pub description: Option<String>,
    pub assigned_to: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateCompanyDto {
    pub name: Option<String>,
    pub industry: Option<String>,
    pub website: Option<String>,
    pub phone: Option<String>,
    pub email: Option<String>,
    pub address: Option<String>,
    pub city: Option<String>,
    pub country: Option<String>,
    pub description: Option<String>,
    pub assigned_to: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub struct CompanyQuery {
    pub search: Option<String>,
    pub assigned_to: Option<u64>,
}
