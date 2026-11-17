mod tree;
mod validation;

pub use validation::{account_type_group, are_account_types_compatible, sanitize_account_code};

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use rust_decimal::Decimal;
use serde_json::json;
use sqlx::PgPool;
use std::collections::HashMap;
use uuid::Uuid;

use crate::auth::AuthUser;
use crate::handlers::{internal_error, ApiError};
use crate::models::{
    calculate_display_balance, Account, AccountBalanceRow, AccountRow, CreateAccountRequest, UpdateAccountRequest,
};
use tree::{build_account_list, build_tree_structure};
use validation::{
    validate_account_name, validate_account_type, validate_currency, verify_children_compatible, verify_parent,
};

pub async fn list_accounts(
    auth: AuthUser,
    State(pool): State<PgPool>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let rows: Vec<AccountRow> = sqlx::query_as::<_, AccountRow>(
        "SELECT id, name, code, account_type, parent_id, currency, description, placeholder, hidden, commodity_fraction, created_at, updated_at
         FROM accounts WHERE user_id = $1 ORDER BY code ASC NULLS LAST, name ASC",
    )
    .bind(auth.id)
    .fetch_all(&pool)
    .await
    .map_err(|e| internal_error("Failed to list accounts", e))?;

    // Fetch direct balances for user's accounts
    let balances: Vec<AccountBalanceRow> = sqlx::query_as::<_, AccountBalanceRow>(
        r#"SELECT s.account_id, COALESCE(SUM(s.amount), 0) as balance
           FROM splits s
           JOIN accounts a ON s.account_id = a.id
           WHERE a.user_id = $1
           GROUP BY s.account_id"#,
    )
    .bind(auth.id)
    .fetch_all(&pool)
    .await
    .map_err(|e| internal_error("Failed to fetch account balances", e))?;

    let balance_map: HashMap<Uuid, Decimal> = balances.into_iter().map(|b| (b.account_id, b.balance)).collect();
    let accounts = build_account_list(rows, &balance_map);

    Ok(Json(accounts))
}

pub async fn get_account_tree(
    auth: AuthUser,
    State(pool): State<PgPool>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let rows: Vec<AccountRow> = sqlx::query_as::<_, AccountRow>(
        "SELECT id, name, code, account_type, parent_id, currency, description, placeholder, hidden, commodity_fraction, created_at, updated_at
         FROM accounts WHERE user_id = $1 ORDER BY code ASC NULLS LAST, name ASC",
    )
    .bind(auth.id)
    .fetch_all(&pool)
    .await
    .map_err(|e| internal_error("Failed to fetch accounts for tree", e))?;

    let balances: Vec<AccountBalanceRow> = sqlx::query_as::<_, AccountBalanceRow>(
        r#"SELECT s.account_id, COALESCE(SUM(s.amount), 0) as balance
           FROM splits s
           JOIN accounts a ON s.account_id = a.id
           WHERE a.user_id = $1
           GROUP BY s.account_id"#,
    )
    .bind(auth.id)
    .fetch_all(&pool)
    .await
    .map_err(|e| internal_error("Failed to fetch account tree balances", e))?;

    let balance_map: HashMap<Uuid, Decimal> = balances.into_iter().map(|b| (b.account_id, b.balance)).collect();
    let accounts = build_account_list(rows, &balance_map);

    let tree = build_tree_structure(accounts);
    Ok(Json(tree))
}

pub async fn get_account(
    auth: AuthUser,
    State(pool): State<PgPool>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let row = sqlx::query_as::<_, AccountRow>(
        "SELECT id, name, code, account_type, parent_id, currency, description, placeholder, hidden, commodity_fraction, created_at, updated_at
         FROM accounts WHERE id = $1 AND user_id = $2",
    )
    .bind(id)
    .bind(auth.id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| internal_error("Failed to get account", e))?
    .ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "Account not found" })),
        )
    })?;

    let balance_row: Option<(Decimal,)> = sqlx::query_as(
        "SELECT COALESCE(SUM(amount), 0) FROM splits WHERE account_id = $1",
    )
    .bind(id)
    .fetch_optional(&pool)
    .await
    .unwrap_or(None);

    let balance = balance_row.map(|r| r.0).unwrap_or(Decimal::ZERO);
    let display_balance = calculate_display_balance(&row.account_type, balance);

    let account = Account {
        id: row.id,
        name: row.name,
        code: row.code,
        account_type: row.account_type,
        parent_id: row.parent_id,
        currency: row.currency,
        description: row.description,
        placeholder: row.placeholder,
        hidden: row.hidden,
        commodity_fraction: row.commodity_fraction,
        balance,
        total_balance: balance,
        display_balance,
        created_at: row.created_at,
        updated_at: row.updated_at,
    };

    Ok(Json(account))
}

/// Builds the API account from a row, with its direct and total balance.
fn account_from_row(row: AccountRow, balance: Decimal, total_balance: Decimal) -> Account {
    let display_balance = calculate_display_balance(&row.account_type, total_balance);
    Account {
        id: row.id,
        name: row.name,
        code: row.code,
        account_type: row.account_type,
        parent_id: row.parent_id,
        currency: row.currency,
        description: row.description,
        placeholder: row.placeholder,
        hidden: row.hidden,
        commodity_fraction: row.commodity_fraction,
        balance,
        total_balance,
        display_balance,
        created_at: row.created_at,
        updated_at: row.updated_at,
    }
}

pub async fn create_account(
    auth: AuthUser,
    State(pool): State<PgPool>,
    Json(payload): Json<CreateAccountRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let name = validate_account_name(&payload.name)?;
    let acc_type = validate_account_type(&payload.account_type)?;
    let currency = validate_currency(payload.currency.as_deref().unwrap_or("INR"))?;
    let code = sanitize_account_code(payload.code.as_deref())?;

    // If parent_id is specified, verify it belongs to this user and has compatible type (ACC-12)
    if let Some(pid) = payload.parent_id {
        verify_parent(&pool, auth.id, pid, &acc_type, None).await?;
    }

    let row = sqlx::query_as::<_, AccountRow>(
        r#"INSERT INTO accounts (name, code, account_type, parent_id, currency, description, placeholder, hidden, user_id)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
           RETURNING id, name, code, account_type, parent_id, currency, description, placeholder, hidden, commodity_fraction, created_at, updated_at"#
    )
    .bind(name)
    .bind(code)
    .bind(&acc_type)
    .bind(payload.parent_id)
    .bind(currency)
    .bind(payload.description)
    .bind(payload.placeholder.unwrap_or(false))
    .bind(payload.hidden.unwrap_or(false))
    .bind(auth.id)
    .fetch_one(&pool)
    .await
    .map_err(|e| internal_error("Failed to create account", e))?;

    Ok((StatusCode::CREATED, Json(account_from_row(row, Decimal::ZERO, Decimal::ZERO))))
}

pub async fn update_account(
    auth: AuthUser,
    State(pool): State<PgPool>,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateAccountRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let existing = sqlx::query_as::<_, AccountRow>(
        "SELECT id, name, code, account_type, parent_id, currency, description, placeholder, hidden, commodity_fraction, created_at, updated_at
         FROM accounts WHERE id = $1 AND user_id = $2",
    )
    .bind(id)
    .bind(auth.id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| internal_error("Failed to fetch account for update", e))?
    .ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "Account not found" })),
        )
    })?;

    let name = match payload.name {
        Some(ref n) => validate_account_name(n)?,
        None => existing.name,
    };
    let code = match payload.code {
        Some(ref c) => sanitize_account_code(c.as_deref())?,
        None => existing.code,
    };
    let account_type = match payload.account_type {
        Some(ref at) => validate_account_type(at)?,
        None => existing.account_type,
    };
    let parent_id = payload.parent_id.unwrap_or(existing.parent_id);
    let currency = match payload.currency {
        Some(ref c) => validate_currency(c)?,
        None => existing.currency,
    };
    let description = match payload.description {
        Some(d) => d.map(|s| s.trim().to_string()).filter(|s| !s.is_empty()),
        None => existing.description,
    };
    let placeholder = payload.placeholder.unwrap_or(existing.placeholder);
    let hidden = payload.hidden.unwrap_or(existing.hidden);

    if let Some(pid) = parent_id {
        verify_parent(&pool, auth.id, pid, &account_type, Some(id)).await?;
    }
    verify_children_compatible(&pool, auth.id, id, &account_type).await?;

    let updated = sqlx::query_as::<_, AccountRow>(
        r#"UPDATE accounts
           SET name = $1, code = $2, account_type = $3, parent_id = $4, currency = $5,
               description = $6, placeholder = $7, hidden = $8, updated_at = NOW()
           WHERE id = $9 AND user_id = $10
           RETURNING id, name, code, account_type, parent_id, currency, description, placeholder, hidden, commodity_fraction, created_at, updated_at"#
    )
    .bind(name)
    .bind(code)
    .bind(account_type.to_lowercase())
    .bind(parent_id)
    .bind(currency)
    .bind(description)
    .bind(placeholder)
    .bind(hidden)
    .bind(id)
    .bind(auth.id)
    .fetch_one(&pool)
    .await
    .map_err(|e| internal_error("Failed to update account", e))?;

    let balance = sqlx::query_scalar::<_, Decimal>(
        "SELECT COALESCE(SUM(amount), 0) FROM splits WHERE account_id = $1",
    )
    .bind(id)
    .fetch_one(&pool)
    .await
    .map_err(|e| internal_error("Failed to compute account balance", e))?;

    Ok(Json(account_from_row(updated, balance, balance)))
}

pub async fn delete_account(
    auth: AuthUser,
    State(pool): State<PgPool>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    // Check if account exists and belongs to user
    let existing = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM accounts WHERE id = $1 AND user_id = $2",
    )
    .bind(id)
    .bind(auth.id)
    .fetch_one(&pool)
    .await
    .map_err(|e| internal_error("Failed to look up account for delete", e))?;

    if existing == 0 {
        return Err((
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "Account not found" })),
        ));
    }

    // Check if account has child accounts
    let child_count: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM accounts WHERE parent_id = $1 AND user_id = $2",
    )
    .bind(id)
    .bind(auth.id)
    .fetch_one(&pool)
    .await
    .map_err(|e| internal_error("Failed to count child accounts for delete", e))?;

    if child_count.0 > 0 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": format!("Cannot delete account with {} sub-account(s). Please delete or reassign child accounts first.", child_count.0)
            })),
        ));
    }

    // Check if account has splits
    let split_count: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM splits WHERE account_id = $1",
    )
    .bind(id)
    .fetch_one(&pool)
    .await
    .map_err(|e| internal_error("Failed to count splits for delete", e))?;

    if split_count.0 > 0 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": format!("Cannot delete account with {} transactions. Please delete or reassign them first.", split_count.0)
            })),
        ));
    }

    let deleted = sqlx::query("DELETE FROM accounts WHERE id = $1 AND user_id = $2")
        .bind(id)
        .bind(auth.id)
        .execute(&pool)
        .await
        .map_err(|e| internal_error("Failed to delete account", e))?;

    if deleted.rows_affected() == 0 {
        return Err((
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "Account not found" })),
        ));
    }

    Ok(Json(json!({ "message": "Account deleted successfully" })))
}
