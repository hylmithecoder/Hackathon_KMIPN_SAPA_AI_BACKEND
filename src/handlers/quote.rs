use crate::database::scheme::{Quote, QuoteItem};
use crate::error::AppError;
use crate::models::quote::{CreateQuoteDto, QuoteStatusDto, UpdateQuoteDto};
use crate::response::ApiResponse;
use crate::state::AppState;
use crate::utils::db::{map_mysql_err, opt_str, opt_u64, req_f64, req_str, req_u64, validate_deal, validate_product};
use crate::ws::event::ChangeAction;
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use mysql::params;
use mysql::prelude::*;

const QUOTE_COLUMNS: &str = "q.id, q.deal_id, q.quote_number, q.issue_date, q.expiry_date, \
    q.subtotal, q.tax_rate, q.tax_amount, q.total_amount, q.currency, q.status, q.notes, q.created_by, q.created_at, q.updated_at";

const QUOTE_ITEM_COLUMNS: &str =
    "id, quote_id, product_id, description, quantity, unit_price, discount, total";

fn row_to_quote(row: &mut mysql::Row) -> Result<Quote, AppError> {
    Ok(Quote {
        id: req_u64(row, "id")?,
        deal_id: req_u64(row, "deal_id")?,
        quote_number: req_str(row, "quote_number")?,
        issue_date: req_str(row, "issue_date")?,
        expiry_date: opt_str(row, "expiry_date"),
        subtotal: req_f64(row, "subtotal")?,
        tax_rate: req_f64(row, "tax_rate")?,
        tax_amount: req_f64(row, "tax_amount")?,
        total_amount: req_f64(row, "total_amount")?,
        currency: req_str(row, "currency")?,
        status: req_str(row, "status")?,
        notes: opt_str(row, "notes"),
        created_by: opt_u64(row, "created_by"),
        created_at: opt_str(row, "created_at"),
        updated_at: opt_str(row, "updated_at"),
    })
}

fn row_to_quote_item(row: &mut mysql::Row) -> Result<QuoteItem, AppError> {
    Ok(QuoteItem {
        id: req_u64(row, "id")?,
        quote_id: req_u64(row, "quote_id")?,
        product_id: opt_u64(row, "product_id"),
        description: req_str(row, "description")?,
        quantity: req_f64(row, "quantity")?,
        unit_price: req_f64(row, "unit_price")?,
        discount: req_f64(row, "discount")?,
        total: req_f64(row, "total")?,
    })
}

fn calc_quote_totals(
    items: &[crate::models::quote::CreateQuoteItemDto],
    tax_rate: f64,
) -> (f64, f64, f64) {
    let subtotal: f64 = items
        .iter()
        .map(|i| {
            let qty = i.quantity;
            let discount = i.discount.unwrap_or(0.0);
            (qty * i.unit_price - discount).max(0.0)
        })
        .sum();
    let tax_amount = subtotal * tax_rate / 100.0;
    let total = subtotal + tax_amount;
    (subtotal, tax_amount, total)
}

pub async fn list_quotes(
    State(state): State<AppState>,
) -> Result<ApiResponse<Vec<Quote>>, AppError> {
    let mut conn = state
        .pool
        .get_conn()
        .map_err(map_mysql_err)?;

    let quotes: Vec<Quote> = conn
        .query_map(
            format!("SELECT {QUOTE_COLUMNS} FROM quotes q ORDER BY q.id DESC"),
            |mut row: mysql::Row| row_to_quote(&mut row),
        )
        .map_err(map_mysql_err)?
        .into_iter()
        .collect::<Result<Vec<_>, _>>()?;

    Ok(ApiResponse::success(quotes))
}

pub async fn create_quote(
    State(state): State<AppState>,
    Json(payload): Json<CreateQuoteDto>,
) -> Result<(StatusCode, ApiResponse<Quote>), AppError> {
    if payload.quote_number.trim().is_empty() {
        return Err(AppError::Validation("quote_number is required".into()));
    }
    if payload.items.is_empty() {
        return Err(AppError::Validation("at least one item is required".into()));
    }

    let tax_rate = payload.tax_rate.unwrap_or(0.0);
    let currency = payload.currency.unwrap_or_else(|| "IDR".to_string());
    let (subtotal, tax_amount, total) = calc_quote_totals(&payload.items, tax_rate);

    let mut conn = state
        .pool
        .get_conn()
        .map_err(map_mysql_err)?;

    validate_deal(&mut conn, payload.deal_id, "deal_id")?;

    conn.exec_drop(
        "INSERT INTO quotes (deal_id, quote_number, issue_date, expiry_date, subtotal, tax_rate, tax_amount, total_amount, currency, notes) \
         VALUES (:deal_id, :quote_number, :issue_date, :expiry_date, :subtotal, :tax_rate, :tax_amount, :total_amount, :currency, :notes)",
        params! {
            "deal_id" => payload.deal_id,
            "quote_number" => payload.quote_number.trim(),
            "issue_date" => &payload.issue_date,
            "expiry_date" => payload.expiry_date.as_deref(),
            "subtotal" => subtotal,
            "tax_rate" => tax_rate,
            "tax_amount" => tax_amount,
            "total_amount" => total,
            "currency" => &currency,
            "notes" => payload.notes.as_deref(),
        },
    )
    .map_err(map_mysql_err)?;

    let quote_id = conn.last_insert_id();

    for item in &payload.items {
        if let Some(product_id) = item.product_id {
            validate_product(&mut conn, product_id, "product_id")?;
        }
        let discount = item.discount.unwrap_or(0.0);
        let total = (item.quantity * item.unit_price - discount).max(0.0);
        conn.exec_drop(
            "INSERT INTO quote_items (quote_id, product_id, description, quantity, unit_price, discount, total) \
             VALUES (:quote_id, :product_id, :description, :quantity, :unit_price, :discount, :total)",
            params! {
                "quote_id" => quote_id,
                "product_id" => item.product_id,
                "description" => &item.description,
                "quantity" => item.quantity,
                "unit_price" => item.unit_price,
                "discount" => discount,
                "total" => total,
            },
        )
        .map_err(map_mysql_err)?;
    }

    let quote = Quote {
        id: quote_id,
        deal_id: payload.deal_id,
        quote_number: payload.quote_number,
        issue_date: payload.issue_date,
        expiry_date: payload.expiry_date,
        subtotal,
        tax_rate,
        tax_amount,
        total_amount: total,
        currency,
        status: "draft".to_string(),
        notes: payload.notes,
        created_by: None,
        created_at: None,
        updated_at: None,
    };

    state
        .broadcaster
        .notify("quote", ChangeAction::Created, Some(quote_id));

    Ok((StatusCode::CREATED, ApiResponse::success(quote)))
}

pub async fn get_quote(
    Path(id): Path<u64>,
    State(state): State<AppState>,
) -> Result<ApiResponse<Quote>, AppError> {
    let mut conn = state
        .pool
        .get_conn()
        .map_err(map_mysql_err)?;

    let quote: Option<Quote> = conn
        .exec_first(
            format!("SELECT {QUOTE_COLUMNS} FROM quotes q WHERE q.id = :id"),
            params! { "id" => id },
        )
        .map_err(map_mysql_err)?
        .map(|mut row: mysql::Row| row_to_quote(&mut row))
        .transpose()?;

    match quote {
        Some(q) => Ok(ApiResponse::success(q)),
        None => Err(AppError::NotFound),
    }
}

pub async fn list_quote_items(
    Path(id): Path<u64>,
    State(state): State<AppState>,
) -> Result<ApiResponse<Vec<QuoteItem>>, AppError> {
    let mut conn = state
        .pool
        .get_conn()
        .map_err(map_mysql_err)?;

    let items: Vec<QuoteItem> = conn
        .exec_map(
            format!("SELECT {QUOTE_ITEM_COLUMNS} FROM quote_items WHERE quote_id = :id"),
            params! { "id" => id },
            |mut row: mysql::Row| row_to_quote_item(&mut row),
        )
        .map_err(map_mysql_err)?
        .into_iter()
        .collect::<Result<Vec<_>, _>>()?;

    Ok(ApiResponse::success(items))
}

pub async fn update_quote(
    Path(id): Path<u64>,
    State(state): State<AppState>,
    Json(payload): Json<UpdateQuoteDto>,
) -> Result<ApiResponse<Quote>, AppError> {
    let mut conn = state
        .pool
        .get_conn()
        .map_err(map_mysql_err)?;

    let existing: Option<Quote> = conn
        .exec_first(
            format!("SELECT {QUOTE_COLUMNS} FROM quotes q WHERE q.id = :id"),
            params! { "id" => id },
        )
        .map_err(map_mysql_err)?
        .map(|mut row: mysql::Row| row_to_quote(&mut row))
        .transpose()?;

    let Some(mut quote) = existing else {
        return Err(AppError::NotFound);
    };

    if let Some(quote_number) = payload.quote_number {
        if quote_number.trim().is_empty() {
            return Err(AppError::Validation("quote_number is required".into()));
        }
        quote.quote_number = quote_number;
    }
    if let Some(issue_date) = payload.issue_date {
        quote.issue_date = issue_date;
    }
    if payload.expiry_date.is_some() {
        quote.expiry_date = payload.expiry_date;
    }
    if let Some(tax_rate) = payload.tax_rate {
        quote.tax_rate = tax_rate;
    }
    if let Some(currency) = payload.currency {
        quote.currency = currency;
    }
    if let Some(status) = payload.status {
        quote.status = status;
    }
    if payload.notes.is_some() {
        quote.notes = payload.notes;
    }

    conn.exec_drop(
        "UPDATE quotes SET quote_number = :quote_number, issue_date = :issue_date, expiry_date = :expiry_date, \
         tax_rate = :tax_rate, currency = :currency, status = :status, notes = :notes WHERE id = :id",
        params! {
            "id" => id,
            "quote_number" => &quote.quote_number,
            "issue_date" => &quote.issue_date,
            "expiry_date" => &quote.expiry_date,
            "tax_rate" => quote.tax_rate,
            "currency" => &quote.currency,
            "status" => &quote.status,
            "notes" => &quote.notes,
        },
    )
    .map_err(map_mysql_err)?;

    state
        .broadcaster
        .notify("quote", ChangeAction::Updated, Some(id));

    Ok(ApiResponse::success(quote))
}

pub async fn update_quote_status(
    Path(id): Path<u64>,
    State(state): State<AppState>,
    Json(payload): Json<QuoteStatusDto>,
) -> Result<ApiResponse<Quote>, AppError> {
    if payload.status.trim().is_empty() {
        return Err(AppError::Validation("status is required".into()));
    }

    let mut conn = state
        .pool
        .get_conn()
        .map_err(map_mysql_err)?;

    conn.exec_drop(
        "UPDATE quotes SET status = :status WHERE id = :id",
        params! { "id" => id, "status" => payload.status.trim() },
    )
    .map_err(map_mysql_err)?;

    if conn.affected_rows() == 0 {
        return Err(AppError::NotFound);
    }

    state
        .broadcaster
        .notify("quote", ChangeAction::Updated, Some(id));

    get_quote(Path(id), State(state)).await
}

pub async fn delete_quote(
    Path(id): Path<u64>,
    State(state): State<AppState>,
) -> Result<StatusCode, AppError> {
    let mut conn = state
        .pool
        .get_conn()
        .map_err(map_mysql_err)?;

    conn.exec_drop("DELETE FROM quotes WHERE id = :id", params! { "id" => id })
        .map_err(map_mysql_err)?;

    if conn.affected_rows() > 0 {
        state
            .broadcaster
            .notify("quote", ChangeAction::Deleted, Some(id));
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(AppError::NotFound)
    }
}
