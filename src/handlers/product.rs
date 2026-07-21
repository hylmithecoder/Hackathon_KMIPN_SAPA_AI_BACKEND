use crate::database::scheme::Product;
use crate::error::AppError;
use crate::models::product::{CreateProductDto, UpdateProductDto};
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

const PRODUCT_COLUMNS: &str =
    "id, name, sku, description, category, unit_price, currency, is_active, created_at, updated_at";

type ProductRow = (
    u64,
    String,
    Option<String>,
    Option<String>,
    Option<String>,
    f64,
    String,
    i8,
    Option<String>,
    Option<String>,
);

fn map_product(row: ProductRow) -> Product {
    let (
        id,
        name,
        sku,
        description,
        category,
        unit_price,
        currency,
        is_active,
        created_at,
        updated_at,
    ) = row;
    Product {
        id,
        name,
        sku,
        description,
        category,
        unit_price,
        currency,
        is_active: is_active != 0,
        created_at,
        updated_at,
    }
}

pub async fn list_products(
    State(state): State<AppState>,
) -> Result<ApiResponse<Vec<Product>>, AppError> {
    let mut conn = state
        .pool
        .get_conn()
        .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

    let products = conn
        .query_map(
            format!("SELECT {PRODUCT_COLUMNS} FROM products ORDER BY id DESC"),
            map_product,
        )
        .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

    Ok(ApiResponse::success(products))
}

pub async fn create_product(
    State(state): State<AppState>,
    Json(payload): Json<CreateProductDto>,
) -> Result<(StatusCode, ApiResponse<Product>), AppError> {
    if payload.name.trim().is_empty() {
        return Err(AppError::Validation("name is required".into()));
    }

    let currency = payload.currency.unwrap_or_else(|| "IDR".to_string());

    let mut conn = state
        .pool
        .get_conn()
        .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

    conn.exec_drop(
        "INSERT INTO products (name, sku, description, category, unit_price, currency) \
         VALUES (:name, :sku, :description, :category, :unit_price, :currency)",
        params! {
            "name" => payload.name.trim(),
            "sku" => payload.sku.as_deref(),
            "description" => payload.description.as_deref(),
            "category" => payload.category.as_deref(),
            "unit_price" => payload.unit_price,
            "currency" => &currency,
        },
    )
    .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

    let last_id = conn.last_insert_id();
    let product = Product {
        id: last_id,
        name: payload.name,
        sku: payload.sku,
        description: payload.description,
        category: payload.category,
        unit_price: payload.unit_price,
        currency,
        is_active: true,
        created_at: None,
        updated_at: None,
    };

    state
        .broadcaster
        .notify("product", ChangeAction::Created, Some(last_id));

    Ok((StatusCode::CREATED, ApiResponse::success(product)))
}

pub async fn get_product(
    Path(id): Path<u64>,
    State(state): State<AppState>,
) -> Result<ApiResponse<Product>, AppError> {
    let mut conn = state
        .pool
        .get_conn()
        .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

    let product: Option<Product> = conn
        .exec_first(
            format!("SELECT {PRODUCT_COLUMNS} FROM products WHERE id = :id"),
            params! { "id" => id },
        )
        .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?
        .map(map_product);

    match product {
        Some(p) => Ok(ApiResponse::success(p)),
        None => Err(AppError::NotFound),
    }
}

pub async fn update_product(
    Path(id): Path<u64>,
    State(state): State<AppState>,
    Json(payload): Json<UpdateProductDto>,
) -> Result<ApiResponse<Product>, AppError> {
    let mut conn = state
        .pool
        .get_conn()
        .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

    let existing: Option<Product> = conn
        .exec_first(
            format!("SELECT {PRODUCT_COLUMNS} FROM products WHERE id = :id"),
            params! { "id" => id },
        )
        .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?
        .map(map_product);

    let Some(mut product) = existing else {
        return Err(AppError::NotFound);
    };

    if let Some(name) = payload.name {
        if name.trim().is_empty() {
            return Err(AppError::Validation("name is required".into()));
        }
        product.name = name;
    }
    if payload.sku.is_some() {
        product.sku = payload.sku;
    }
    if payload.description.is_some() {
        product.description = payload.description;
    }
    if payload.category.is_some() {
        product.category = payload.category;
    }
    if let Some(unit_price) = payload.unit_price {
        product.unit_price = unit_price;
    }
    if let Some(currency) = payload.currency {
        product.currency = currency;
    }
    if let Some(is_active) = payload.is_active {
        product.is_active = is_active;
    }

    conn.exec_drop(
        "UPDATE products SET name = :name, sku = :sku, description = :description, \
         category = :category, unit_price = :unit_price, currency = :currency, is_active = :is_active WHERE id = :id",
        params! {
            "id" => id,
            "name" => &product.name,
            "sku" => &product.sku,
            "description" => &product.description,
            "category" => &product.category,
            "unit_price" => product.unit_price,
            "currency" => &product.currency,
            "is_active" => product.is_active as i8,
        },
    )
    .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

    state
        .broadcaster
        .notify("product", ChangeAction::Updated, Some(id));

    Ok(ApiResponse::success(product))
}

pub async fn delete_product(
    Path(id): Path<u64>,
    State(state): State<AppState>,
) -> Result<StatusCode, AppError> {
    let mut conn = state
        .pool
        .get_conn()
        .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

    conn.exec_drop(
        "DELETE FROM products WHERE id = :id",
        params! { "id" => id },
    )
    .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

    if conn.affected_rows() > 0 {
        state
            .broadcaster
            .notify("product", ChangeAction::Deleted, Some(id));
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(AppError::NotFound)
    }
}
