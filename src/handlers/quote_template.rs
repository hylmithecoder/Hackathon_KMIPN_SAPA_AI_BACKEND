use crate::database::scheme::{Quote, QuoteTemplate, QuoteTemplateItem};
use crate::error::AppError;
use crate::models::quote_template::{
    CreateQuoteTemplateDto, InstantiateQuoteTemplateDto, QuoteTemplateItemDto,
    UpdateQuoteTemplateDto,
};
use crate::response::ApiResponse;
use crate::state::AppState;
use crate::utils::db::{map_mysql_err, validate_deal, validate_product};
use crate::ws::event::ChangeAction;
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use chrono::Utc;
use mysql::params;
use mysql::prelude::*;
use uuid::Uuid;

const TEMPLATE_COLUMNS: &str = "id, name, description, currency, tax_rate, notes, terms, is_active, \
    DATE_FORMAT(created_at, '%Y-%m-%d %H:%i:%s') AS created_at, \
    DATE_FORMAT(updated_at, '%Y-%m-%d %H:%i:%s') AS updated_at";
const TEMPLATE_ITEM_COLUMNS: &str =
    "id, template_id, product_id, description, quantity, unit_price, discount, position";

type TemplateRow = (
    u64,
    String,
    Option<String>,
    String,
    f64,
    Option<String>,
    Option<String>,
    i8,
    Option<String>,
    Option<String>,
);
type TemplateItemRow = (u64, u64, Option<u64>, String, f64, f64, f64, u32);

fn map_template(row: TemplateRow) -> QuoteTemplate {
    let (
        id,
        name,
        description,
        currency,
        tax_rate,
        notes,
        terms,
        is_active,
        created_at,
        updated_at,
    ) = row;
    QuoteTemplate {
        id,
        name,
        description,
        currency,
        tax_rate,
        notes,
        terms,
        is_active: is_active != 0,
        created_at,
        updated_at,
    }
}

fn map_template_item(row: TemplateItemRow) -> QuoteTemplateItem {
    let (id, template_id, product_id, description, quantity, unit_price, discount, position) = row;
    QuoteTemplateItem {
        id,
        template_id,
        product_id,
        description,
        quantity,
        unit_price,
        discount,
        position,
    }
}

fn validate_template_items(items: &[QuoteTemplateItemDto]) -> Result<(), AppError> {
    if items.is_empty() {
        return Err(AppError::Validation(
            "at least one template item is required".into(),
        ));
    }
    for item in items {
        if item.description.trim().is_empty() {
            return Err(AppError::Validation(
                "template item description is required".into(),
            ));
        }
        if !item.quantity.is_finite() || item.quantity <= 0.0 {
            return Err(AppError::Validation(
                "template item quantity must be positive".into(),
            ));
        }
        if !item.unit_price.is_finite() || item.unit_price < 0.0 {
            return Err(AppError::Validation(
                "template item unit_price must be non-negative".into(),
            ));
        }
        if let Some(discount) = item.discount
            && (!discount.is_finite() || discount < 0.0)
        {
            return Err(AppError::Validation(
                "template item discount must be non-negative".into(),
            ));
        }
    }
    Ok(())
}

fn validate_template_products(
    conn: &mut mysql::PooledConn,
    items: &[QuoteTemplateItemDto],
) -> Result<(), AppError> {
    for item in items {
        if let Some(product_id) = item.product_id {
            validate_product(conn, product_id, "product_id")?;
        }
    }
    Ok(())
}

fn validate_template(template: &QuoteTemplate) -> Result<(), AppError> {
    if template.name.trim().is_empty() {
        return Err(AppError::Validation("name is required".into()));
    }
    if template.currency.len() != 3
        || !template
            .currency
            .bytes()
            .all(|byte| byte.is_ascii_alphabetic())
    {
        return Err(AppError::Validation(
            "currency must be a 3-letter ISO code".into(),
        ));
    }
    if !template.tax_rate.is_finite() || !(0.0..=100.0).contains(&template.tax_rate) {
        return Err(AppError::Validation(
            "tax_rate must be between 0 and 100".into(),
        ));
    }
    Ok(())
}

fn insert_template_items(
    conn: &mut mysql::PooledConn,
    template_id: u64,
    items: &[QuoteTemplateItemDto],
) -> Result<(), AppError> {
    for (position, item) in items.iter().enumerate() {
        conn.exec_drop(
            "INSERT INTO quote_template_items (template_id, product_id, description, quantity, unit_price, discount, position) VALUES (:template_id, :product_id, :description, :quantity, :unit_price, :discount, :position)",
            params! { "template_id" => template_id, "product_id" => item.product_id, "description" => item.description.trim(), "quantity" => item.quantity, "unit_price" => item.unit_price, "discount" => item.discount.unwrap_or(0.0), "position" => position as u32 },
        ).map_err(map_mysql_err)?;
    }
    Ok(())
}

pub async fn list_quote_templates(
    State(state): State<AppState>,
) -> Result<ApiResponse<Vec<QuoteTemplate>>, AppError> {
    let mut conn = state.pool.get_conn().map_err(map_mysql_err)?;
    let templates = conn
        .query_map(
            format!("SELECT {TEMPLATE_COLUMNS} FROM quote_templates ORDER BY name"),
            map_template,
        )
        .map_err(map_mysql_err)?;
    Ok(ApiResponse::success(templates))
}

pub async fn create_quote_template(
    State(state): State<AppState>,
    Json(payload): Json<CreateQuoteTemplateDto>,
) -> Result<(StatusCode, ApiResponse<QuoteTemplate>), AppError> {
    validate_template_items(&payload.items)?;
    let template = QuoteTemplate {
        id: 0,
        name: payload.name,
        description: payload.description,
        currency: payload
            .currency
            .unwrap_or_else(|| "IDR".into())
            .to_uppercase(),
        tax_rate: payload.tax_rate.unwrap_or(0.0),
        notes: payload.notes,
        terms: payload.terms,
        is_active: true,
        created_at: None,
        updated_at: None,
    };
    validate_template(&template)?;
    let mut conn = state.pool.get_conn().map_err(map_mysql_err)?;
    validate_template_products(&mut conn, &payload.items)?;
    conn.exec_drop(
        "INSERT INTO quote_templates (name, description, currency, tax_rate, notes, terms) VALUES (:name, :description, :currency, :tax_rate, :notes, :terms)",
        params! { "name" => template.name.trim(), "description" => &template.description, "currency" => &template.currency, "tax_rate" => template.tax_rate, "notes" => &template.notes, "terms" => &template.terms },
    ).map_err(map_mysql_err)?;
    let id = conn.last_insert_id();
    insert_template_items(&mut conn, id, &payload.items)?;
    let template = QuoteTemplate { id, ..template };
    state
        .broadcaster
        .notify("quote_template", ChangeAction::Created, Some(id));
    Ok((StatusCode::CREATED, ApiResponse::success(template)))
}

pub async fn get_quote_template(
    Path(id): Path<u64>,
    State(state): State<AppState>,
) -> Result<ApiResponse<QuoteTemplate>, AppError> {
    let mut conn = state.pool.get_conn().map_err(map_mysql_err)?;
    let template = conn
        .exec_first(
            format!("SELECT {TEMPLATE_COLUMNS} FROM quote_templates WHERE id = :id"),
            params! { "id" => id },
        )
        .map_err(map_mysql_err)?
        .map(map_template)
        .ok_or(AppError::NotFound)?;
    Ok(ApiResponse::success(template))
}

pub async fn list_quote_template_items(
    Path(id): Path<u64>,
    State(state): State<AppState>,
) -> Result<ApiResponse<Vec<QuoteTemplateItem>>, AppError> {
    let mut conn = state.pool.get_conn().map_err(map_mysql_err)?;
    let items = conn.exec_map(format!("SELECT {TEMPLATE_ITEM_COLUMNS} FROM quote_template_items WHERE template_id = :id ORDER BY position, id"), params! { "id" => id }, map_template_item).map_err(map_mysql_err)?;
    Ok(ApiResponse::success(items))
}

pub async fn update_quote_template(
    Path(id): Path<u64>,
    State(state): State<AppState>,
    Json(payload): Json<UpdateQuoteTemplateDto>,
) -> Result<ApiResponse<QuoteTemplate>, AppError> {
    if let Some(items) = &payload.items {
        validate_template_items(items)?;
    }
    let mut conn = state.pool.get_conn().map_err(map_mysql_err)?;
    if let Some(items) = &payload.items {
        validate_template_products(&mut conn, items)?;
    }
    let mut template = conn
        .exec_first(
            format!("SELECT {TEMPLATE_COLUMNS} FROM quote_templates WHERE id = :id"),
            params! { "id" => id },
        )
        .map_err(map_mysql_err)?
        .map(map_template)
        .ok_or(AppError::NotFound)?;
    if let Some(name) = payload.name {
        template.name = name;
    }
    if payload.description.is_some() {
        template.description = payload.description;
    }
    if let Some(currency) = payload.currency {
        template.currency = currency.to_uppercase();
    }
    if let Some(tax_rate) = payload.tax_rate {
        template.tax_rate = tax_rate;
    }
    if payload.notes.is_some() {
        template.notes = payload.notes;
    }
    if payload.terms.is_some() {
        template.terms = payload.terms;
    }
    if let Some(is_active) = payload.is_active {
        template.is_active = is_active;
    }
    validate_template(&template)?;
    conn.exec_drop(
        "UPDATE quote_templates SET name = :name, description = :description, currency = :currency, tax_rate = :tax_rate, notes = :notes, terms = :terms, is_active = :is_active WHERE id = :id",
        params! { "id" => id, "name" => template.name.trim(), "description" => &template.description, "currency" => &template.currency, "tax_rate" => template.tax_rate, "notes" => &template.notes, "terms" => &template.terms, "is_active" => template.is_active as i8 },
    ).map_err(map_mysql_err)?;
    if let Some(items) = payload.items {
        conn.exec_drop(
            "DELETE FROM quote_template_items WHERE template_id = :id",
            params! { "id" => id },
        )
        .map_err(map_mysql_err)?;
        insert_template_items(&mut conn, id, &items)?;
    }
    state
        .broadcaster
        .notify("quote_template", ChangeAction::Updated, Some(id));
    Ok(ApiResponse::success(template))
}

pub async fn delete_quote_template(
    Path(id): Path<u64>,
    State(state): State<AppState>,
) -> Result<StatusCode, AppError> {
    let mut conn = state.pool.get_conn().map_err(map_mysql_err)?;
    conn.exec_drop(
        "DELETE FROM quote_templates WHERE id = :id",
        params! { "id" => id },
    )
    .map_err(map_mysql_err)?;
    if conn.affected_rows() == 0 {
        return Err(AppError::NotFound);
    }
    state
        .broadcaster
        .notify("quote_template", ChangeAction::Deleted, Some(id));
    Ok(StatusCode::NO_CONTENT)
}

fn calc_subtotal(items: &[QuoteTemplateItem]) -> f64 {
    items
        .iter()
        .map(|item| (item.quantity * item.unit_price - item.discount).max(0.0))
        .sum()
}

fn compose_notes(notes: Option<String>, terms: Option<String>) -> Option<String> {
    match (
        notes.filter(|value| !value.trim().is_empty()),
        terms.filter(|value| !value.trim().is_empty()),
    ) {
        (Some(notes), Some(terms)) => Some(format!("{notes}\n\nTerms:\n{terms}")),
        (Some(notes), None) => Some(notes),
        (None, Some(terms)) => Some(format!("Terms:\n{terms}")),
        (None, None) => None,
    }
}

fn generated_quote_number(template_id: u64) -> String {
    format!("QT-{template_id}-{}", Uuid::new_v4().simple())
}

pub async fn instantiate_quote_template(
    Path(id): Path<u64>,
    State(state): State<AppState>,
    Json(payload): Json<InstantiateQuoteTemplateDto>,
) -> Result<(StatusCode, ApiResponse<Quote>), AppError> {
    let mut conn = state.pool.get_conn().map_err(map_mysql_err)?;
    let template = conn
        .exec_first(
            format!("SELECT {TEMPLATE_COLUMNS} FROM quote_templates WHERE id = :id"),
            params! { "id" => id },
        )
        .map_err(map_mysql_err)?
        .map(map_template)
        .ok_or(AppError::NotFound)?;
    if !template.is_active {
        return Err(AppError::BadRequest("quote template is inactive".into()));
    }
    validate_deal(&mut conn, payload.deal_id, "deal_id")?;
    let items = conn.exec_map(format!("SELECT {TEMPLATE_ITEM_COLUMNS} FROM quote_template_items WHERE template_id = :id ORDER BY position, id"), params! { "id" => id }, map_template_item).map_err(map_mysql_err)?;
    if items.is_empty() {
        return Err(AppError::Validation("quote template has no items".into()));
    }
    let quote_number = payload
        .quote_number
        .unwrap_or_else(|| generated_quote_number(id));
    if quote_number.trim().is_empty() {
        return Err(AppError::Validation("quote_number is required".into()));
    }
    let issue_date = payload
        .issue_date
        .unwrap_or_else(|| Utc::now().date_naive().to_string());
    let subtotal = calc_subtotal(&items);
    let tax_amount = subtotal * template.tax_rate / 100.0;
    let total_amount = subtotal + tax_amount;
    let notes = compose_notes(
        payload.notes.or(template.notes.clone()),
        template.terms.clone(),
    );
    conn.exec_drop(
        "INSERT INTO quotes (deal_id, template_id, quote_number, issue_date, expiry_date, subtotal, tax_rate, tax_amount, total_amount, currency, notes) VALUES (:deal_id, :template_id, :quote_number, :issue_date, :expiry_date, :subtotal, :tax_rate, :tax_amount, :total_amount, :currency, :notes)",
        params! { "deal_id" => payload.deal_id, "template_id" => id, "quote_number" => quote_number.trim(), "issue_date" => &issue_date, "expiry_date" => payload.expiry_date.as_deref(), "subtotal" => subtotal, "tax_rate" => template.tax_rate, "tax_amount" => tax_amount, "total_amount" => total_amount, "currency" => &template.currency, "notes" => &notes },
    ).map_err(map_mysql_err)?;
    let quote_id = conn.last_insert_id();
    for item in &items {
        conn.exec_drop(
            "INSERT INTO quote_items (quote_id, product_id, description, quantity, unit_price, discount, total) VALUES (:quote_id, :product_id, :description, :quantity, :unit_price, :discount, :total)",
            params! { "quote_id" => quote_id, "product_id" => item.product_id, "description" => &item.description, "quantity" => item.quantity, "unit_price" => item.unit_price, "discount" => item.discount, "total" => (item.quantity * item.unit_price - item.discount).max(0.0) },
        ).map_err(map_mysql_err)?;
    }
    let quote = Quote {
        id: quote_id,
        deal_id: payload.deal_id,
        template_id: Some(id),
        quote_number,
        issue_date,
        expiry_date: payload.expiry_date,
        subtotal,
        tax_rate: template.tax_rate,
        tax_amount,
        total_amount,
        currency: template.currency,
        status: "draft".into(),
        notes,
        created_by: None,
        created_at: None,
        updated_at: None,
    };
    state
        .broadcaster
        .notify("quote", ChangeAction::Created, Some(quote_id));
    Ok((StatusCode::CREATED, ApiResponse::success(quote)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn template_validation_rejects_bad_line_items() {
        assert!(validate_template_items(&[]).is_err());
        assert!(
            validate_template_items(&[QuoteTemplateItemDto {
                product_id: None,
                description: "Setup".into(),
                quantity: 1.0,
                unit_price: 0.0,
                discount: Some(0.0)
            }])
            .is_ok()
        );
    }

    #[test]
    fn notes_keep_terms_in_the_quote_snapshot() {
        assert_eq!(
            compose_notes(Some("Intro".into()), Some("30 days".into())),
            Some("Intro\n\nTerms:\n30 days".into())
        );
    }
}
