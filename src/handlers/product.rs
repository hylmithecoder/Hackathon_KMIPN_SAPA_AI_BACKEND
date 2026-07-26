use crate::database::scheme::{Product, ProductFile};
use crate::error::AppError;
use crate::models::product::{CreateProductDto, ProductFileDto, UpdateProductDto};
use crate::response::ApiResponse;
use crate::state::AppState;
use crate::utils::db::map_mysql_err;
use crate::ws::event::ChangeAction;
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use mysql::params;
use mysql::prelude::*;
use std::collections::HashMap;

const PRODUCT_COLUMNS: &str = "id, name, sku, description, category, unit_price, currency, file_url, file_name, is_active, \
     DATE_FORMAT(created_at, '%Y-%m-%d %H:%i:%s') AS created_at, \
     DATE_FORMAT(updated_at, '%Y-%m-%d %H:%i:%s') AS updated_at";

const PRODUCT_FILE_COLUMNS: &str = "id, product_id, file_url, file_name, \
    DATE_FORMAT(created_at, '%Y-%m-%d %H:%i:%s') AS created_at";

type ProductRow = (
    u64,
    String,
    Option<String>,
    Option<String>,
    Option<String>,
    f64,
    String,
    Option<String>,
    Option<String>,
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
        file_url,
        file_name,
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
        file_url,
        file_name,
        files: Vec::new(),
        is_active: is_active != 0,
        created_at,
        updated_at,
    }
}

fn map_product_file(row: (u64, u64, String, String, Option<String>)) -> ProductFile {
    let (id, product_id, file_url, file_name, created_at) = row;
    ProductFile {
        id,
        product_id,
        file_url,
        file_name,
        created_at,
    }
}

fn load_product_files(
    conn: &mut mysql::PooledConn,
    product_id: Option<u64>,
) -> Result<Vec<ProductFile>, AppError> {
    match product_id {
        Some(product_id) => conn
            .exec_map(
                format!(
                    "SELECT {PRODUCT_FILE_COLUMNS} FROM product_files \
                     WHERE product_id = :product_id ORDER BY id"
                ),
                params! { "product_id" => product_id },
                map_product_file,
            )
            .map_err(map_mysql_err),
        None => conn
            .query_map(
                format!(
                    "SELECT {PRODUCT_FILE_COLUMNS} FROM product_files \
                     ORDER BY product_id, id"
                ),
                map_product_file,
            )
            .map_err(map_mysql_err),
    }
}

fn validate_product_files(files: &[ProductFileDto]) -> Result<(), AppError> {
    if files.len() > 20 {
        return Err(AppError::Validation(
            "a product may contain at most 20 files".into(),
        ));
    }
    for (index, file) in files.iter().enumerate() {
        if file.file_url.trim().is_empty() {
            return Err(AppError::Validation(format!(
                "files[{index}].file_url is required"
            )));
        }
        if file.file_name.trim().is_empty() {
            return Err(AppError::Validation(format!(
                "files[{index}].file_name is required"
            )));
        }
    }
    Ok(())
}

fn legacy_file_list(file_url: &Option<String>, file_name: &Option<String>) -> Vec<ProductFileDto> {
    match (file_url.as_ref(), file_name.as_ref()) {
        (Some(file_url), Some(file_name))
            if !file_url.trim().is_empty() && !file_name.trim().is_empty() =>
        {
            vec![ProductFileDto {
                file_url: file_url.clone(),
                file_name: file_name.clone(),
            }]
        }
        _ => Vec::new(),
    }
}

pub async fn list_products(
    State(state): State<AppState>,
) -> Result<ApiResponse<Vec<Product>>, AppError> {
    let mut conn = state.pool.get_conn().map_err(map_mysql_err)?;

    let mut products: Vec<Product> = conn
        .query_map(
            format!("SELECT {PRODUCT_COLUMNS} FROM products ORDER BY id DESC"),
            map_product,
        )
        .map_err(map_mysql_err)?;
    let mut files_by_product: HashMap<u64, Vec<ProductFile>> = HashMap::new();
    for file in load_product_files(&mut conn, None)? {
        files_by_product
            .entry(file.product_id)
            .or_default()
            .push(file);
    }
    for product in &mut products {
        product.files = files_by_product.remove(&product.id).unwrap_or_default();
    }

    Ok(ApiResponse::success(products))
}

pub async fn create_product(
    State(state): State<AppState>,
    Json(payload): Json<CreateProductDto>,
) -> Result<(StatusCode, ApiResponse<Product>), AppError> {
    if payload.name.trim().is_empty() {
        return Err(AppError::Validation("name is required".into()));
    }

    let files = payload
        .files
        .clone()
        .unwrap_or_else(|| legacy_file_list(&payload.file_url, &payload.file_name));
    validate_product_files(&files)?;
    let primary_file = files.first();
    let currency = payload
        .currency
        .clone()
        .unwrap_or_else(|| "IDR".to_string());

    let mut conn = state.pool.get_conn().map_err(map_mysql_err)?;
    let mut transaction = conn
        .start_transaction(mysql::TxOpts::default())
        .map_err(map_mysql_err)?;
    transaction.exec_drop(
        "INSERT INTO products (name, sku, description, category, unit_price, currency, file_url, file_name) \
         VALUES (:name, :sku, :description, :category, :unit_price, :currency, :file_url, :file_name)",
        params! {
            "name" => payload.name.trim(),
            "sku" => payload.sku.as_deref(),
            "description" => payload.description.as_deref(),
            "category" => payload.category.as_deref(),
            "unit_price" => payload.unit_price,
            "currency" => &currency,
            "file_url" => primary_file.map(|file| file.file_url.as_str()),
            "file_name" => primary_file.map(|file| file.file_name.as_str()),
        },
    )
    .map_err(map_mysql_err)?;

    let last_id: u64 = transaction
        .query_first("SELECT LAST_INSERT_ID()")
        .map_err(map_mysql_err)?
        .ok_or_else(|| AppError::Internal(anyhow::anyhow!("missing product insert id")))?;
    for file in &files {
        transaction
            .exec_drop(
                "INSERT INTO product_files (product_id, file_url, file_name) \
                 VALUES (:product_id, :file_url, :file_name)",
                params! {
                    "product_id" => last_id,
                    "file_url" => file.file_url.trim(),
                    "file_name" => file.file_name.trim(),
                },
            )
            .map_err(map_mysql_err)?;
    }
    transaction.commit().map_err(map_mysql_err)?;
    drop(conn);

    let response = get_product(Path(last_id), State(state.clone())).await?;
    let product = response
        .data
        .clone()
        .ok_or_else(|| AppError::Internal(anyhow::anyhow!("created product missing")))?;
    state.broadcaster.notify_with_payload(
        "product",
        ChangeAction::Created,
        Some(last_id),
        &product,
    );

    Ok((StatusCode::CREATED, ApiResponse::success(product)))
}

pub async fn get_product(
    Path(id): Path<u64>,
    State(state): State<AppState>,
) -> Result<ApiResponse<Product>, AppError> {
    let mut conn = state.pool.get_conn().map_err(map_mysql_err)?;

    let product: Option<Product> = conn
        .exec_first(
            format!("SELECT {PRODUCT_COLUMNS} FROM products WHERE id = :id"),
            params! { "id" => id },
        )
        .map_err(map_mysql_err)?
        .map(map_product);

    match product {
        Some(mut product) => {
            product.files = load_product_files(&mut conn, Some(id))?;
            Ok(ApiResponse::success(product))
        }
        None => Err(AppError::NotFound),
    }
}

pub async fn update_product(
    Path(id): Path<u64>,
    State(state): State<AppState>,
    Json(payload): Json<UpdateProductDto>,
) -> Result<ApiResponse<Product>, AppError> {
    let mut conn = state.pool.get_conn().map_err(map_mysql_err)?;

    let existing: Option<Product> = conn
        .exec_first(
            format!("SELECT {PRODUCT_COLUMNS} FROM products WHERE id = :id"),
            params! { "id" => id },
        )
        .map_err(map_mysql_err)?
        .map(map_product);

    let Some(mut product) = existing else {
        return Err(AppError::NotFound);
    };

    let UpdateProductDto {
        name,
        sku,
        description,
        category,
        unit_price,
        currency,
        file_url,
        file_name,
        files,
        is_active,
    } = payload;
    let legacy_file_changed = file_url.is_some() || file_name.is_some();

    if let Some(name) = name {
        if name.trim().is_empty() {
            return Err(AppError::Validation("name is required".into()));
        }
        product.name = name;
    }
    if sku.is_some() {
        product.sku = sku;
    }
    if description.is_some() {
        product.description = description;
    }
    if category.is_some() {
        product.category = category;
    }
    if let Some(unit_price) = unit_price {
        product.unit_price = unit_price;
    }
    if let Some(currency) = currency {
        product.currency = currency;
    }
    if let Some(file_url) = file_url {
        product.file_url = file_url;
    }
    if let Some(file_name) = file_name {
        product.file_name = file_name;
    }
    if let Some(is_active) = is_active {
        product.is_active = is_active;
    }

    let replacement_files = match files {
        Some(files) => Some(files),
        None if legacy_file_changed => {
            Some(legacy_file_list(&product.file_url, &product.file_name))
        }
        None => None,
    };
    if let Some(files) = replacement_files.as_ref() {
        validate_product_files(files)?;
        product.file_url = files.first().map(|file| file.file_url.clone());
        product.file_name = files.first().map(|file| file.file_name.clone());
    }

    let mut transaction = conn
        .start_transaction(mysql::TxOpts::default())
        .map_err(map_mysql_err)?;
    transaction
        .exec_drop(
            "UPDATE products SET name = :name, sku = :sku, description = :description, \
         category = :category, unit_price = :unit_price, currency = :currency, \
         file_url = :file_url, file_name = :file_name, is_active = :is_active WHERE id = :id",
            params! {
                "id" => id,
                "name" => &product.name,
                "sku" => &product.sku,
                "description" => &product.description,
                "category" => &product.category,
                "unit_price" => product.unit_price,
                "currency" => &product.currency,
                "file_url" => &product.file_url,
                "file_name" => &product.file_name,
                "is_active" => product.is_active as i8,
            },
        )
        .map_err(map_mysql_err)?;

    if let Some(files) = replacement_files {
        transaction
            .exec_drop(
                "DELETE FROM product_files WHERE product_id = :product_id",
                params! { "product_id" => id },
            )
            .map_err(map_mysql_err)?;
        for file in files {
            transaction
                .exec_drop(
                    "INSERT INTO product_files (product_id, file_url, file_name) \
                     VALUES (:product_id, :file_url, :file_name)",
                    params! {
                        "product_id" => id,
                        "file_url" => file.file_url.trim(),
                        "file_name" => file.file_name.trim(),
                    },
                )
                .map_err(map_mysql_err)?;
        }
    }
    transaction.commit().map_err(map_mysql_err)?;
    drop(conn);

    let response = get_product(Path(id), State(state.clone())).await?;
    if let Some(product) = response.data.as_ref() {
        state
            .broadcaster
            .notify_with_payload("product", ChangeAction::Updated, Some(id), product);
    }
    Ok(response)
}

pub async fn delete_product(
    Path(id): Path<u64>,
    State(state): State<AppState>,
) -> Result<StatusCode, AppError> {
    let mut conn = state.pool.get_conn().map_err(map_mysql_err)?;

    conn.exec_drop(
        "DELETE FROM products WHERE id = :id",
        params! { "id" => id },
    )
    .map_err(map_mysql_err)?;

    if conn.affected_rows() > 0 {
        state
            .broadcaster
            .notify("product", ChangeAction::Deleted, Some(id));
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(AppError::NotFound)
    }
}
