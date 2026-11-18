use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use chrono::{Duration, Utc};
use serde_json::json;
use sqlx::PgPool;
use subtle::ConstantTimeEq;
use uuid::Uuid;

use crate::auth::{
    api_token_prefix, create_jwt, generate_api_token, hash_api_token, require_session,
    ApiTokenSummary, AuthResponse, AuthUser, LoginRequest, SignupRequest, User, UserWithPassword,
};
use crate::handlers::internal_error;

fn verify_admin_key(header_key: Option<&str>) -> bool {
    let header_key = match header_key {
        Some(k) if !k.is_empty() => k,
        _ => return false,
    };
    let expected_key = std::env::var("ADMIN_API_KEY")
        .ok()
        .map(|k| k.trim().to_string())
        .filter(|k| !k.is_empty())
        .unwrap_or_else(crate::auth::get_jwt_secret);

    if header_key.len() != expected_key.len() {
        return false;
    }
    header_key.as_bytes().ct_eq(expected_key.as_bytes()).into()
}

async fn hash_password_async(password: String) -> Result<String, (StatusCode, Json<serde_json::Value>)> {
    tokio::task::spawn_blocking(move || bcrypt::hash(password, bcrypt::DEFAULT_COST))
        .await
        .map_err(|e| internal_error("Password hashing task failed", e))?
        .map_err(|e| internal_error("Password hashing failed", e))
}

async fn verify_password_async(password: String, hash: String) -> bool {
    tokio::task::spawn_blocking(move || bcrypt::verify(password, &hash).unwrap_or(false))
        .await
        .unwrap_or(false)
}

pub async fn signup(
    headers: axum::http::HeaderMap,
    State(pool): State<PgPool>,
    Json(payload): Json<SignupRequest>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let admin_key = headers
        .get("x-admin-key")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.trim());

    let is_authorized_admin = verify_admin_key(admin_key);

    let allow_signup = std::env::var("ALLOW_SIGNUP")
        .map(|v| v == "true" || v == "1")
        .unwrap_or(false);

    if !allow_signup && !is_authorized_admin {
        return Err((
            StatusCode::FORBIDDEN,
            Json(json!({ "error": "Public registration is disabled on this server. Please contact the administrator for access." })),
        ));
    }

    let username = payload.username.trim();
    let email = payload.email.trim().to_lowercase();
    let password = payload.password;

    if username.len() < 3 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "Username must be at least 3 characters long" })),
        ));
    }

    if !email.contains('@') || !email.contains('.') {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "Please provide a valid email address" })),
        ));
    }

    if password.len() < 6 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "Password must be at least 6 characters long" })),
        ));
    }

    if password.len() > 72 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "Password cannot exceed 72 characters" })),
        ));
    }

    // Check if username or email already exists
    let existing = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM users WHERE LOWER(username) = LOWER($1) OR LOWER(email) = $2",
    )
    .bind(username)
    .bind(&email)
    .fetch_one(&pool)
    .await
    .map_err(|e| internal_error("Database error checking user", e))?;

    if existing > 0 {
        return Err((
            StatusCode::CONFLICT,
            Json(json!({ "error": "A user with this username or email already exists" })),
        ));
    }

    let password_hash = hash_password_async(password).await?;

    let user = sqlx::query_as::<_, User>(
        r#"INSERT INTO users (username, email, password_hash)
           VALUES ($1, $2, $3)
           RETURNING id, username, email, token_version, created_at, updated_at"#
    )
    .bind(username)
    .bind(&email)
    .bind(&password_hash)
    .fetch_one(&pool)
    .await
    .map_err(|e| internal_error("Failed to create user", e))?;

    // Provision standard initial Chart of Accounts for new user
    if let Err(e) = provision_starter_accounts(&pool, user.id).await {
        tracing::error!("Failed to provision starter accounts for user {}: {}", user.id, e);
    }

    let token = create_jwt(user.id, &user.username, &user.email, user.token_version)
        .map_err(|e| internal_error("Failed to generate token", e))?;

    Ok((StatusCode::CREATED, Json(AuthResponse { token, user })))
}

pub async fn login(
    State(pool): State<PgPool>,
    Json(payload): Json<LoginRequest>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let identifier = payload.username.trim();
    let password = payload.password;

    if identifier.is_empty() || password.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "Username/email and password are required" })),
        ));
    }

    let user_row = sqlx::query_as::<_, UserWithPassword>(
        r#"SELECT id, username, email, password_hash, token_version, created_at, updated_at
           FROM users
           WHERE LOWER(username) = LOWER($1) OR LOWER(email) = LOWER($1)"#
    )
    .bind(identifier)
    .fetch_optional(&pool)
    .await
    .map_err(|e| internal_error("Database error during login", e))?;

    // Constant-time execution to prevent timing-based username enumeration
    // DUMMY_HASH is a valid bcrypt hash so verify performs identical work when user does not exist
    const DUMMY_HASH: &str = "$2b$12$yfdyhpFIw7xpXN2Fz9mv4OJH1E3CRg/wSi/eHQAy.kESbVIcWB8Jm";
    let hash_to_verify = match &user_row {
        Some(u) => u.password_hash.clone(),
        None => DUMMY_HASH.to_string(),
    };
    let valid = verify_password_async(password, hash_to_verify).await;

    let user_row = match user_row {
        Some(u) if valid => u,
        _ => {
            return Err((
                StatusCode::UNAUTHORIZED,
                Json(json!({ "error": "Invalid username or password" })),
            ));
        }
    };

    let user = User {
        id: user_row.id,
        username: user_row.username,
        email: user_row.email,
        token_version: user_row.token_version,
        created_at: user_row.created_at,
        updated_at: user_row.updated_at,
    };

    let token = create_jwt(user.id, &user.username, &user.email, user.token_version)
        .map_err(|e| internal_error("Failed to generate token", e))?;

    Ok(Json(AuthResponse { token, user }))
}

pub async fn get_current_user(
    auth: AuthUser,
    State(pool): State<PgPool>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let user = sqlx::query_as::<_, User>(
        "SELECT id, username, email, token_version, created_at, updated_at FROM users WHERE id = $1",
    )
    .bind(auth.id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| internal_error("Database error fetching user", e))?
    .ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "User not found" })),
        )
    })?;

    Ok(Json(user))
}

#[derive(Debug, serde::Deserialize)]
pub struct ResetPasswordRequest {
    pub username: String,
    pub new_password: Option<String>,
}

pub fn write_reset_notice(content: &str) {
    let mut written_paths = Vec::new();

    // 1. Env var explicit path if configured
    if let Ok(env_path) = std::env::var("PASSWORD_RESET_FILE") {
        if write_file_secure(&std::path::PathBuf::from(&env_path), content) {
            written_paths.push(env_path);
        }
    } else {
        // 2. Standard container security directory /app/security
        let sec_dir = std::path::Path::new("/app/security");
        if sec_dir.is_dir() || std::fs::create_dir_all(sec_dir).is_ok() {
            let p = sec_dir.join("password_reset.txt");
            if write_file_secure(&p, content) {
                written_paths.push(p.to_string_lossy().to_string());
            }
        } else {
            // 3. Fallback to local security directory
            let repo_sec_dir = std::path::Path::new("security");
            if repo_sec_dir.is_dir() || std::fs::create_dir_all(repo_sec_dir).is_ok() {
                let p = repo_sec_dir.join("password_reset.txt");
                if write_file_secure(&p, content) {
                    written_paths.push(p.to_string_lossy().to_string());
                }
            }
        }
    }

    tracing::info!(
        "SECURITY: Password reset notice written to: {:?}",
        written_paths
    );
}

fn write_file_secure(path: &std::path::Path, content: &str) -> bool {
    use std::io::Write;

    let mut options = std::fs::OpenOptions::new();
    options.create(true).write(true).truncate(true);

    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }

    match options.open(path) {
        Ok(mut file) => {
            if file.write_all(content.as_bytes()).is_ok() {
                file.flush().ok();
                true
            } else {
                false
            }
        }
        Err(_) => false,
    }
}

pub fn read_latest_temporary_password() -> Option<String> {
    let candidate_paths = vec![
        std::env::var("PASSWORD_RESET_FILE").unwrap_or_default(),
        "/app/security/password_reset.txt".to_string(),
        "security/password_reset.txt".to_string(),
    ];

    for p in candidate_paths {
        if p.is_empty() {
            continue;
        }
        if let Ok(content) = std::fs::read_to_string(&p) {
            for line in content.lines() {
                if line.starts_with("Temporary Pass  :") {
                    let pass = line.trim_start_matches("Temporary Pass  :").trim();
                    if !pass.is_empty() {
                        return Some(pass.to_string());
                    }
                }
            }
        }
    }
    None
}

pub async fn reset_password(
    headers: axum::http::HeaderMap,
    State(pool): State<PgPool>,
    Json(payload): Json<ResetPasswordRequest>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let admin_key = headers
        .get("x-admin-key")
        .and_then(|v| v.to_str().ok());

    let is_admin = verify_admin_key(admin_key);

    if !is_admin {
        return Err((
            StatusCode::FORBIDDEN,
            Json(json!({
                "error": "Public password reset is disabled. Please use the server CLI script (./scripts/reset_password.sh) or contact the system administrator."
            })),
        ));
    }

    let identifier = payload.username.trim();

    if identifier.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "Username or email is required" })),
        ));
    }

    // 1. Check if user exists
    let user = sqlx::query_as::<_, User>(
        r#"SELECT id, username, email, token_version, created_at, updated_at
           FROM users
           WHERE LOWER(username) = LOWER($1) OR LOWER(email) = LOWER($1)"#
    )
    .bind(identifier)
    .fetch_optional(&pool)
    .await
    .map_err(|e| internal_error("Database error finding user for reset", e))?;

    let user = match user {
        Some(u) => u,
        None => {
            return Err((
                StatusCode::NOT_FOUND,
                Json(json!({ "error": "User not found" })),
            ));
        }
    };

    // 2. Generate secure temporary password or use admin-provided password
    let (target_password, is_temporary) = match &payload.new_password {
        Some(p) => {
            if p.len() < 6 {
                return Err((
                    StatusCode::BAD_REQUEST,
                    Json(json!({ "error": "Password must be at least 6 characters long" })),
                ));
            }
            if p.len() > 72 {
                return Err((
                    StatusCode::BAD_REQUEST,
                    Json(json!({ "error": "Password cannot exceed 72 characters" })),
                ));
            }
            (p.clone(), false)
        }
        None => {
            let random_part1 = &Uuid::new_v4().simple().to_string()[..8];
            let random_part2 = &Uuid::new_v4().simple().to_string()[..8];
            (format!("TempPass-{}-{}", random_part1, random_part2), true)
        }
    };

    let password_hash = hash_password_async(target_password.clone()).await?;

    // An admin reset implies the account may be compromised: revoke every session JWT (SEC-5) and
    // every API token (SEC-20) for this user in one transaction.
    let mut tx = pool
        .begin()
        .await
        .map_err(|e| internal_error("Database error starting password reset", e))?;

    sqlx::query(
        r#"UPDATE users
           SET password_hash = $1, token_version = token_version + 1, updated_at = NOW()
           WHERE id = $2"#
    )
    .bind(password_hash)
    .bind(user.id)
    .execute(&mut *tx)
    .await
    .map_err(|e| internal_error("Database error updating password", e))?;

    sqlx::query(
        r#"UPDATE api_tokens
           SET revoked_at = NOW()
           WHERE user_id = $1 AND revoked_at IS NULL"#
    )
    .bind(user.id)
    .execute(&mut *tx)
    .await
    .map_err(|e| internal_error("Database error revoking API tokens", e))?;

    tx.commit()
        .await
        .map_err(|e| internal_error("Database error committing password reset", e))?;

    // 3. Write new password notice to secure file on server only if temporary password was generated
    if is_temporary {
        let timestamp = chrono::Utc::now().to_rfc3339();
        let notice_title = "GNUCASH WEB - TEMPORARY PASSWORD RESET NOTICE";
        let pass_label = "Temporary Pass";
        let notice_content = format!(
r#"================================================================================
{}
================================================================================
Timestamp (UTC) : {}
Username        : {}
Email           : {}
{}  : {}
================================================================================
Next Steps:
1. Log in to GnuCash Web with your username and this temporary password.
2. In the top-right user menu, select "Change Password" to set your permanent password.
3. Delete this file after successful login.
================================================================================
"#,
            notice_title, timestamp, user.username, user.email, pass_label, target_password
        );

        write_reset_notice(&notice_content);
    }

    Ok(Json(json!({
        "success": true,
        "message": "Password reset successfully.",
        "username": user.username,
        "temp_password": target_password,
        "is_temporary": is_temporary
    })))
}

#[derive(Debug, serde::Deserialize)]
pub struct ChangePasswordRequest {
    pub current_password: String,
    pub new_password: String,
}

pub async fn change_password(
    auth: AuthUser,
    State(pool): State<PgPool>,
    Json(payload): Json<ChangePasswordRequest>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    require_session(&auth)?;

    let current_password = payload.current_password;
    let new_password = payload.new_password;

    if current_password.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "Current password is required" })),
        ));
    }

    if new_password.len() < 6 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "New password must be at least 6 characters long" })),
        ));
    }

    if new_password.len() > 72 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "New password cannot exceed 72 characters" })),
        ));
    }

    // Fetch existing password hash for this user
    let existing_hash: Option<String> = sqlx::query_scalar(
        "SELECT password_hash FROM users WHERE id = $1"
    )
    .bind(auth.id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| internal_error("Database error finding user for reset", e))?;

    let existing_hash = match existing_hash {
        Some(h) => h,
        None => {
            return Err((
                StatusCode::NOT_FOUND,
                Json(json!({ "error": "User not found" })),
            ));
        }
    };

    let valid = verify_password_async(current_password, existing_hash).await;
    if !valid {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "Incorrect current password" })),
        ));
    }

    let password_hash = hash_password_async(new_password).await?;

    // SEC-20: a user's own password change keeps API tokens working; it only revokes session JWTs
    // (the token_version bump below). Each API token is revoked individually.
    sqlx::query(
        "UPDATE users SET password_hash = $1, token_version = token_version + 1, updated_at = NOW() WHERE id = $2"
    )
    .bind(password_hash)
    .bind(auth.id)
    .execute(&pool)
    .await
    .map_err(|e| internal_error("Failed to update password", e))?;

    Ok(Json(json!({
        "success": true,
        "message": "Password changed successfully"
    })))
}

#[derive(Debug, serde::Deserialize)]
pub struct CreateApiTokenRequest {
    pub name: String,
    pub expires_in_days: Option<i64>,
}

/// Lists the caller's API tokens (SEC-19). Never returns the token or its hash.
pub async fn list_api_tokens(
    auth: AuthUser,
    State(pool): State<PgPool>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    require_session(&auth)?;

    let tokens = sqlx::query_as::<_, ApiTokenSummary>(
        r#"SELECT id, name, token_prefix, created_at, last_used_at, expires_at, revoked_at
           FROM api_tokens
           WHERE user_id = $1
           ORDER BY created_at DESC"#,
    )
    .bind(auth.id)
    .fetch_all(&pool)
    .await
    .map_err(|e| internal_error("Failed to list API tokens", e))?;

    Ok(Json(tokens))
}

/// Creates an API token for the caller (SEC-16, SEC-19). This is the only response that contains
/// the plaintext token, so the UI must show it once and never fetch it again.
pub async fn create_api_token(
    auth: AuthUser,
    State(pool): State<PgPool>,
    Json(payload): Json<CreateApiTokenRequest>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    require_session(&auth)?;

    let name = payload.name.trim();
    if name.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "Token name is required" })),
        ));
    }
    if name.chars().count() > 100 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "Token name cannot exceed 100 characters" })),
        ));
    }

    let expires_at = match payload.expires_in_days {
        Some(days) if days <= 0 => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(json!({ "error": "Expiry must be at least 1 day" })),
            ));
        }
        Some(days) if days > 3650 => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(json!({ "error": "Expiry cannot exceed 3650 days" })),
            ));
        }
        Some(days) => Some(Utc::now() + Duration::days(days)),
        None => None,
    };

    let token = generate_api_token();
    let token_prefix = api_token_prefix(&token);
    let token_hash = hash_api_token(&token);

    let row = sqlx::query_as::<_, (Uuid, chrono::DateTime<Utc>, Option<chrono::DateTime<Utc>>)>(
        r#"INSERT INTO api_tokens (user_id, name, token_prefix, token_hash, expires_at)
           VALUES ($1, $2, $3, $4, $5)
           RETURNING id, created_at, expires_at"#,
    )
    .bind(auth.id)
    .bind(name)
    .bind(&token_prefix)
    .bind(&token_hash)
    .bind(expires_at)
    .fetch_one(&pool)
    .await
    .map_err(|e| internal_error("Failed to create API token", e))?;

    Ok((
        StatusCode::CREATED,
        Json(json!({
            "id": row.0,
            "name": name,
            "token": token,
            "token_prefix": token_prefix,
            "created_at": row.1,
            "expires_at": row.2,
        })),
    ))
}

/// Revokes one of the caller's API tokens (SEC-19). Another user's token is a 404, not a 403, so
/// its existence is not leaked (SEC-10).
pub async fn revoke_api_token(
    auth: AuthUser,
    State(pool): State<PgPool>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    require_session(&auth)?;

    let result = sqlx::query(
        r#"UPDATE api_tokens
           SET revoked_at = NOW()
           WHERE id = $1 AND user_id = $2 AND revoked_at IS NULL"#,
    )
    .bind(id)
    .bind(auth.id)
    .execute(&pool)
    .await
    .map_err(|e| internal_error("Failed to revoke API token", e))?;

    if result.rows_affected() == 0 {
        return Err((
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "Token not found" })),
        ));
    }

    Ok(Json(json!({ "success": true })))
}

/// Provisions standard GnuCash root accounts for new user
pub async fn provision_starter_accounts(pool: &PgPool, user_id: Uuid) -> Result<(), sqlx::Error> {
    // 1. Assets
    let assets_id = sqlx::query_scalar::<_, Uuid>(
        r#"INSERT INTO accounts (name, code, account_type, currency, placeholder, user_id)
           VALUES ('Assets', '1000', 'asset', 'INR', true, $1) RETURNING id"#
    )
    .bind(user_id)
    .fetch_one(pool)
    .await?;

    let curr_assets_id = sqlx::query_scalar::<_, Uuid>(
        r#"INSERT INTO accounts (name, code, account_type, parent_id, currency, placeholder, user_id)
           VALUES ('Current Assets', '1010', 'asset', $1, 'INR', true, $2) RETURNING id"#
    )
    .bind(assets_id)
    .bind(user_id)
    .fetch_one(pool)
    .await?;

    sqlx::query(
        r#"INSERT INTO accounts (name, code, account_type, parent_id, currency, placeholder, user_id)
           VALUES
           ('Checking Account', '1011', 'bank', $1, 'INR', false, $2),
           ('Savings Account', '1012', 'bank', $1, 'INR', false, $2),
           ('Cash in Wallet', '1013', 'cash', $1, 'INR', false, $2)"#
    )
    .bind(curr_assets_id)
    .bind(user_id)
    .execute(pool)
    .await?;

    // 2. Liabilities
    let liab_id = sqlx::query_scalar::<_, Uuid>(
        r#"INSERT INTO accounts (name, code, account_type, currency, placeholder, user_id)
           VALUES ('Liabilities', '2000', 'liability', 'INR', true, $1) RETURNING id"#
    )
    .bind(user_id)
    .fetch_one(pool)
    .await?;

    sqlx::query(
        r#"INSERT INTO accounts (name, code, account_type, parent_id, currency, placeholder, user_id)
           VALUES ('Credit Card', '2010', 'credit', $1, 'INR', false, $2)"#
    )
    .bind(liab_id)
    .bind(user_id)
    .execute(pool)
    .await?;

    // 3. Equity
    let equity_id = sqlx::query_scalar::<_, Uuid>(
        r#"INSERT INTO accounts (name, code, account_type, currency, placeholder, user_id)
           VALUES ('Equity', '3000', 'equity', 'INR', true, $1) RETURNING id"#
    )
    .bind(user_id)
    .fetch_one(pool)
    .await?;

    sqlx::query(
        r#"INSERT INTO accounts (name, code, account_type, parent_id, currency, placeholder, user_id)
           VALUES ('Opening Balances', '3010', 'equity', $1, 'INR', false, $2)"#
    )
    .bind(equity_id)
    .bind(user_id)
    .execute(pool)
    .await?;

    // 4. Income
    let income_id = sqlx::query_scalar::<_, Uuid>(
        r#"INSERT INTO accounts (name, code, account_type, currency, placeholder, user_id)
           VALUES ('Income', '4000', 'income', 'INR', true, $1) RETURNING id"#
    )
    .bind(user_id)
    .fetch_one(pool)
    .await?;

    sqlx::query(
        r#"INSERT INTO accounts (name, code, account_type, parent_id, currency, placeholder, user_id)
           VALUES
           ('Salary', '4010', 'income', $1, 'INR', false, $2),
           ('Other Income', '4020', 'income', $1, 'INR', false, $2)"#
    )
    .bind(income_id)
    .bind(user_id)
    .execute(pool)
    .await?;

    // 5. Expenses
    let exp_id = sqlx::query_scalar::<_, Uuid>(
        r#"INSERT INTO accounts (name, code, account_type, currency, placeholder, user_id)
           VALUES ('Expenses', '5000', 'expense', 'INR', true, $1) RETURNING id"#
    )
    .bind(user_id)
    .fetch_one(pool)
    .await?;

    sqlx::query(
        r#"INSERT INTO accounts (name, code, account_type, parent_id, currency, placeholder, user_id)
           VALUES
           ('Groceries', '5010', 'expense', $1, 'INR', false, $2),
           ('Dining & Restaurants', '5020', 'expense', $1, 'INR', false, $2),
           ('Rent & Housing', '5030', 'expense', $1, 'INR', false, $2),
           ('Utilities', '5040', 'expense', $1, 'INR', false, $2),
           ('Transportation', '5050', 'expense', $1, 'INR', false, $2)"#
    )
    .bind(exp_id)
    .bind(user_id)
    .execute(pool)
    .await?;

    Ok(())
}
