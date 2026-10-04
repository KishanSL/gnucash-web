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
    State(pool): State<PgPool>,
    Json(payload): Json<SignupRequest>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
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

    let user_row = match user_row {
        Some(u) => u,
        None => {
            return Err((
                StatusCode::UNAUTHORIZED,
                Json(json!({ "error": "Invalid username or password" })),
            ));
        }
    };

    let valid = bcrypt::verify(password, &user_row.password_hash).unwrap_or(false);
    if !valid {
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(json!({ "error": "Invalid username or password" })),
        ));
    }

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

/// Provisions standard GnuCash root accounts for new user
async fn provision_starter_accounts(pool: &PgPool, user_id: Uuid) -> Result<(), sqlx::Error> {
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
