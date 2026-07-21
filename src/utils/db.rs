//! Database helpers used across handlers.
//!
//! Provides safe row extraction that never panics on `NULL` values, foreign-key
//! validation helpers, and a centralized MySQL error mapper that turns database
//! failures into the appropriate `AppError` variant.

use crate::error::AppError;
use mysql::params;
use mysql::prelude::*;

/// Safely extract a required value from a row.
///
/// Returns `AppError::Internal` if the column is missing or could not be
/// converted. This should be used for `NOT NULL` columns only.
pub fn take_required<T>(row: &mut mysql::Row, name: &str) -> Result<T, AppError>
where
    T: mysql::prelude::FromValue,
{
    row.take_opt::<T, _>(name)
        .ok_or_else(|| AppError::Internal(anyhow::anyhow!("missing column: {name}")))?
        .map_err(|e| AppError::Internal(anyhow::anyhow!("failed to read {name}: {e:?}")))
}

/// Safely extract an optional value from a row.
///
/// Handles `NULL` columns by returning `None` instead of panicking.
pub fn take_optional<T>(row: &mut mysql::Row, name: &str) -> Option<T>
where
    T: mysql::prelude::FromValue,
{
    row.take::<Option<T>, _>(name)?
}

/// Shorthand for extracting a required `String`.
pub fn req_str(row: &mut mysql::Row, name: &str) -> Result<String, AppError> {
    take_required::<String>(row, name)
}

/// Shorthand for extracting an optional `String` without panicking on `NULL`.
pub fn opt_str(row: &mut mysql::Row, name: &str) -> Option<String> {
    take_optional::<String>(row, name)
}

/// Shorthand for extracting a required `u64`.
pub fn req_u64(row: &mut mysql::Row, name: &str) -> Result<u64, AppError> {
    take_required::<u64>(row, name)
}

/// Shorthand for extracting an optional `u64` without panicking on `NULL`.
pub fn opt_u64(row: &mut mysql::Row, name: &str) -> Option<u64> {
    take_optional::<u64>(row, name)
}

/// Shorthand for extracting a required `i32`.
pub fn req_i32(row: &mut mysql::Row, name: &str) -> Result<i32, AppError> {
    take_required::<i32>(row, name)
}

/// Shorthand for extracting an optional `i32` without panicking on `NULL`.
pub fn opt_i32(row: &mut mysql::Row, name: &str) -> Option<i32> {
    take_optional::<i32>(row, name)
}

/// Shorthand for extracting a required `f64`.
pub fn req_f64(row: &mut mysql::Row, name: &str) -> Result<f64, AppError> {
    take_required::<f64>(row, name)
}

/// Shorthand for extracting an optional `f64` without panicking on `NULL`.
pub fn opt_f64(row: &mut mysql::Row, name: &str) -> Option<f64> {
    take_optional::<f64>(row, name)
}

/// Shorthand for extracting a boolean stored as `TINYINT(1)`.
pub fn req_bool(row: &mut mysql::Row, name: &str) -> Result<bool, AppError> {
    take_required::<i8>(row, name).map(|v| v != 0)
}

/// Maps a raw MySQL error to the appropriate `AppError` variant.
///
/// Foreign-key and check constraint violations become `BadRequest` with a
/// human-readable message. Duplicate-key violations become `Conflict`. Anything
/// else is treated as an internal error.
pub fn map_mysql_err(e: mysql::Error) -> AppError {
    use mysql::Error;

    match &e {
        Error::MySqlError(err) => {
            let msg = err.message.clone();
            match err.code {
                1451 => AppError::BadRequest(
                    "Cannot delete or update this record because it is still referenced by other records."
                        .into(),
                ),
                1452 => AppError::BadRequest(
                    "Referenced record does not exist. Please check the provided identifiers."
                        .into(),
                ),
                1062 => AppError::Conflict(
                    "A record with the same unique value already exists.".into(),
                ),
                1048 => AppError::Validation(format!("Required field is missing: {msg}")),
                4025 | 3819 => AppError::Validation(format!("Constraint check failed: {msg}")),
                _ => {
                    crate::log_err!("MySQL error {}: {}", err.code, msg);
                    AppError::Internal(anyhow::anyhow!(e))
                }
            }
        }
        _ => AppError::Internal(anyhow::anyhow!(e)),
    }
}

/// Check whether a row with the given id exists in `table`.
pub fn record_exists(
    conn: &mut mysql::PooledConn,
    table: &str,
    id: u64,
) -> Result<bool, AppError> {
    let sql = format!("SELECT 1 FROM {table} WHERE id = :id LIMIT 1");
    let found: Option<u8> = conn
        .exec_first(&sql, params! { "id" => id })
        .map_err(map_mysql_err)?;
    Ok(found.is_some())
}

/// Validate that a foreign-key reference exists, returning a clear `BadRequest`
/// error if it does not.
pub fn validate_fk(
    conn: &mut mysql::PooledConn,
    table: &str,
    id: u64,
    field_name: &str,
) -> Result<(), AppError> {
    if record_exists(conn, table, id)? {
        Ok(())
    } else {
        Err(AppError::BadRequest(format!(
            "{field_name} does not refer to an existing {table} record"
        )))
    }
}

/// Validate an optional foreign-key reference.
pub fn validate_optional_fk(
    conn: &mut mysql::PooledConn,
    table: &str,
    id: Option<u64>,
    field_name: &str,
) -> Result<(), AppError> {
    if let Some(id) = id {
        validate_fk(conn, table, id, field_name)?;
    }
    Ok(())
}

/// Convenience: validate a contact reference.
pub fn validate_contact(
    conn: &mut mysql::PooledConn,
    id: u64,
    field_name: &str,
) -> Result<(), AppError> {
    validate_fk(conn, "contacts", id, field_name)
}

/// Convenience: validate a company reference.
pub fn validate_company(
    conn: &mut mysql::PooledConn,
    id: u64,
    field_name: &str,
) -> Result<(), AppError> {
    validate_fk(conn, "companies", id, field_name)
}

/// Convenience: validate a user reference.
pub fn validate_user(
    conn: &mut mysql::PooledConn,
    id: u64,
    field_name: &str,
) -> Result<(), AppError> {
    validate_fk(conn, "users", id, field_name)
}

/// Convenience: validate a deal reference.
pub fn validate_deal(
    conn: &mut mysql::PooledConn,
    id: u64,
    field_name: &str,
) -> Result<(), AppError> {
    validate_fk(conn, "deals", id, field_name)
}

/// Convenience: validate a deal stage reference.
pub fn validate_deal_stage(
    conn: &mut mysql::PooledConn,
    id: u64,
    field_name: &str,
) -> Result<(), AppError> {
    validate_fk(conn, "deal_stages", id, field_name)
}

/// Convenience: validate a product reference.
pub fn validate_product(
    conn: &mut mysql::PooledConn,
    id: u64,
    field_name: &str,
) -> Result<(), AppError> {
    validate_fk(conn, "products", id, field_name)
}

/// Convenience: validate a tag reference.
pub fn validate_tag(
    conn: &mut mysql::PooledConn,
    id: u64,
    field_name: &str,
) -> Result<(), AppError> {
    validate_fk(conn, "tags", id, field_name)
}

/// Convenience: validate a quote reference.
pub fn validate_quote(
    conn: &mut mysql::PooledConn,
    id: u64,
    field_name: &str,
) -> Result<(), AppError> {
    validate_fk(conn, "quotes", id, field_name)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mysql_err(code: u16, message: &str) -> mysql::Error {
        mysql::Error::MySqlError(mysql::MySqlError {
            code,
            message: message.into(),
            state: String::new(),
        })
    }

    #[test]
    fn map_mysql_err_maps_fk_violations() {
        assert!(matches!(
            map_mysql_err(mysql_err(1452, "fk fail")),
            AppError::BadRequest(_)
        ));
    }

    #[test]
    fn map_mysql_err_maps_duplicate_key() {
        assert!(matches!(
            map_mysql_err(mysql_err(1062, "dup")),
            AppError::Conflict(_)
        ));
    }

    #[test]
    fn map_mysql_err_maps_child_row_fk() {
        assert!(matches!(
            map_mysql_err(mysql_err(1451, "child row")),
            AppError::BadRequest(_)
        ));
    }

    #[test]
    fn map_mysql_err_maps_not_null_violation() {
        assert!(matches!(
            map_mysql_err(mysql_err(1048, "null")),
            AppError::Validation(_)
        ));
    }
}
