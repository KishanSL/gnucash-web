use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use rust_decimal::Decimal;
use serde_json::json;
use sqlx::PgPool;
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

use crate::auth::AuthUser;
use crate::models::{
    calculate_display_balance, Account, AccountRow, AccountTreeNode, CreateAccountRequest,
    UpdateAccountRequest,
};

const VALID_ACCOUNT_TYPES: &[&str] = &[
    "bank", "cash", "asset", "credit", "liability", "equity", "income", "expense", "receivable", "payable", "root"
];

pub async fn list_accounts(
    auth: AuthUser,
    State(pool): State<PgPool>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let rows: Vec<AccountRow> = sqlx::query_as::<_, AccountRow>(
        "SELECT id, name, code, account_type, parent_id, currency, description, placeholder, hidden, created_at, updated_at
         FROM accounts WHERE user_id = $1 ORDER BY code ASC NULLS LAST, name ASC",
    )
    .bind(auth.id)
    .fetch_all(&pool)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Database error: {}", e) })),
        )
    })?;

    // Fetch direct balances for user's accounts
    let balances: Vec<(Uuid, Decimal)> = sqlx::query_as::<_, (Uuid, Decimal)>(
        r#"SELECT s.account_id, COALESCE(SUM(s.amount), 0) as balance
           FROM splits s
           JOIN accounts a ON s.account_id = a.id
           WHERE a.user_id = $1
           GROUP BY s.account_id"#,
    )
    .bind(auth.id)
    .fetch_all(&pool)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Failed to fetch account balances: {}", e) })),
        )
    })?;

    let balance_map: HashMap<Uuid, Decimal> = balances.into_iter().collect();
    let accounts = build_account_list(rows, &balance_map);

    Ok(Json(accounts))
}

pub async fn get_account_tree(
    auth: AuthUser,
    State(pool): State<PgPool>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let rows: Vec<AccountRow> = sqlx::query_as::<_, AccountRow>(
        "SELECT id, name, code, account_type, parent_id, currency, description, placeholder, hidden, created_at, updated_at
         FROM accounts WHERE user_id = $1 ORDER BY code ASC NULLS LAST, name ASC",
    )
    .bind(auth.id)
    .fetch_all(&pool)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Database error: {}", e) })),
        )
    })?;

    let balances: Vec<(Uuid, Decimal)> = sqlx::query_as::<_, (Uuid, Decimal)>(
        r#"SELECT s.account_id, COALESCE(SUM(s.amount), 0) as balance
           FROM splits s
           JOIN accounts a ON s.account_id = a.id
           WHERE a.user_id = $1
           GROUP BY s.account_id"#,
    )
    .bind(auth.id)
    .fetch_all(&pool)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Failed to fetch account tree balances: {}", e) })),
        )
    })?;

    let balance_map: HashMap<Uuid, Decimal> = balances.into_iter().collect();
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
        "SELECT id, name, code, account_type, parent_id, currency, description, placeholder, hidden, created_at, updated_at
         FROM accounts WHERE id = $1 AND user_id = $2",
    )
    .bind(id)
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
    auth: AuthUser,
    State(pool): State<PgPool>,
    Json(payload): Json<CreateAccountRequest>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let name = payload.name.trim();
    if name.is_empty() || name.len() > 255 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "Account name must be between 1 and 255 characters" })),
        ));
    }

    let acc_type = payload.account_type.trim().to_lowercase();
    if !VALID_ACCOUNT_TYPES.contains(&acc_type.as_str()) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": format!("Invalid account type '{}'", acc_type) })),
        ));
    }

    let currency = payload.currency.unwrap_or_else(|| "INR".to_string()).trim().to_uppercase();
    if currency.len() != 3 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "Currency must be a valid 3-letter code (e.g. INR, USD)" })),
        ));
    }

    // If parent_id is specified, verify it belongs to this user
    if let Some(pid) = payload.parent_id {
        let parent_exists = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM accounts WHERE id = $1 AND user_id = $2",
        )
        .bind(pid)
        .bind(auth.id)
        .fetch_one(&pool)
        .await
        .unwrap_or(0);

        if parent_exists == 0 {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(json!({ "error": "Parent account not found" })),
            ));
        }
    }

    let placeholder = payload.placeholder.unwrap_or(false);
    let hidden = payload.hidden.unwrap_or(false);

    let row = sqlx::query_as::<_, AccountRow>(
        r#"INSERT INTO accounts (name, code, account_type, parent_id, currency, description, placeholder, hidden, user_id)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
           RETURNING id, name, code, account_type, parent_id, currency, description, placeholder, hidden, created_at, updated_at"#
    )
    .bind(name)
    .bind(payload.code.map(|s| s.trim().to_string()).filter(|s| !s.is_empty()))
    .bind(&acc_type)
    .bind(payload.parent_id)
    .bind(currency)
    .bind(payload.description)
    .bind(placeholder)
    .bind(hidden)
    .bind(auth.id)
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
    auth: AuthUser,
    State(pool): State<PgPool>,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateAccountRequest>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let existing = sqlx::query_as::<_, AccountRow>(
        "SELECT id, name, code, account_type, parent_id, currency, description, placeholder, hidden, created_at, updated_at
         FROM accounts WHERE id = $1 AND user_id = $2",
    )
    .bind(id)
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
            Json(json!({ "error": "Account not found" })),
        )
    })?;

    let name = match payload.name {
        Some(ref n) => {
            let trimmed = n.trim();
            if trimmed.is_empty() || trimmed.len() > 255 {
                return Err((
                    StatusCode::BAD_REQUEST,
                    Json(json!({ "error": "Account name must be between 1 and 255 characters" })),
                ));
            }
            trimmed.to_string()
        }
        None => existing.name,
    };

    let code = match payload.code {
        Some(c) => c.map(|s| s.trim().to_string()).filter(|s| !s.is_empty()),
        None => existing.code,
    };

    let account_type = match payload.account_type {
        Some(ref at) => {
            let normalized = at.trim().to_lowercase();
            if !VALID_ACCOUNT_TYPES.contains(&normalized.as_str()) {
                return Err((
                    StatusCode::BAD_REQUEST,
                    Json(json!({ "error": format!("Invalid account type '{}'", normalized) })),
                ));
            }
            normalized
        }
        None => existing.account_type,
    };

    let parent_id = match payload.parent_id {
        Some(p) => p,
        None => existing.parent_id,
    };

    let currency = match payload.currency {
        Some(ref c) => {
            let trimmed = c.trim().to_uppercase();
            if trimmed.is_empty() || trimmed.len() > 64 {
                return Err((
                    StatusCode::BAD_REQUEST,
                    Json(json!({ "error": "Currency code must be between 1 and 64 characters" })),
                ));
            }
            trimmed
        }
        None => existing.currency,
    };

    let description = match payload.description {
        Some(d) => d.map(|s| s.trim().to_string()).filter(|s| !s.is_empty()),
        None => existing.description,
    };

    let placeholder = payload.placeholder.unwrap_or(existing.placeholder);
    let hidden = payload.hidden.unwrap_or(existing.hidden);

    // Prevent setting self as parent
    if parent_id == Some(id) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "Account cannot be its own parent" })),
        ));
    }

    if let Some(pid) = parent_id {
        let parent_exists = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM accounts WHERE id = $1 AND user_id = $2",
        )
        .bind(pid)
        .bind(auth.id)
        .fetch_one(&pool)
        .await
        .unwrap_or(0);

        if parent_exists == 0 {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(json!({ "error": "Parent account not found" })),
            ));
        }

        // Cycle check: verify that 'id' is NOT an ancestor of 'pid'
        let has_cycle = sqlx::query_scalar::<_, bool>(
            r#"WITH RECURSIVE anc AS (
                SELECT id, parent_id FROM accounts WHERE id = $1 AND user_id = $2
                UNION ALL
                SELECT a.id, a.parent_id FROM accounts a JOIN anc ON a.id = anc.parent_id
            )
            SELECT EXISTS (SELECT 1 FROM anc WHERE id = $3)"#
        )
        .bind(pid)
        .bind(auth.id)
        .bind(id)
        .fetch_one(&pool)
        .await
        .unwrap_or(false);

        if has_cycle {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(json!({ "error": "Cannot assign a descendant as parent (cycle detected)" })),
            ));
        }
    }

    let updated = sqlx::query_as::<_, AccountRow>(
        r#"UPDATE accounts
           SET name = $1, code = $2, account_type = $3, parent_id = $4, currency = $5,
               description = $6, placeholder = $7, hidden = $8, updated_at = NOW()
           WHERE id = $9 AND user_id = $10
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
    .bind(auth.id)
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
    .unwrap_or(0);

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
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Database error: {}", e) })),
        )
    })?;

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

    let deleted = sqlx::query("DELETE FROM accounts WHERE id = $1 AND user_id = $2")
        .bind(id)
        .bind(auth.id)
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

    for r in &rows {
        let b = balance_map.get(&r.id).copied().unwrap_or(Decimal::ZERO);
        direct_balances.insert(r.id, b);
        parent_to_children
            .entry(r.parent_id)
            .or_default()
            .push(r.id);
    }

    // Compute total recursive balance with cycle guard
    fn calc_total_balance(
        acc_id: Uuid,
        direct_balances: &HashMap<Uuid, Decimal>,
        parent_to_children: &HashMap<Option<Uuid>, Vec<Uuid>>,
        visited: &mut HashSet<Uuid>,
    ) -> Decimal {
        if !visited.insert(acc_id) {
            return Decimal::ZERO;
        }
        let direct = direct_balances.get(&acc_id).copied().unwrap_or(Decimal::ZERO);
        let children = parent_to_children.get(&Some(acc_id));
        let children_total: Decimal = match children {
            Some(c_ids) => c_ids
                .iter()
                .map(|&cid| calc_total_balance(cid, direct_balances, parent_to_children, visited))
                .sum(),
            None => Decimal::ZERO,
        };
        visited.remove(&acc_id);
        direct + children_total
    }

    let mut accounts = Vec::new();
    for r in rows {
        let direct_b = direct_balances.get(&r.id).copied().unwrap_or(Decimal::ZERO);
        let mut visited = HashSet::new();
        let total_b = calc_total_balance(r.id, &direct_balances, &parent_to_children, &mut visited);
        let display_b = calculate_display_balance(&r.account_type, total_b);

        accounts.push(Account {
            id: r.id,
            name: r.name,
            code: r.code,
            account_type: r.account_type,
            parent_id: r.parent_id,
            currency: r.currency,
            description: r.description,
            placeholder: r.placeholder,
            hidden: r.hidden,
            balance: direct_b,
            total_balance: total_b,
            display_balance: display_b,
            created_at: r.created_at,
            updated_at: r.updated_at,
        });
    }

    accounts
}

fn build_tree_structure(accounts: Vec<Account>) -> Vec<AccountTreeNode> {
    let mut account_map: HashMap<Uuid, Account> = HashMap::new();
    let mut parent_to_children: HashMap<Option<Uuid>, Vec<Uuid>> = HashMap::new();

    for acc in accounts {
        let id = acc.id;
        let parent_id = acc.parent_id;
        account_map.insert(id, acc);
        parent_to_children.entry(parent_id).or_default().push(id);
    }

    fn build_node(
        acc_id: Uuid,
        account_map: &HashMap<Uuid, Account>,
        parent_to_children: &HashMap<Option<Uuid>, Vec<Uuid>>,
        visited: &mut HashSet<Uuid>,
    ) -> Option<AccountTreeNode> {
        if !visited.insert(acc_id) {
            return None;
        }
        let account = account_map.get(&acc_id)?.clone();
        let child_ids = parent_to_children.get(&Some(acc_id));
        let mut children = Vec::new();
        if let Some(c_ids) = child_ids {
            for &cid in c_ids {
                if let Some(child_node) = build_node(cid, account_map, parent_to_children, visited) {
                    children.push(child_node);
                }
            }
        }
        visited.remove(&acc_id);
        Some(AccountTreeNode { account, children })
    }

    let mut roots = Vec::new();
    if let Some(root_ids) = parent_to_children.get(&None) {
        for &rid in root_ids {
            let mut visited = HashSet::new();
            if let Some(root_node) = build_node(rid, &account_map, &parent_to_children, &mut visited) {
                roots.push(root_node);
            }
        }
    }

    roots
}
