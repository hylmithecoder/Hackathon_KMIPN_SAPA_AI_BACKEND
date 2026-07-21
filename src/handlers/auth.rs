use crate::error::AppError;
use crate::models::auth::{LoginDto, LoginResponse, RegisterDto, UpdateUserDto, User};
use crate::response::ApiResponse;
use crate::state::AppState;
use crate::ws::event::ChangeAction;
use axum::extract::FromRef;
use axum::{
    Json,
    extract::{FromRequestParts, OptionalFromRequestParts, Path, State},
    http::{StatusCode, request::Parts},
};
use mysql::params;
use mysql::prelude::*;
use uuid::Uuid;

const USER_COLUMNS: &str = "id, username, full_name, role, email, phone, photo_url, is_active";

type UserRow = (
    u64,
    String,
    String,
    String,
    String,
    Option<String>,
    Option<String>,
    i8,
);
type LoginRow = (
    u64,
    String,
    String,
    String,
    String,
    Option<String>,
    Option<String>,
    i8,
    String,
);

fn map_user(row: UserRow) -> User {
    let (id, username, full_name, role, email, phone, photo_url, is_active) = row;
    User {
        id,
        username,
        full_name,
        role,
        email,
        phone,
        photo_url,
        is_active: is_active != 0,
    }
}

fn validate_role(role: &str) -> Result<(), AppError> {
    match role {
        "admin" | "sales" | "support" | "manager" => Ok(()),
        _ => Err(AppError::Validation(format!("invalid role: {role}"))),
    }
}

// Extractor to authenticate users via Bearer token
impl<S> FromRequestParts<S> for User
where
    S: Send + Sync,
    AppState: FromRef<S>,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let app_state = AppState::from_ref(state);

        let auth_header = parts
            .headers
            .get(axum::http::header::AUTHORIZATION)
            .and_then(|h| h.to_str().ok())
            .ok_or(AppError::Unauthorized)?;

        if !auth_header.starts_with("Bearer ") {
            return Err(AppError::Unauthorized);
        }

        let token = &auth_header[7..];

        let mut conn = app_state
            .pool
            .get_conn()
            .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

        let user: Option<UserRow> = conn
            .exec_first(
                format!(
                    "SELECT {USER_COLUMNS} FROM users u \
                     JOIN user_tokens t ON u.id = t.user_id \
                     WHERE t.token = :token AND u.is_active = 1"
                ),
                params! { "token" => token },
            )
            .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

        match user {
            Some(row) => Ok(map_user(row)),
            None => Err(AppError::Unauthorized),
        }
    }
}

// Optional extractor for public endpoints
impl<S> OptionalFromRequestParts<S> for User
where
    S: Send + Sync,
    AppState: FromRef<S>,
{
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &S,
    ) -> Result<Option<Self>, Self::Rejection> {
        Ok(
            <User as FromRequestParts<S>>::from_request_parts(parts, state)
                .await
                .ok(),
        )
    }
}

pub async fn login(
    State(state): State<AppState>,
    Json(payload): Json<LoginDto>,
) -> Result<ApiResponse<LoginResponse>, AppError> {
    let mut conn = state
        .pool
        .get_conn()
        .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

    let db_user: Option<LoginRow> = conn
        .exec_first(
            format!(
                "SELECT {USER_COLUMNS}, u.password FROM users u \
                 WHERE u.username = :ident OR u.email = :ident \
                 ORDER BY (u.username = :ident) DESC LIMIT 1"
            ),
            params! { "ident" => &payload.username },
        )
        .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

    let (id, username, full_name, role, email, phone, photo_url, is_active, hashed_password) =
        match db_user {
            Some(u) => u,
            None => return Err(AppError::Unauthorized),
        };

    let password_ok = bcrypt::verify(&payload.password, &hashed_password).unwrap_or(false);
    if !password_ok {
        return Err(AppError::Unauthorized);
    }

    let token = Uuid::new_v4().to_string();

    conn.exec_drop(
        "INSERT INTO user_tokens (user_id, token) VALUES (:user_id, :token)",
        params! {
            "user_id" => id,
            "token" => &token,
        },
    )
    .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

    Ok(ApiResponse::success(LoginResponse {
        user: User {
            id,
            username,
            full_name,
            role,
            email,
            phone,
            photo_url,
            is_active: is_active != 0,
        },
        token,
    }))
}

pub async fn register(
    State(state): State<AppState>,
    Json(payload): Json<RegisterDto>,
) -> Result<(StatusCode, ApiResponse<User>), AppError> {
    validate_role(&payload.role)?;

    if payload.username.trim().is_empty() {
        return Err(AppError::Validation("username is required".into()));
    }
    if payload.password.len() < 6 {
        return Err(AppError::Validation(
            "password must be at least 6 characters".into(),
        ));
    }
    if payload.full_name.trim().is_empty() {
        return Err(AppError::Validation("full_name is required".into()));
    }
    if payload.email.trim().is_empty() {
        return Err(AppError::Validation("email is required".into()));
    }

    let mut conn = state
        .pool
        .get_conn()
        .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

    let hashed_pass =
        bcrypt::hash(&payload.password, 10).map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

    conn.exec_drop(
        "INSERT INTO users (username, password, full_name, role, email, phone) \
         VALUES (:username, :password, :full_name, :role, :email, :phone)",
        params! {
            "username" => payload.username.trim(),
            "password" => hashed_pass,
            "full_name" => payload.full_name.trim(),
            "role" => &payload.role,
            "email" => payload.email.trim(),
            "phone" => payload.phone.as_deref(),
        },
    )
    .map_err(|e| {
        crate::log_err!("Database error in register: {:?}", e);
        AppError::Conflict("username or email already exists".into())
    })?;

    let last_id = conn.last_insert_id();

    state
        .broadcaster
        .notify("user", ChangeAction::Created, Some(last_id));

    Ok((
        StatusCode::CREATED,
        ApiResponse::success(User {
            id: last_id,
            username: payload.username,
            full_name: payload.full_name,
            role: payload.role,
            email: payload.email,
            phone: payload.phone,
            photo_url: None,
            is_active: true,
        }),
    ))
}

pub async fn list_users(
    State(state): State<AppState>,
    _user: User,
) -> Result<ApiResponse<Vec<User>>, AppError> {
    let mut conn = state
        .pool
        .get_conn()
        .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

    let users = conn
        .query_map(
            format!("SELECT {USER_COLUMNS} FROM users ORDER BY id"),
            map_user,
        )
        .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

    Ok(ApiResponse::success(users))
}

pub async fn update_user(
    Path(id): Path<u64>,
    State(state): State<AppState>,
    Json(payload): Json<UpdateUserDto>,
) -> Result<ApiResponse<User>, AppError> {
    let mut conn = state
        .pool
        .get_conn()
        .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

    let existing: Option<UserRow> = conn
        .exec_first(
            format!("SELECT {USER_COLUMNS} FROM users WHERE id = :id"),
            params! { "id" => id },
        )
        .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

    let Some(row) = existing else {
        return Err(AppError::NotFound);
    };

    let mut user = map_user(row);

    if let Some(username) = payload.username {
        let username = username.trim().to_string();
        if username.is_empty() {
            return Err(AppError::Validation("username is required".into()));
        }
        user.username = username;
    }
    if let Some(full_name) = payload.full_name {
        user.full_name = full_name.trim().to_string();
    }
    if let Some(email) = payload.email {
        user.email = email.trim().to_string();
    }
    if payload.phone.is_some() {
        user.phone = payload.phone;
    }
    if let Some(role) = payload.role {
        validate_role(&role)?;
        user.role = role;
    }
    if let Some(is_active) = payload.is_active {
        user.is_active = is_active;
    }

    let hashed_pass = match payload.password.as_deref() {
        Some(p) if !p.is_empty() => {
            Some(bcrypt::hash(p, 10).map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?)
        }
        _ => None,
    };

    if let Some(hashed) = hashed_pass {
        conn.exec_drop(
            "UPDATE users SET username = :username, full_name = :full_name, email = :email, \
             phone = :phone, role = :role, is_active = :is_active, password = :password \
             WHERE id = :id",
            params! {
                "id" => id,
                "username" => &user.username,
                "full_name" => &user.full_name,
                "email" => &user.email,
                "phone" => &user.phone,
                "role" => &user.role,
                "is_active" => user.is_active as i8,
                "password" => hashed,
            },
        )
    } else {
        conn.exec_drop(
            "UPDATE users SET username = :username, full_name = :full_name, email = :email, \
             phone = :phone, role = :role, is_active = :is_active WHERE id = :id",
            params! {
                "id" => id,
                "username" => &user.username,
                "full_name" => &user.full_name,
                "email" => &user.email,
                "phone" => &user.phone,
                "role" => &user.role,
                "is_active" => user.is_active as i8,
            },
        )
    }
    .map_err(|e| {
        crate::log_err!("Database error in update_user: {:?}", e);
        AppError::Conflict("username or email already exists".into())
    })?;

    state
        .broadcaster
        .notify("user", ChangeAction::Updated, Some(id));

    Ok(ApiResponse::success(user))
}

pub async fn delete_user(
    Path(id): Path<u64>,
    State(state): State<AppState>,
) -> Result<StatusCode, AppError> {
    let mut conn = state
        .pool
        .get_conn()
        .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

    conn.exec_drop("DELETE FROM users WHERE id = :id", params! { "id" => id })
        .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

    if conn.affected_rows() > 0 {
        state
            .broadcaster
            .notify("user", ChangeAction::Deleted, Some(id));
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(AppError::NotFound)
    }
}

pub async fn logout(State(state): State<AppState>, user: User) -> Result<StatusCode, AppError> {
    let mut conn = state
        .pool
        .get_conn()
        .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

    conn.exec_drop(
        "DELETE FROM user_tokens WHERE user_id = :user_id",
        params! { "user_id" => user.id },
    )
    .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_roles_accepted() {
        assert!(validate_role("admin").is_ok());
        assert!(validate_role("sales").is_ok());
        assert!(validate_role("support").is_ok());
        assert!(validate_role("manager").is_ok());
    }

    #[test]
    fn invalid_role_rejected() {
        assert!(validate_role("hacker").is_err());
    }
}
