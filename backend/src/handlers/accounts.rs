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

use crate::models::{
    calculate_display_balance, Account, AccountRow, AccountTreeNode, CreateAccountRequest,
    UpdateAccountRequest,
};

pub async fn list_accounts(
    State(pool): State<PgPool>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let rows: Vec<AccountRow> = sqlx::query_as::<_, AccountRow>(
        "SELECT id, name, code, account_type, parent_id, currency, description, placeholder, hidden, created_at, updated_at
         FROM accounts ORDER BY code ASC NULLS LAST, name ASC",
    )
    .fetch_all(&pool)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Database error: {}", e) })),
        )
    })?;

    // Fetch direct balances
    let balances: Vec<(Uuid, Decimal)> = sqlx::query_as::<_, (Uuid, Decimal)>(
        "SELECT account_id, COALESCE(SUM(amount), 0) as balance FROM splits GROUP BY account_id",
    )
    .fetch_all(&pool)
    .await
    .unwrap_or_default();

    let balance_map: HashMap<Uuid, Decimal> = balances.into_iter().collect();

    let accounts = build_account_list(rows, &balance_map);

    Ok(Json(accounts))
}

pub async fn get_account_tree(
    State(pool): State<PgPool>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let rows: Vec<AccountRow> = sqlx::query_as::<_, AccountRow>(
        "SELECT id, name, code, account_type, parent_id, currency, description, placeholder, hidden, created_at, updated_at
         FROM accounts ORDER BY code ASC NULLS LAST, name ASC",
    )
    .fetch_all(&pool)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Database error: {}", e) })),
        )
    })?;

    let balances: Vec<(Uuid, Decimal)> = sqlx::query_as::<_, (Uuid, Decimal)>(
        "SELECT account_id, COALESCE(SUM(amount), 0) as balance FROM splits GROUP BY account_id",
    )
    .fetch_all(&pool)
    .await
    .unwrap_or_default();

    let balance_map: HashMap<Uuid, Decimal> = balances.into_iter().collect();
    let accounts = build_account_list(rows, &balance_map);

    let tree = build_tree_structure(accounts);
    Ok(Json(tree))
}

pub async fn get_account(
    State(pool): State<PgPool>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let row = sqlx::query_as::<_, AccountRow>(
        "SELECT id, name, code, account_type, parent_id, currency, description, placeholder, hidden, created_at, updated_at
         FROM accounts WHERE id = $1",
    )
    .bind(id)
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
        balance,
        total_balance: balance,
        display_balance,
        created_at: row.created_at,
        updated_at: row.updated_at,
    };

    Ok(Json(account))
}

pub async fn create_account(
    State(pool): State<PgPool>,
    Json(payload): Json<CreateAccountRequest>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    if payload.name.trim().is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "Account name cannot be empty" })),
        ));
    }

    let currency = payload.currency.unwrap_or_else(|| "INR".to_string());
    let placeholder = payload.placeholder.unwrap_or(false);
    let hidden = payload.hidden.unwrap_or(false);

    let row = sqlx::query_as::<_, AccountRow>(
        r#"INSERT INTO accounts (name, code, account_type, parent_id, currency, description, placeholder, hidden)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
           RETURNING id, name, code, account_type, parent_id, currency, description, placeholder, hidden, created_at, updated_at"#
    )
    .bind(payload.name.trim())
    .bind(payload.code.map(|s| s.trim().to_string()).filter(|s| !s.is_empty()))
    .bind(payload.account_type.trim().to_lowercase())
    .bind(payload.parent_id)
    .bind(currency)
    .bind(payload.description)
    .bind(placeholder)
    .bind(hidden)
    .fetch_one(&pool)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Failed to create account: {}", e) })),
        )
    })?;

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
        balance: Decimal::ZERO,
        total_balance: Decimal::ZERO,
        display_balance: Decimal::ZERO,
        created_at: row.created_at,
        updated_at: row.updated_at,
    };

    Ok((StatusCode::CREATED, Json(account)))
}

pub async fn update_account(
    State(pool): State<PgPool>,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateAccountRequest>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let existing = sqlx::query_as::<_, AccountRow>(
        "SELECT id, name, code, account_type, parent_id, currency, description, placeholder, hidden, created_at, updated_at
         FROM accounts WHERE id = $1",
    )
    .bind(id)
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
            Json(json!({ "error": "Account not found" })),
        )
    })?;

    let name = payload.name.unwrap_or(existing.name);
    let code = payload.code.or(existing.code);
    let account_type = payload.account_type.unwrap_or(existing.account_type);
    let parent_id = payload.parent_id.or(existing.parent_id);
    let currency = payload.currency.unwrap_or(existing.currency);
    let description = payload.description.or(existing.description);
    let placeholder = payload.placeholder.unwrap_or(existing.placeholder);
    let hidden = payload.hidden.unwrap_or(existing.hidden);

    // Prevent setting self as parent
    if parent_id == Some(id) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "Account cannot be its own parent" })),
        ));
    }

    let updated = sqlx::query_as::<_, AccountRow>(
        r#"UPDATE accounts
           SET name = $1, code = $2, account_type = $3, parent_id = $4, currency = $5,
               description = $6, placeholder = $7, hidden = $8, updated_at = NOW()
           WHERE id = $9
           RETURNING id, name, code, account_type, parent_id, currency, description, placeholder, hidden, created_at, updated_at"#
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
    .fetch_one(&pool)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Failed to update account: {}", e) })),
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
    let display_balance = calculate_display_balance(&updated.account_type, balance);

    let account = Account {
        id: updated.id,
        name: updated.name,
        code: updated.code,
        account_type: updated.account_type,
        parent_id: updated.parent_id,
        currency: updated.currency,
        description: updated.description,
        placeholder: updated.placeholder,
        hidden: updated.hidden,
        balance,
        total_balance: balance,
        display_balance,
        created_at: updated.created_at,
        updated_at: updated.updated_at,
    };

    Ok(Json(account))
}

pub async fn delete_account(
    State(pool): State<PgPool>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    // Check if account has splits
    let split_count: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM splits WHERE account_id = $1",
    )
    .bind(id)
    .fetch_one(&pool)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Database error: {}", e) })),
        )
    })?;

    if split_count.0 > 0 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": format!("Cannot delete account with {} transactions. Please delete or reassign them first.", split_count.0)
            })),
        ));
    }

    let deleted = sqlx::query("DELETE FROM accounts WHERE id = $1")
        .bind(id)
        .execute(&pool)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": format!("Failed to delete account: {}", e) })),
            )
        })?;

    if deleted.rows_affected() == 0 {
        return Err((
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "Account not found" })),
        ));
    }

    Ok(Json(json!({ "message": "Account deleted successfully" })))
}

fn build_account_list(rows: Vec<AccountRow>, balance_map: &HashMap<Uuid, Decimal>) -> Vec<Account> {
    // Map of id -> direct balance
    let mut direct_balances: HashMap<Uuid, Decimal> = HashMap::new();
    let mut parent_to_children: HashMap<Option<Uuid>, Vec<Uuid>> = HashMap::new();

    for row in &rows {
        let direct = balance_map.get(&row.id).copied().unwrap_or(Decimal::ZERO);
        direct_balances.insert(row.id, direct);
        parent_to_children
            .entry(row.parent_id)
            .or_default()
            .push(row.id);
    }

    // Function to calculate recursive total balance for an account
    fn calc_total(
        account_id: Uuid,
        direct_balances: &HashMap<Uuid, Decimal>,
        parent_to_children: &HashMap<Option<Uuid>, Vec<Uuid>>,
    ) -> Decimal {
        let mut total = direct_balances.get(&account_id).copied().unwrap_or(Decimal::ZERO);
        if let Some(children) = parent_to_children.get(&Some(account_id)) {
            for child in children {
                total += calc_total(*child, direct_balances, parent_to_children);
            }
        }
        total
    }

    rows.into_iter()
        .map(|row| {
            let direct = direct_balances.get(&row.id).copied().unwrap_or(Decimal::ZERO);
            let total = calc_total(row.id, &direct_balances, &parent_to_children);
            let display_balance = calculate_display_balance(&row.account_type, total);

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
                balance: direct,
                total_balance: total,
                display_balance,
                created_at: row.created_at,
                updated_at: row.updated_at,
            }
        })
        .collect()
}

fn build_tree_structure(accounts: Vec<Account>) -> Vec<AccountTreeNode> {
    let mut by_parent: HashMap<Option<Uuid>, Vec<Account>> = HashMap::new();
    for acc in accounts {
        by_parent.entry(acc.parent_id).or_default().push(acc);
    }

    fn recurse(parent_id: Option<Uuid>, by_parent: &HashMap<Option<Uuid>, Vec<Account>>) -> Vec<AccountTreeNode> {
        let mut nodes = Vec::new();
        if let Some(children) = by_parent.get(&parent_id) {
            for child in children {
                let sub_nodes = recurse(Some(child.id), by_parent);
                nodes.push(AccountTreeNode {
                    account: child.clone(),
                    children: sub_nodes,
                });
            }
        }
        nodes
    }

    recurse(None, &by_parent)
}
