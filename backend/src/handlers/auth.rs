use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

use crate::auth::{create_jwt, AuthResponse, AuthUser, LoginRequest, SignupRequest, User, UserWithPassword};

pub async fn signup(
    headers: axum::http::HeaderMap,
    State(pool): State<PgPool>,
    Json(payload): Json<SignupRequest>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let admin_key = headers
        .get("x-admin-key")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.trim());

    let jwt_secret = crate::auth::get_jwt_secret();
    let is_authorized_admin = admin_key.map(|k| k == jwt_secret).unwrap_or(false);

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
    let password = payload.password.trim();

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

    // Check if username or email already exists
    let existing = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM users WHERE LOWER(username) = LOWER($1) OR LOWER(email) = $2",
    )
    .bind(username)
    .bind(&email)
    .fetch_one(&pool)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Database error checking user: {}", e) })),
        )
    })?;

    if existing > 0 {
        return Err((
            StatusCode::CONFLICT,
            Json(json!({ "error": "A user with this username or email already exists" })),
        ));
    }

    let password_hash = bcrypt::hash(password, bcrypt::DEFAULT_COST).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Password hashing failed: {}", e) })),
        )
    })?;

    let user = sqlx::query_as::<_, User>(
        r#"INSERT INTO users (username, email, password_hash)
           VALUES ($1, $2, $3)
           RETURNING id, username, email, created_at, updated_at"#
    )
    .bind(username)
    .bind(&email)
    .bind(&password_hash)
    .fetch_one(&pool)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Failed to create user: {}", e) })),
        )
    })?;

    // Provision standard initial Chart of Accounts for new user
    if let Err(e) = provision_starter_accounts(&pool, user.id).await {
        tracing::error!("Failed to provision starter accounts for user {}: {}", user.id, e);
    }

    let token = create_jwt(user.id, &user.username, &user.email).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Failed to generate token: {}", e) })),
        )
    })?;

    Ok((StatusCode::CREATED, Json(AuthResponse { token, user })))
}

pub async fn login(
    State(pool): State<PgPool>,
    Json(payload): Json<LoginRequest>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let identifier = payload.username.trim();
    let password = payload.password.trim();

    if identifier.is_empty() || password.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "Username/email and password are required" })),
        ));
    }

    let user_row = sqlx::query_as::<_, UserWithPassword>(
        r#"SELECT id, username, email, password_hash, created_at, updated_at
           FROM users
           WHERE LOWER(username) = LOWER($1) OR LOWER(email) = LOWER($1)"#
    )
    .bind(identifier)
    .fetch_optional(&pool)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Database error: {}", e) })),
        )
    })?;

    // Constant-time execution to prevent timing-based username enumeration
    // DUMMY_HASH is a valid bcrypt hash so verify performs identical work when user does not exist
    const DUMMY_HASH: &str = "$2b$12$yfdyhpFIw7xpXN2Fz9mv4OJH1E3CRg/wSi/eHQAy.kESbVIcWB8Jm";
    let hash_to_verify = match &user_row {
        Some(u) => &u.password_hash,
        None => DUMMY_HASH,
    };
    let valid = bcrypt::verify(password, hash_to_verify).unwrap_or(false);

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
        created_at: user_row.created_at,
        updated_at: user_row.updated_at,
    };

    let token = create_jwt(user.id, &user.username, &user.email).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Failed to generate token: {}", e) })),
        )
    })?;

    Ok(Json(AuthResponse { token, user }))
}

pub async fn get_current_user(
    auth: AuthUser,
    State(pool): State<PgPool>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let user = sqlx::query_as::<_, User>(
        "SELECT id, username, email, created_at, updated_at FROM users WHERE id = $1",
    )
    .bind(auth.id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Database error: {}", e) })),
        )
    })?
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
    pub new_password: Option<String>, // Deprecated / ignored; temporary password is generated on server
}

pub fn write_reset_notice(content: &str, _temp_password: &str) {
    let mut written_paths = Vec::new();

    // 1. Env var explicit path if configured
    if let Ok(env_path) = std::env::var("PASSWORD_RESET_FILE") {
        if write_file_secure(&std::path::PathBuf::from(&env_path), content) {
            written_paths.push(env_path);
        }
    }

    // 2. Container mapped security directory /app/security
    let sec_dir = std::path::Path::new("/app/security");
    if sec_dir.is_dir() || std::fs::create_dir_all(sec_dir).is_ok() {
        let p = sec_dir.join("password_reset.txt");
        if write_file_secure(&p, content) {
            written_paths.push(p.to_string_lossy().to_string());
        }
    }

    // 3. /app directory
    let app_dir = std::path::Path::new("/app");
    if app_dir.is_dir() {
        let p = app_dir.join("password_reset.txt");
        if write_file_secure(&p, content) {
            written_paths.push(p.to_string_lossy().to_string());
        }
    }

    // 4. Local relative password_reset.txt (for local development & tests)
    let local_path = std::path::PathBuf::from("password_reset.txt");
    if write_file_secure(&local_path, content) {
        written_paths.push("password_reset.txt".to_string());
    }

    // 5. Host repository security dir if available relative to current dir
    let repo_sec_dir = std::path::Path::new("security");
    if repo_sec_dir.is_dir() {
        let p = repo_sec_dir.join("password_reset.txt");
        if write_file_secure(&p, content) {
            written_paths.push(p.to_string_lossy().to_string());
        }
    }

    // 6. /tmp directory as fallback
    let tmp_path = std::path::PathBuf::from("/tmp/gnucash_password_reset.txt");
    if write_file_secure(&tmp_path, content) {
        written_paths.push("/tmp/gnucash_password_reset.txt".to_string());
    }

    tracing::warn!(
        "SECURITY: Temporary password generated for user and saved to server file(s): {:?}",
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
        "password_reset.txt".to_string(),
        "security/password_reset.txt".to_string(),
        "/app/security/password_reset.txt".to_string(),
        "/app/password_reset.txt".to_string(),
        "/tmp/gnucash_password_reset.txt".to_string(),
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
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    let jwt_secret = crate::auth::get_jwt_secret();
    let is_admin = !admin_key.is_empty() && admin_key == jwt_secret;

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
        r#"SELECT id, username, email, created_at, updated_at
           FROM users
           WHERE LOWER(username) = LOWER($1) OR LOWER(email) = LOWER($1)"#
    )
    .bind(identifier)
    .fetch_optional(&pool)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Database error: {}", e) })),
        )
    })?;

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
        Some(p) if p.trim().len() >= 6 => (p.trim().to_string(), false),
        _ => {
            let random_part1 = &Uuid::new_v4().simple().to_string()[..8];
            let random_part2 = &Uuid::new_v4().simple().to_string()[..8];
            (format!("TempPass-{}-{}", random_part1, random_part2), true)
        }
    };

    let password_hash = bcrypt::hash(&target_password, bcrypt::DEFAULT_COST).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Password hashing failed: {}", e) })),
        )
    })?;

    sqlx::query(
        r#"UPDATE users
           SET password_hash = $1, updated_at = NOW()
           WHERE id = $2"#
    )
    .bind(password_hash)
    .bind(user.id)
    .execute(&pool)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Database error updating password: {}", e) })),
        )
    })?;

    // 3. Write new password notice to secure file on server
    let timestamp = chrono::Utc::now().to_rfc3339();
    let notice_title = if is_temporary {
        "GNUCASH WEB - TEMPORARY PASSWORD RESET NOTICE"
    } else {
        "GNUCASH WEB - ADMIN PASSWORD RESET NOTICE"
    };
    let pass_label = if is_temporary { "Temporary Pass" } else { "New Password  " };
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
1. Log in to GnuCash Web with your username and this password.
2. In the top-right user menu, select "Change Password" to set your permanent password if needed.
3. Delete this file after successful login.
================================================================================
"#,
        notice_title, timestamp, user.username, user.email, pass_label, target_password
    );

    write_reset_notice(&notice_content, &target_password);

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
    pub current_password: Option<String>,
    pub new_password: String,
}

pub async fn change_password(
    auth: AuthUser,
    State(pool): State<PgPool>,
    Json(payload): Json<ChangePasswordRequest>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let new_password = payload.new_password.trim();

    if new_password.len() < 6 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "New password must be at least 6 characters long" })),
        ));
    }

    // Fetch existing password hash for this user
    let existing_hash: Option<String> = sqlx::query_scalar(
        "SELECT password_hash FROM users WHERE id = $1"
    )
    .bind(auth.id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Database error: {}", e) })),
        )
    })?;

    let existing_hash = match existing_hash {
        Some(h) => h,
        None => {
            return Err((
                StatusCode::NOT_FOUND,
                Json(json!({ "error": "User not found" })),
            ));
        }
    };

    // If current_password was supplied, verify it
    if let Some(curr) = payload.current_password {
        let curr = curr.trim();
        if !curr.is_empty() {
            let valid = bcrypt::verify(curr, &existing_hash).unwrap_or(false);
            if !valid {
                return Err((
                    StatusCode::BAD_REQUEST,
                    Json(json!({ "error": "Incorrect current password" })),
                ));
            }
        }
    }

    let password_hash = bcrypt::hash(new_password, bcrypt::DEFAULT_COST).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Password hashing failed: {}", e) })),
        )
    })?;

    sqlx::query(
        "UPDATE users SET password_hash = $1, updated_at = NOW() WHERE id = $2"
    )
    .bind(password_hash)
    .bind(auth.id)
    .execute(&pool)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Failed to update password: {}", e) })),
        )
    })?;

    Ok(Json(json!({
        "success": true,
        "message": "Password changed successfully"
    })))
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
