use crate::database::scheme::Company;
use crate::error::AppError;
use crate::models::company::{CreateCompanyDto, UpdateCompanyDto};
use crate::response::ApiResponse;
use crate::state::AppState;
use crate::ws::event::ChangeAction;
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use mysql::params;
use mysql::prelude::*;

const COMPANY_COLUMNS: &str = "c.id, c.name, c.industry, c.website, c.phone, c.email, \
    c.address, c.city, c.country, c.description, c.assigned_to, c.created_at, c.updated_at";

fn row_to_company(row: &mut mysql::Row) -> Company {
    Company {
        id: row.take("id").unwrap_or_default(),
        name: row.take("name").unwrap_or_default(),
        industry: row.take("industry"),
        website: row.take("website"),
        phone: row.take("phone"),
        email: row.take("email"),
        address: row.take("address"),
        city: row.take("city"),
        country: row.take("country"),
        description: row.take("description"),
        assigned_to: row.take("assigned_to"),
        created_at: row.take("created_at"),
        updated_at: row.take("updated_at"),
    }
}

pub async fn list_companies(
    State(state): State<AppState>,
) -> Result<ApiResponse<Vec<Company>>, AppError> {
    let mut conn = state
        .pool
        .get_conn()
        .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

    let companies: Vec<Company> = conn
        .query_map(
            format!("SELECT {COMPANY_COLUMNS} FROM companies c ORDER BY c.id DESC"),
            |mut row: mysql::Row| row_to_company(&mut row),
        )
        .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

    Ok(ApiResponse::success(companies))
}

pub async fn create_company(
    State(state): State<AppState>,
    Json(payload): Json<CreateCompanyDto>,
) -> Result<(StatusCode, ApiResponse<Company>), AppError> {
    if payload.name.trim().is_empty() {
        return Err(AppError::Validation("name is required".into()));
    }

    let mut conn = state
        .pool
        .get_conn()
        .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

    conn.exec_drop(
        "INSERT INTO companies (name, industry, website, phone, email, address, city, country, description, assigned_to) \
         VALUES (:name, :industry, :website, :phone, :email, :address, :city, :country, :description, :assigned_to)",
        params! {
            "name" => payload.name.trim(),
            "industry" => payload.industry.as_deref(),
            "website" => payload.website.as_deref(),
            "phone" => payload.phone.as_deref(),
            "email" => payload.email.as_deref(),
            "address" => payload.address.as_deref(),
            "city" => payload.city.as_deref(),
            "country" => payload.country.as_deref(),
            "description" => payload.description.as_deref(),
            "assigned_to" => payload.assigned_to,
        },
    )
    .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

    let last_id = conn.last_insert_id();
    let company = Company {
        id: last_id,
        name: payload.name,
        industry: payload.industry,
        website: payload.website,
        phone: payload.phone,
        email: payload.email,
        address: payload.address,
        city: payload.city,
        country: payload.country,
        description: payload.description,
        assigned_to: payload.assigned_to,
        created_at: None,
        updated_at: None,
    };

    state
        .broadcaster
        .notify("company", ChangeAction::Created, Some(last_id));

    Ok((StatusCode::CREATED, ApiResponse::success(company)))
}

pub async fn get_company(
    Path(id): Path<u64>,
    State(state): State<AppState>,
) -> Result<ApiResponse<Company>, AppError> {
    let mut conn = state
        .pool
        .get_conn()
        .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

    let company: Option<Company> = conn
        .exec_first(
            format!("SELECT {COMPANY_COLUMNS} FROM companies c WHERE c.id = :id"),
            params! { "id" => id },
        )
        .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?
        .map(|mut row: mysql::Row| row_to_company(&mut row));

    match company {
        Some(c) => Ok(ApiResponse::success(c)),
        None => Err(AppError::NotFound),
    }
}

pub async fn update_company(
    Path(id): Path<u64>,
    State(state): State<AppState>,
    Json(payload): Json<UpdateCompanyDto>,
) -> Result<ApiResponse<Company>, AppError> {
    let mut conn = state
        .pool
        .get_conn()
        .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

    let existing: Option<Company> = conn
        .exec_first(
            format!("SELECT {COMPANY_COLUMNS} FROM companies c WHERE c.id = :id"),
            params! { "id" => id },
        )
        .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?
        .map(|mut row: mysql::Row| row_to_company(&mut row));

    let Some(mut company) = existing else {
        return Err(AppError::NotFound);
    };

    if let Some(name) = payload.name {
        if name.trim().is_empty() {
            return Err(AppError::Validation("name is required".into()));
        }
        company.name = name;
    }
    if payload.industry.is_some() {
        company.industry = payload.industry;
    }
    if payload.website.is_some() {
        company.website = payload.website;
    }
    if payload.phone.is_some() {
        company.phone = payload.phone;
    }
    if payload.email.is_some() {
        company.email = payload.email;
    }
    if payload.address.is_some() {
        company.address = payload.address;
    }
    if payload.city.is_some() {
        company.city = payload.city;
    }
    if payload.country.is_some() {
        company.country = payload.country;
    }
    if payload.description.is_some() {
        company.description = payload.description;
    }
    if payload.assigned_to.is_some() {
        company.assigned_to = payload.assigned_to;
    }

    conn.exec_drop(
        "UPDATE companies SET name = :name, industry = :industry, website = :website, phone = :phone, \
         email = :email, address = :address, city = :city, country = :country, description = :description, \
         assigned_to = :assigned_to WHERE id = :id",
        params! {
            "id" => id,
            "name" => &company.name,
            "industry" => &company.industry,
            "website" => &company.website,
            "phone" => &company.phone,
            "email" => &company.email,
            "address" => &company.address,
            "city" => &company.city,
            "country" => &company.country,
            "description" => &company.description,
            "assigned_to" => company.assigned_to,
        },
    )
    .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

    state
        .broadcaster
        .notify("company", ChangeAction::Updated, Some(id));

    Ok(ApiResponse::success(company))
}

pub async fn delete_company(
    Path(id): Path<u64>,
    State(state): State<AppState>,
) -> Result<StatusCode, AppError> {
    let mut conn = state
        .pool
        .get_conn()
        .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

    conn.exec_drop(
        "DELETE FROM companies WHERE id = :id",
        params! { "id" => id },
    )
    .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

    if conn.affected_rows() > 0 {
        state
            .broadcaster
            .notify("company", ChangeAction::Deleted, Some(id));
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(AppError::NotFound)
    }
}
