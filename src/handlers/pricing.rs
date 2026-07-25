use crate::database::scheme::{PriceBook, PriceBookItem};
use crate::error::AppError;
use crate::models::pricing::{
    CreatePriceBookDto, CreatePriceBookItemDto, PriceResolutionQuery, UpdatePriceBookDto,
    UpdatePriceBookItemDto,
};
use crate::response::ApiResponse;
use crate::state::AppState;
use crate::utils::db::{map_mysql_err, validate_product};
use crate::ws::event::ChangeAction;
use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
};
use mysql::params;
use mysql::prelude::*;
use serde::Serialize;

const PRICE_BOOK_COLUMNS: &str = "id, name, currency, description, is_default, is_active, \
    DATE_FORMAT(created_at, '%Y-%m-%d %H:%i:%s') AS created_at, \
    DATE_FORMAT(updated_at, '%Y-%m-%d %H:%i:%s') AS updated_at";
const PRICE_BOOK_ITEM_COLUMNS: &str = "id, price_book_id, product_id, min_quantity, unit_price, \
    DATE_FORMAT(created_at, '%Y-%m-%d %H:%i:%s') AS created_at, \
    DATE_FORMAT(updated_at, '%Y-%m-%d %H:%i:%s') AS updated_at";

type PriceBookRow = (
    u64,
    String,
    String,
    Option<String>,
    i8,
    i8,
    Option<String>,
    Option<String>,
);
type PriceBookItemRow = (u64, u64, u64, f64, f64, Option<String>, Option<String>);

fn map_price_book(row: PriceBookRow) -> PriceBook {
    let (id, name, currency, description, is_default, is_active, created_at, updated_at) = row;
    PriceBook {
        id,
        name,
        currency,
        description,
        is_default: is_default != 0,
        is_active: is_active != 0,
        created_at,
        updated_at,
    }
}

fn map_price_book_item(row: PriceBookItemRow) -> PriceBookItem {
    let (id, price_book_id, product_id, min_quantity, unit_price, created_at, updated_at) = row;
    PriceBookItem {
        id,
        price_book_id,
        product_id,
        min_quantity,
        unit_price,
        created_at,
        updated_at,
    }
}

fn validate_amount(value: f64, field: &str, allow_zero: bool) -> Result<(), AppError> {
    if !value.is_finite() || value < 0.0 || (!allow_zero && value == 0.0) {
        return Err(AppError::Validation(format!(
            "{field} must be a {}finite number",
            if allow_zero {
                "non-negative "
            } else {
                "positive "
            }
        )));
    }
    Ok(())
}

fn validate_currency(currency: &str) -> Result<(), AppError> {
    if currency.len() != 3 || !currency.bytes().all(|byte| byte.is_ascii_alphabetic()) {
        return Err(AppError::Validation(
            "currency must be a 3-letter ISO code".into(),
        ));
    }
    Ok(())
}

fn clear_default(conn: &mut mysql::PooledConn) -> Result<(), AppError> {
    conn.query_drop("UPDATE price_books SET is_default = 0 WHERE is_default = 1")
        .map_err(map_mysql_err)
}

pub async fn list_price_books(
    State(state): State<AppState>,
) -> Result<ApiResponse<Vec<PriceBook>>, AppError> {
    let mut conn = state.pool.get_conn().map_err(map_mysql_err)?;
    let items = conn
        .query_map(
            format!(
                "SELECT {PRICE_BOOK_COLUMNS} FROM price_books ORDER BY is_default DESC, name ASC"
            ),
            map_price_book,
        )
        .map_err(map_mysql_err)?;
    Ok(ApiResponse::success(items))
}

pub async fn create_price_book(
    State(state): State<AppState>,
    Json(payload): Json<CreatePriceBookDto>,
) -> Result<(StatusCode, ApiResponse<PriceBook>), AppError> {
    if payload.name.trim().is_empty() {
        return Err(AppError::Validation("name is required".into()));
    }
    let currency = payload
        .currency
        .unwrap_or_else(|| "IDR".into())
        .to_uppercase();
    validate_currency(&currency)?;
    let is_default = payload.is_default.unwrap_or(false);
    let mut conn = state.pool.get_conn().map_err(map_mysql_err)?;
    if is_default {
        clear_default(&mut conn)?;
    }
    conn.exec_drop(
        "INSERT INTO price_books (name, currency, description, is_default) VALUES (:name, :currency, :description, :is_default)",
        params! { "name" => payload.name.trim(), "currency" => &currency, "description" => payload.description.as_deref(), "is_default" => is_default as i8 },
    ).map_err(map_mysql_err)?;
    let id = conn.last_insert_id();
    let price_book = PriceBook {
        id,
        name: payload.name,
        currency,
        description: payload.description,
        is_default,
        is_active: true,
        created_at: None,
        updated_at: None,
    };
    state
        .broadcaster
        .notify("price_book", ChangeAction::Created, Some(id));
    Ok((StatusCode::CREATED, ApiResponse::success(price_book)))
}

pub async fn get_price_book(
    Path(id): Path<u64>,
    State(state): State<AppState>,
) -> Result<ApiResponse<PriceBook>, AppError> {
    let mut conn = state.pool.get_conn().map_err(map_mysql_err)?;
    let price_book = conn
        .exec_first(
            format!("SELECT {PRICE_BOOK_COLUMNS} FROM price_books WHERE id = :id"),
            params! { "id" => id },
        )
        .map_err(map_mysql_err)?
        .map(map_price_book)
        .ok_or(AppError::NotFound)?;
    Ok(ApiResponse::success(price_book))
}

pub async fn update_price_book(
    Path(id): Path<u64>,
    State(state): State<AppState>,
    Json(payload): Json<UpdatePriceBookDto>,
) -> Result<ApiResponse<PriceBook>, AppError> {
    let mut conn = state.pool.get_conn().map_err(map_mysql_err)?;
    let mut item = conn
        .exec_first(
            format!("SELECT {PRICE_BOOK_COLUMNS} FROM price_books WHERE id = :id"),
            params! { "id" => id },
        )
        .map_err(map_mysql_err)?
        .map(map_price_book)
        .ok_or(AppError::NotFound)?;
    if let Some(name) = payload.name {
        if name.trim().is_empty() {
            return Err(AppError::Validation("name is required".into()));
        }
        item.name = name;
    }
    if let Some(currency) = payload.currency {
        let currency = currency.to_uppercase();
        validate_currency(&currency)?;
        item.currency = currency;
    }
    if payload.description.is_some() {
        item.description = payload.description;
    }
    if let Some(is_active) = payload.is_active {
        item.is_active = is_active;
    }
    if let Some(is_default) = payload.is_default {
        if is_default {
            clear_default(&mut conn)?;
        }
        item.is_default = is_default;
    }
    conn.exec_drop(
        "UPDATE price_books SET name = :name, currency = :currency, description = :description, is_default = :is_default, is_active = :is_active WHERE id = :id",
        params! { "id" => id, "name" => &item.name, "currency" => &item.currency, "description" => &item.description, "is_default" => item.is_default as i8, "is_active" => item.is_active as i8 },
    ).map_err(map_mysql_err)?;
    state
        .broadcaster
        .notify("price_book", ChangeAction::Updated, Some(id));
    Ok(ApiResponse::success(item))
}

pub async fn delete_price_book(
    Path(id): Path<u64>,
    State(state): State<AppState>,
) -> Result<StatusCode, AppError> {
    let mut conn = state.pool.get_conn().map_err(map_mysql_err)?;
    conn.exec_drop(
        "DELETE FROM price_books WHERE id = :id",
        params! { "id" => id },
    )
    .map_err(map_mysql_err)?;
    if conn.affected_rows() == 0 {
        return Err(AppError::NotFound);
    }
    state
        .broadcaster
        .notify("price_book", ChangeAction::Deleted, Some(id));
    Ok(StatusCode::NO_CONTENT)
}

pub async fn list_price_book_items(
    Path(id): Path<u64>,
    State(state): State<AppState>,
) -> Result<ApiResponse<Vec<PriceBookItem>>, AppError> {
    let mut conn = state.pool.get_conn().map_err(map_mysql_err)?;
    let items = conn.exec_map(format!("SELECT {PRICE_BOOK_ITEM_COLUMNS} FROM price_book_items WHERE price_book_id = :id ORDER BY product_id, min_quantity"), params! { "id" => id }, map_price_book_item).map_err(map_mysql_err)?;
    Ok(ApiResponse::success(items))
}

pub async fn create_price_book_item(
    Path(id): Path<u64>,
    State(state): State<AppState>,
    Json(payload): Json<CreatePriceBookItemDto>,
) -> Result<(StatusCode, ApiResponse<PriceBookItem>), AppError> {
    let min_quantity = payload.min_quantity.unwrap_or(1.0);
    validate_amount(min_quantity, "min_quantity", false)?;
    validate_amount(payload.unit_price, "unit_price", true)?;
    let mut conn = state.pool.get_conn().map_err(map_mysql_err)?;
    let exists: Option<u8> = conn
        .exec_first(
            "SELECT 1 FROM price_books WHERE id = :id",
            params! { "id" => id },
        )
        .map_err(map_mysql_err)?;
    if exists.is_none() {
        return Err(AppError::NotFound);
    }
    validate_product(&mut conn, payload.product_id, "product_id")?;
    conn.exec_drop(
        "INSERT INTO price_book_items (price_book_id, product_id, min_quantity, unit_price) VALUES (:price_book_id, :product_id, :min_quantity, :unit_price)",
        params! { "price_book_id" => id, "product_id" => payload.product_id, "min_quantity" => min_quantity, "unit_price" => payload.unit_price },
    ).map_err(map_mysql_err)?;
    let item = PriceBookItem {
        id: conn.last_insert_id(),
        price_book_id: id,
        product_id: payload.product_id,
        min_quantity,
        unit_price: payload.unit_price,
        created_at: None,
        updated_at: None,
    };
    state
        .broadcaster
        .notify("price_book", ChangeAction::Updated, Some(id));
    Ok((StatusCode::CREATED, ApiResponse::success(item)))
}

pub async fn update_price_book_item(
    Path((price_book_id, item_id)): Path<(u64, u64)>,
    State(state): State<AppState>,
    Json(payload): Json<UpdatePriceBookItemDto>,
) -> Result<ApiResponse<PriceBookItem>, AppError> {
    let mut conn = state.pool.get_conn().map_err(map_mysql_err)?;
    let mut item = conn.exec_first(format!("SELECT {PRICE_BOOK_ITEM_COLUMNS} FROM price_book_items WHERE id = :item_id AND price_book_id = :price_book_id"), params! { "item_id" => item_id, "price_book_id" => price_book_id }).map_err(map_mysql_err)?.map(map_price_book_item).ok_or(AppError::NotFound)?;
    if let Some(value) = payload.min_quantity {
        validate_amount(value, "min_quantity", false)?;
        item.min_quantity = value;
    }
    if let Some(value) = payload.unit_price {
        validate_amount(value, "unit_price", true)?;
        item.unit_price = value;
    }
    conn.exec_drop("UPDATE price_book_items SET min_quantity = :min_quantity, unit_price = :unit_price WHERE id = :id", params! { "id" => item_id, "min_quantity" => item.min_quantity, "unit_price" => item.unit_price }).map_err(map_mysql_err)?;
    state
        .broadcaster
        .notify("price_book", ChangeAction::Updated, Some(price_book_id));
    Ok(ApiResponse::success(item))
}

pub async fn delete_price_book_item(
    Path((price_book_id, item_id)): Path<(u64, u64)>,
    State(state): State<AppState>,
) -> Result<StatusCode, AppError> {
    let mut conn = state.pool.get_conn().map_err(map_mysql_err)?;
    conn.exec_drop(
        "DELETE FROM price_book_items WHERE id = :item_id AND price_book_id = :price_book_id",
        params! { "item_id" => item_id, "price_book_id" => price_book_id },
    )
    .map_err(map_mysql_err)?;
    if conn.affected_rows() == 0 {
        return Err(AppError::NotFound);
    }
    state
        .broadcaster
        .notify("price_book", ChangeAction::Updated, Some(price_book_id));
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug, Serialize)]
pub struct ResolvedPrice {
    pub price_book_id: u64,
    pub product_id: u64,
    pub quantity: f64,
    pub min_quantity: f64,
    pub unit_price: f64,
}

pub async fn resolve_price(
    Path((price_book_id, product_id)): Path<(u64, u64)>,
    Query(query): Query<PriceResolutionQuery>,
    State(state): State<AppState>,
) -> Result<ApiResponse<ResolvedPrice>, AppError> {
    let quantity = query.quantity.unwrap_or(1.0);
    validate_amount(quantity, "quantity", false)?;
    let mut conn = state.pool.get_conn().map_err(map_mysql_err)?;
    let row: Option<(f64, f64)> = conn.exec_first(
        "SELECT pbi.min_quantity, pbi.unit_price FROM price_book_items pbi JOIN price_books pb ON pb.id = pbi.price_book_id WHERE pbi.price_book_id = :price_book_id AND pbi.product_id = :product_id AND pb.is_active = 1 AND pbi.min_quantity <= :quantity ORDER BY pbi.min_quantity DESC LIMIT 1",
        params! { "price_book_id" => price_book_id, "product_id" => product_id, "quantity" => quantity },
    ).map_err(map_mysql_err)?;
    let Some((min_quantity, unit_price)) = row else {
        return Err(AppError::NotFound);
    };
    Ok(ApiResponse::success(ResolvedPrice {
        price_book_id,
        product_id,
        quantity,
        min_quantity,
        unit_price,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn amount_validation_rejects_invalid_prices() {
        assert!(validate_amount(0.0, "quantity", false).is_err());
        assert!(validate_amount(-1.0, "price", true).is_err());
        assert!(validate_amount(f64::NAN, "price", true).is_err());
        assert!(validate_amount(0.0, "price", true).is_ok());
    }

    #[test]
    fn currency_validation_requires_iso_shape() {
        assert!(validate_currency("IDR").is_ok());
        assert!(validate_currency("RP").is_err());
        assert!(validate_currency("12!").is_err());
    }
}
