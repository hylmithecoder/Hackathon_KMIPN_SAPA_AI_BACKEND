use crate::database::scheme::{Contact, Company, Deal, DealDetail, DealDiscussion, DiscussionFile};
use crate::error::AppError;
use crate::models::deal::{CreateDealDto, DealStageMoveDto, UpdateDealDto};
use crate::response::ApiResponse;
use crate::state::AppState;
use crate::utils::db::{
    map_mysql_err, opt_str, opt_u64, req_f64, req_str, req_u64, validate_company,
    validate_contact, validate_deal_stage, validate_user,
};
use crate::ws::event::ChangeAction;
use axum::{
    Json,
    extract::{Multipart, Path, State},
    http::StatusCode,
};
use mysql::params;
use mysql::prelude::*;
use std::path::PathBuf;
use uuid::Uuid;

const DEAL_COLUMNS: &str = "d.id, d.title, d.contact_id, \
    CONCAT(c.first_name, ' ', IFNULL(c.last_name, '')) AS contact_name, \
    d.company_id, co.name AS company_name, d.stage_id, s.name AS stage_name, \
    d.owner_id, u.full_name AS owner_name, d.value, d.currency, \
    DATE_FORMAT(d.expected_close_date, '%Y-%m-%d') AS expected_close_date, \
    DATE_FORMAT(d.actual_close_date, '%Y-%m-%d') AS actual_close_date, \
    d.status, d.description, \
    DATE_FORMAT(d.created_at, '%Y-%m-%d %H:%i:%s') AS created_at, \
    DATE_FORMAT(d.updated_at, '%Y-%m-%d %H:%i:%s') AS updated_at";

fn row_to_deal(row: &mut mysql::Row) -> Result<Deal, AppError> {
    Ok(Deal {
        id: req_u64(row, "id")?,
        title: req_str(row, "title")?,
        contact_id: opt_u64(row, "contact_id"),
        contact_name: opt_str(row, "contact_name"),
        company_id: opt_u64(row, "company_id"),
        company_name: opt_str(row, "company_name"),
        stage_id: req_u64(row, "stage_id")?,
        stage_name: opt_str(row, "stage_name"),
        owner_id: opt_u64(row, "owner_id"),
        owner_name: opt_str(row, "owner_name"),
        value: req_f64(row, "value")?,
        currency: req_str(row, "currency")?,
        expected_close_date: opt_str(row, "expected_close_date"),
        actual_close_date: opt_str(row, "actual_close_date"),
        status: req_str(row, "status")?,
        description: opt_str(row, "description"),
        created_at: opt_str(row, "created_at"),
        updated_at: opt_str(row, "updated_at"),
    })
}

pub async fn list_deals(State(state): State<AppState>) -> Result<ApiResponse<Vec<Deal>>, AppError> {
    let mut conn = state
        .pool
        .get_conn()
        .map_err(map_mysql_err)?;

    let deals: Vec<Deal> = conn
        .query_map(
            format!(
                "SELECT {DEAL_COLUMNS} FROM deals d \
                 LEFT JOIN contacts c ON d.contact_id = c.id \
                 LEFT JOIN companies co ON d.company_id = co.id \
                 LEFT JOIN deal_stages s ON d.stage_id = s.id \
                 LEFT JOIN users u ON d.owner_id = u.id \
                 ORDER BY d.id DESC"
            ),
            |mut row: mysql::Row| row_to_deal(&mut row),
        )
        .map_err(map_mysql_err)?
        .into_iter()
        .collect::<Result<Vec<_>, _>>()?;

    Ok(ApiResponse::success(deals))
}

pub async fn create_deal(
    State(state): State<AppState>,
    Json(payload): Json<CreateDealDto>,
) -> Result<(StatusCode, ApiResponse<Deal>), AppError> {
    if payload.title.trim().is_empty() {
        return Err(AppError::Validation("title is required".into()));
    }

    let status = payload.status.unwrap_or_else(|| "open".to_string());
    let currency = payload.currency.unwrap_or_else(|| "IDR".to_string());
    let value = payload.value.unwrap_or(0.0);

    let mut conn = state
        .pool
        .get_conn()
        .map_err(map_mysql_err)?;

    let resolved_contact_id = if let Some(cid) = payload.contact_id {
        if validate_contact(&mut conn, cid, "contact_id").is_ok() {
            Some(cid)
        } else {
            None
        }
    } else {
        None
    };
    validate_deal_stage(&mut conn, payload.stage_id, "stage_id")?;
    if let Some(company_id) = payload.company_id {
        validate_company(&mut conn, company_id, "company_id")?;
    }
    if let Some(owner_id) = payload.owner_id {
        validate_user(&mut conn, owner_id, "owner_id")?;
    }

    conn.exec_drop(
        "INSERT INTO deals (title, contact_id, company_id, stage_id, owner_id, value, currency, expected_close_date, status, description) \
         VALUES (:title, :contact_id, :company_id, :stage_id, :owner_id, :value, :currency, :expected_close_date, :status, :description)",
        params! {
            "title" => payload.title.trim(),
            "contact_id" => resolved_contact_id,
            "company_id" => payload.company_id,
            "stage_id" => payload.stage_id,
            "owner_id" => payload.owner_id,
            "value" => value,
            "currency" => &currency,
            "expected_close_date" => payload.expected_close_date.as_deref(),
            "status" => &status,
            "description" => payload.description.as_deref(),
        },
    )
    .map_err(map_mysql_err)?;

    let last_id = conn.last_insert_id();
    let deal = Deal {
        id: last_id,
        title: payload.title,
        contact_id: resolved_contact_id,
        contact_name: None,
        company_id: payload.company_id,
        company_name: None,
        stage_id: payload.stage_id,
        stage_name: None,
        owner_id: payload.owner_id,
        owner_name: None,
        value,
        currency,
        expected_close_date: payload.expected_close_date,
        actual_close_date: None,
        status,
        description: payload.description,
        created_at: None,
        updated_at: None,
    };

    state
        .broadcaster
        .notify("deal", ChangeAction::Created, Some(last_id));

    Ok((StatusCode::CREATED, ApiResponse::success(deal)))
}

pub async fn get_deal(
    Path(id): Path<u64>,
    State(state): State<AppState>,
) -> Result<ApiResponse<Deal>, AppError> {
    let mut conn = state
        .pool
        .get_conn()
        .map_err(map_mysql_err)?;

    let deal: Option<Deal> = conn
        .exec_first(
            format!(
                "SELECT {DEAL_COLUMNS} FROM deals d \
                 LEFT JOIN contacts c ON d.contact_id = c.id \
                 LEFT JOIN companies co ON d.company_id = co.id \
                 LEFT JOIN deal_stages s ON d.stage_id = s.id \
                 LEFT JOIN users u ON d.owner_id = u.id \
                 WHERE d.id = :id"
            ),
            params! { "id" => id },
        )
        .map_err(map_mysql_err)?
        .map(|mut row: mysql::Row| row_to_deal(&mut row))
        .transpose()?;

    match deal {
        Some(d) => Ok(ApiResponse::success(d)),
        None => Err(AppError::NotFound),
    }
}

pub async fn update_deal(
    Path(id): Path<u64>,
    State(state): State<AppState>,
    Json(payload): Json<UpdateDealDto>,
) -> Result<ApiResponse<Deal>, AppError> {
    let mut conn = state
        .pool
        .get_conn()
        .map_err(map_mysql_err)?;

    let existing: Option<Deal> = conn
        .exec_first(
            format!(
                "SELECT {DEAL_COLUMNS} FROM deals d \
                 LEFT JOIN contacts c ON d.contact_id = c.id \
                 LEFT JOIN companies co ON d.company_id = co.id \
                 LEFT JOIN deal_stages s ON d.stage_id = s.id \
                 LEFT JOIN users u ON d.owner_id = u.id \
                 WHERE d.id = :id"
            ),
            params! { "id" => id },
        )
        .map_err(map_mysql_err)?
        .map(|mut row: mysql::Row| row_to_deal(&mut row))
        .transpose()?;

    let Some(mut deal) = existing else {
        return Err(AppError::NotFound);
    };

    if let Some(title) = payload.title {
        if title.trim().is_empty() {
            return Err(AppError::Validation("title is required".into()));
        }
        deal.title = title;
    }
    if let Some(contact_id) = payload.contact_id {
        deal.contact_id = if validate_contact(&mut conn, contact_id, "contact_id").is_ok() {
            Some(contact_id)
        } else {
            None
        };
    }
    if let Some(company_id) = payload.company_id {
        validate_company(&mut conn, company_id, "company_id")?;
        deal.company_id = Some(company_id);
    }
    if let Some(stage_id) = payload.stage_id {
        validate_deal_stage(&mut conn, stage_id, "stage_id")?;
        deal.stage_id = stage_id;
    }
    if let Some(owner_id) = payload.owner_id {
        validate_user(&mut conn, owner_id, "owner_id")?;
        deal.owner_id = Some(owner_id);
    }
    if let Some(value) = payload.value {
        deal.value = value;
    }
    if let Some(currency) = payload.currency {
        deal.currency = currency;
    }
    if payload.expected_close_date.is_some() {
        deal.expected_close_date = payload.expected_close_date;
    }
    if payload.actual_close_date.is_some() {
        deal.actual_close_date = payload.actual_close_date;
    }
    if let Some(status) = payload.status {
        deal.status = status;
    }
    if payload.description.is_some() {
        deal.description = payload.description;
    }

    conn.exec_drop(
        "UPDATE deals SET title = :title, contact_id = :contact_id, company_id = :company_id, \
         stage_id = :stage_id, owner_id = :owner_id, value = :value, currency = :currency, \
         expected_close_date = :expected_close_date, actual_close_date = :actual_close_date, \
         status = :status, description = :description WHERE id = :id",
        params! {
            "id" => id,
            "title" => &deal.title,
            "contact_id" => deal.contact_id,
            "company_id" => deal.company_id,
            "stage_id" => deal.stage_id,
            "owner_id" => deal.owner_id,
            "value" => deal.value,
            "currency" => &deal.currency,
            "expected_close_date" => &deal.expected_close_date,
            "actual_close_date" => &deal.actual_close_date,
            "status" => &deal.status,
            "description" => &deal.description,
        },
    )
    .map_err(map_mysql_err)?;

    state
        .broadcaster
        .notify("deal", ChangeAction::Updated, Some(id));

    Ok(ApiResponse::success(deal))
}

pub async fn delete_deal(
    Path(id): Path<u64>,
    State(state): State<AppState>,
) -> Result<StatusCode, AppError> {
    let mut conn = state
        .pool
        .get_conn()
        .map_err(map_mysql_err)?;

    conn.exec_drop("DELETE FROM deals WHERE id = :id", params! { "id" => id })
        .map_err(map_mysql_err)?;

    if conn.affected_rows() > 0 {
        state
            .broadcaster
            .notify("deal", ChangeAction::Deleted, Some(id));
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(AppError::NotFound)
    }
}

pub async fn move_deal_stage(
    Path(id): Path<u64>,
    State(state): State<AppState>,
    Json(payload): Json<DealStageMoveDto>,
) -> Result<ApiResponse<Deal>, AppError> {
    let mut conn = state
        .pool
        .get_conn()
        .map_err(map_mysql_err)?;

    validate_deal_stage(&mut conn, payload.stage_id, "stage_id")?;

    // Verify the deal exists before updating; affected_rows() is unreliable here.
    let exists: Option<u8> = conn
        .exec_first(
            "SELECT 1 FROM deals WHERE id = :id",
            params! { "id" => id },
        )
        .map_err(map_mysql_err)?;
    if exists.is_none() {
        return Err(AppError::NotFound);
    }

    conn.exec_drop(
        "UPDATE deals SET stage_id = :stage_id WHERE id = :id",
        params! { "id" => id, "stage_id" => payload.stage_id },
    )
    .map_err(map_mysql_err)?;

    // Fetch the updated deal in the same connection so the result is consistent.
    let deal: Option<Deal> = conn
        .exec_first(
            format!(
                "SELECT {DEAL_COLUMNS} FROM deals d \
                 LEFT JOIN contacts c ON d.contact_id = c.id \
                 LEFT JOIN companies co ON d.company_id = co.id \
                 LEFT JOIN deal_stages s ON d.stage_id = s.id \
                 LEFT JOIN users u ON d.owner_id = u.id \
                 WHERE d.id = :id"
            ),
            params! { "id" => id },
        )
        .map_err(map_mysql_err)?
        .map(|mut row: mysql::Row| row_to_deal(&mut row))
        .transpose()?;

    let deal = deal.ok_or(AppError::NotFound)?;

    state
        .broadcaster
        .notify("deal", ChangeAction::Updated, Some(id));

    Ok(ApiResponse::success(deal))
}

fn row_to_contact_compact(row: &mut mysql::Row) -> Result<Contact, AppError> {
    Ok(Contact {
        id: req_u64(row, "id")?,
        first_name: req_str(row, "first_name")?,
        last_name: opt_str(row, "last_name"),
        email: opt_str(row, "email"),
        phone: opt_str(row, "phone"),
        job_title: opt_str(row, "job_title"),
        company_id: opt_u64(row, "company_id"),
        company_name: opt_str(row, "company_name"),
        source: opt_str(row, "source"),
        status: req_str(row, "status")?,
        assigned_to: opt_u64(row, "assigned_to"),
        description: opt_str(row, "description"),
        created_at: opt_str(row, "created_at"),
        updated_at: opt_str(row, "updated_at"),
    })
}

fn row_to_company_compact(row: &mut mysql::Row) -> Result<Company, AppError> {
    Ok(Company {
        id: req_u64(row, "id")?,
        name: req_str(row, "name")?,
        industry: opt_str(row, "industry"),
        website: opt_str(row, "website"),
        phone: opt_str(row, "phone"),
        email: opt_str(row, "email"),
        address: opt_str(row, "address"),
        city: opt_str(row, "city"),
        country: opt_str(row, "country"),
        description: opt_str(row, "description"),
        assigned_to: opt_u64(row, "assigned_to"),
        created_at: opt_str(row, "created_at"),
        updated_at: opt_str(row, "updated_at"),
    })
}

fn row_to_deal_discussion(row: &mut mysql::Row) -> Result<DealDiscussion, AppError> {
    Ok(DealDiscussion {
        id: req_u64(row, "id")?,
        deal_id: req_u64(row, "deal_id")?,
        user_id: opt_u64(row, "user_id"),
        author_name: opt_str(row, "author_name"),
        content: req_str(row, "content")?,
        files: Vec::new(),
        created_at: opt_str(row, "created_at"),
    })
}

fn row_to_discussion_file(row: &mut mysql::Row) -> Result<DiscussionFile, AppError> {
    Ok(DiscussionFile {
        id: req_u64(row, "id")?,
        discussion_id: req_u64(row, "discussion_id")?,
        file_name: req_str(row, "file_name")?,
        file_url: format!("/uploads/{}", req_str(row, "file_path")?),
        mime_type: opt_str(row, "mime_type"),
        file_size: req_u64(row, "file_size")?,
        created_at: opt_str(row, "created_at"),
    })
}

fn attach_discussion_files(
    conn: &mut mysql::PooledConn,
    deal_id: u64,
    discussions: &mut [DealDiscussion],
) -> Result<(), AppError> {
    if discussions.is_empty() {
        return Ok(());
    }

    let files: Vec<DiscussionFile> = conn
        .exec_map(
            "SELECT df.id, df.discussion_id, df.file_name, df.file_path, \
             df.mime_type, df.file_size, df.created_at FROM discussion_files df \
             INNER JOIN deal_discussions dd ON df.discussion_id = dd.id \
             WHERE dd.deal_id = :deal_id \
             ORDER BY df.created_at ASC",
            params! { "deal_id" => deal_id },
            |mut row: mysql::Row| row_to_discussion_file(&mut row),
        )
        .map_err(map_mysql_err)?
        .into_iter()
        .collect::<Result<Vec<_>, _>>()?;

    for file in files {
        if let Some(discussion) = discussions.iter_mut().find(|d| d.id == file.discussion_id) {
            discussion.files.push(file);
        }
    }

    Ok(())
}

pub async fn get_deal_detail(
    Path(id): Path<u64>,
    State(state): State<AppState>,
) -> Result<ApiResponse<DealDetail>, AppError> {
    let mut conn = state
        .pool
        .get_conn()
        .map_err(map_mysql_err)?;

    let deal: Option<Deal> = conn
        .exec_first(
            format!(
                "SELECT {DEAL_COLUMNS} FROM deals d \
                 LEFT JOIN contacts c ON d.contact_id = c.id \
                 LEFT JOIN companies co ON d.company_id = co.id \
                 LEFT JOIN deal_stages s ON d.stage_id = s.id \
                 LEFT JOIN users u ON d.owner_id = u.id \
                 WHERE d.id = :id"
            ),
            params! { "id" => id },
        )
        .map_err(map_mysql_err)?
        .map(|mut row: mysql::Row| row_to_deal(&mut row))
        .transpose()?;

    let deal = deal.ok_or(AppError::NotFound)?;

    let contact: Option<Contact> = if let Some(contact_id) = deal.contact_id {
        conn
            .exec_first(
                "SELECT c.id, c.first_name, c.last_name, c.email, c.phone, c.job_title, \
                 c.company_id, co.name AS company_name, c.source, c.status, c.assigned_to, c.description, \
                 c.created_at, c.updated_at FROM contacts c \
                 LEFT JOIN companies co ON c.company_id = co.id \
                 WHERE c.id = :id",
                params! { "id" => contact_id },
            )
            .map_err(map_mysql_err)?
            .map(|mut row: mysql::Row| row_to_contact_compact(&mut row))
            .transpose()?
    } else {
        None
    };

    let company: Option<Company> = if let Some(company_id) = deal.company_id {
        conn
            .exec_first(
                "SELECT id, name, industry, website, phone, email, address, city, country, \
                 description, assigned_to, created_at, updated_at FROM companies WHERE id = :id",
                params! { "id" => company_id },
            )
            .map_err(map_mysql_err)?
            .map(|mut row: mysql::Row| row_to_company_compact(&mut row))
            .transpose()?
    } else {
        None
    };

    Ok(ApiResponse::success(DealDetail {
        id: deal.id,
        title: deal.title,
        contact,
        company,
        stage_id: deal.stage_id,
        stage_name: deal.stage_name,
        owner_id: deal.owner_id,
        owner_name: deal.owner_name,
        value: deal.value,
        currency: deal.currency,
        expected_close_date: deal.expected_close_date,
        actual_close_date: deal.actual_close_date,
        status: deal.status,
        description: deal.description,
        created_at: deal.created_at,
        updated_at: deal.updated_at,
    }))
}

pub async fn list_deal_discussions(
    Path(id): Path<u64>,
    State(state): State<AppState>,
) -> Result<ApiResponse<Vec<DealDiscussion>>, AppError> {
    let mut conn = state
        .pool
        .get_conn()
        .map_err(map_mysql_err)?;

    let deal_exists: Option<u8> = conn
        .exec_first(
            "SELECT 1 FROM deals WHERE id = :id",
            params! { "id" => id },
        )
        .map_err(map_mysql_err)?;
    if deal_exists.is_none() {
        return Err(AppError::NotFound);
    }

    let mut discussions: Vec<DealDiscussion> = conn
        .exec_map(
            "SELECT dd.id, dd.deal_id, dd.user_id, u.full_name AS author_name, \
             dd.content, dd.created_at FROM deal_discussions dd \
             LEFT JOIN users u ON dd.user_id = u.id \
             WHERE dd.deal_id = :id \
             ORDER BY dd.created_at ASC",
            params! { "id" => id },
            |mut row: mysql::Row| row_to_deal_discussion(&mut row),
        )
        .map_err(map_mysql_err)?
        .into_iter()
        .collect::<Result<Vec<_>, _>>()?;

    attach_discussion_files(&mut conn, id, &mut discussions)?;

    Ok(ApiResponse::success(discussions))
}

pub async fn create_deal_discussion(
    Path(id): Path<u64>,
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> Result<(StatusCode, ApiResponse<DealDiscussion>), AppError> {
    let mut content = String::new();
    let mut staged_files: Vec<(String, Option<String>, Vec<u8>)> = Vec::new();

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| AppError::BadRequest(format!("failed to read multipart: {e}")))?
    {
        let name = field.name().unwrap_or("").to_string();
        match name.as_str() {
            "content" => {
                content = field
                    .text()
                    .await
                    .map_err(|e| AppError::BadRequest(format!("failed to read content: {e}")))?;
            }
            "files" | "file" => {
                let file_name = field
                    .file_name()
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| "upload.bin".to_string());
                let content_type = field.content_type().map(|s| s.to_string());
                let data = field
                    .bytes()
                    .await
                    .map_err(|e| AppError::BadRequest(format!("failed to read file: {e}")))?;
                staged_files.push((file_name, content_type, data.to_vec()));
            }
            _ => {}
        }
    }

    if content.trim().is_empty() && staged_files.is_empty() {
        return Err(AppError::Validation("content or file is required".into()));
    }

    let mut conn = state
        .pool
        .get_conn()
        .map_err(map_mysql_err)?;

    let deal_exists: Option<u8> = conn
        .exec_first(
            "SELECT 1 FROM deals WHERE id = :id",
            params! { "id" => id },
        )
        .map_err(map_mysql_err)?;
    if deal_exists.is_none() {
        return Err(AppError::NotFound);
    }

    // TODO: replace with authenticated user id once auth is wired.
    let user_id: Option<u64> = None;

    conn.exec_drop(
        "INSERT INTO deal_discussions (deal_id, user_id, content) \
         VALUES (:deal_id, :user_id, :content)",
        params! {
            "deal_id" => id,
            "user_id" => user_id,
            "content" => content.trim(),
        },
    )
    .map_err(map_mysql_err)?;

    let last_id = conn.last_insert_id();

    let mut saved_files = Vec::new();
    let base_dir = PathBuf::from("uploads/discussions").join(last_id.to_string());
    const MAX_FILE_SIZE: usize = 10 * 1024 * 1024;

    for (file_name, content_type, data) in staged_files {
        let file_size = data.len() as u64;
        if file_size as usize > MAX_FILE_SIZE {
            return Err(AppError::BadRequest(format!(
                "file '{}' exceeds 10 MB limit",
                file_name
            )));
        }

        let ext = PathBuf::from(&file_name)
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| format!(".{}", e))
            .unwrap_or_default();
        let stored_name = format!("{}{}", Uuid::new_v4(), ext);
        let relative_path = format!("discussions/{}/{}", last_id, stored_name);
        let full_path = base_dir.join(&stored_name);

        if let Some(parent) = full_path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| AppError::Internal(anyhow::anyhow!("create upload dir: {e}")))?;
        }
        tokio::fs::write(&full_path, &data)
            .await
            .map_err(|e| AppError::Internal(anyhow::anyhow!("write upload file: {e}")))?;

        conn.exec_drop(
            "INSERT INTO discussion_files (discussion_id, file_name, file_path, mime_type, file_size) \
             VALUES (:discussion_id, :file_name, :file_path, :mime_type, :file_size)",
            params! {
                "discussion_id" => last_id,
                "file_name" => &file_name,
                "file_path" => &relative_path,
                "mime_type" => content_type.as_deref(),
                "file_size" => file_size,
            },
        )
        .map_err(map_mysql_err)?;

        let file_id = conn.last_insert_id();
        saved_files.push(DiscussionFile {
            id: file_id,
            discussion_id: last_id,
            file_name,
            file_url: format!("/uploads/{}", relative_path),
            mime_type: content_type,
            file_size,
            created_at: None,
        });
    }

    let discussion: Option<DealDiscussion> = conn
        .exec_first(
            "SELECT dd.id, dd.deal_id, dd.user_id, u.full_name AS author_name, \
             dd.content, dd.created_at FROM deal_discussions dd \
             LEFT JOIN users u ON dd.user_id = u.id \
             WHERE dd.id = :id",
            params! { "id" => last_id },
        )
        .map_err(map_mysql_err)?
        .map(|mut row: mysql::Row| row_to_deal_discussion(&mut row))
        .transpose()?;

    let mut discussion = discussion.ok_or_else(|| {
        AppError::Internal(anyhow::anyhow!("failed to read created discussion"))
    })?;
    discussion.files = saved_files;

    state
        .broadcaster
        .notify("deal_discussion", ChangeAction::Created, Some(last_id));

    Ok((StatusCode::CREATED, ApiResponse::success(discussion)))
}

