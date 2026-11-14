use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::Deserialize;
use serde_json::json;
use sqlx::PgPool;
use std::collections::HashMap;
use uuid::Uuid;

use crate::auth::AuthUser;
use crate::models::{
    calculate_display_balance, is_asset_type, is_expense_type,
    CreateTransactionRequest, RegisterEntry, SplitRow, SplitWithAccount, TransactionRow,
    TransactionWithSplits, UpdateTransactionRequest,
};

#[derive(Debug, Deserialize)]
pub struct TransactionListQuery {
    pub from_date: Option<NaiveDate>,
    pub to_date: Option<NaiveDate>,
    pub search: Option<String>,
    pub account_id: Option<Uuid>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

fn escape_like_pattern(s: &str) -> String {
    s.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_")
}

pub async fn list_transactions(
    auth: AuthUser,
    State(pool): State<PgPool>,
    Query(query): Query<TransactionListQuery>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let limit = query.limit.unwrap_or(100).clamp(1, 500);
    let offset = query.offset.unwrap_or(0).max(0);

    let mut sql = String::from(
        "SELECT t.id, t.post_date, t.enter_date, t.num, t.description, t.notes, t.created_at, t.updated_at
         FROM transactions t
         WHERE t.user_id = $1",
    );

    let mut param_idx = 2;
    if query.account_id.is_some() {
        sql.push_str(&format!(" AND EXISTS (SELECT 1 FROM splits s WHERE s.transaction_id = t.id AND s.account_id = ${})", param_idx));
        param_idx += 1;
    }
    if query.from_date.is_some() {
        sql.push_str(&format!(" AND t.post_date >= ${}", param_idx));
        param_idx += 1;
    }
    if query.to_date.is_some() {
        sql.push_str(&format!(" AND t.post_date <= ${}", param_idx));
        param_idx += 1;
    }
    if query.search.is_some() {
        sql.push_str(&format!(" AND (t.description ILIKE ${0} ESCAPE '\\' OR t.num ILIKE ${0} ESCAPE '\\' OR t.notes ILIKE ${0} ESCAPE '\\')", param_idx));
        param_idx += 1;
    }

    sql.push_str(&format!(" ORDER BY t.post_date DESC, t.enter_date DESC LIMIT ${} OFFSET ${}", param_idx, param_idx + 1));

    let search_pattern = query.search.as_ref().map(|s| format!("%{}%", escape_like_pattern(s)));

    let mut q = sqlx::query_as::<_, TransactionRow>(&sql).bind(auth.id);

    if let Some(acc_id) = query.account_id {
        q = q.bind(acc_id);
    }
    if let Some(from_d) = query.from_date {
        q = q.bind(from_d);
    }
    if let Some(to_d) = query.to_date {
        q = q.bind(to_d);
    }
    if let Some(sp) = search_pattern {
        q = q.bind(sp);
    }
    q = q.bind(limit).bind(offset);

    let rows: Vec<TransactionRow> = q
        .fetch_all(&pool)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": format!("Database error: {}", e) })),
            )
        })?;

    if rows.is_empty() {
        return Ok(Json(Vec::<TransactionWithSplits>::new()));
    }

    let tx_ids: Vec<Uuid> = rows.iter().map(|r| r.id).collect();

    // Fetch splits for all returned transactions
    let split_rows = sqlx::query_as::<_, (Uuid, Uuid, Uuid, String, String, Decimal, Option<String>, String, Option<chrono::DateTime<chrono::Utc>>)>(
        r#"SELECT s.id, s.transaction_id, s.account_id, a.name, a.account_type, s.amount, s.memo, s.reconciled_state, s.reconciled_date
           FROM splits s
           JOIN accounts a ON s.account_id = a.id
           WHERE s.transaction_id = ANY($1)
           ORDER BY s.amount DESC"#
    )
    .bind(&tx_ids)
    .fetch_all(&pool)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Failed to fetch splits: {}", e) })),
        )
    })?;

    let mut splits_by_tx: HashMap<Uuid, Vec<SplitWithAccount>> = HashMap::new();
    for (id, tx_id, account_id, account_name, account_type, amount, memo, reconciled_state, reconciled_date) in split_rows {
        splits_by_tx.entry(tx_id).or_default().push(SplitWithAccount {
            id,
            transaction_id: tx_id,
            account_id,
            account_name,
            account_type,
            amount,
            memo,
            reconciled_state,
            reconciled_date,
        });
    }

    let result: Vec<TransactionWithSplits> = rows
        .into_iter()
        .map(|tx| {
            let splits = splits_by_tx.remove(&tx.id).unwrap_or_default();
            TransactionWithSplits {
                id: tx.id,
                post_date: tx.post_date,
                enter_date: tx.enter_date,
                num: tx.num,
                description: tx.description,
                notes: tx.notes,
                splits,
                created_at: tx.created_at,
                updated_at: tx.updated_at,
            }
        })
        .collect();

    Ok(Json(result))
}

pub async fn get_transaction(
    auth: AuthUser,
    State(pool): State<PgPool>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let tx = sqlx::query_as::<_, TransactionRow>(
        "SELECT id, post_date, enter_date, num, description, notes, created_at, updated_at
         FROM transactions WHERE id = $1 AND user_id = $2",
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
            Json(json!({ "error": "Transaction not found" })),
        )
    })?;

    let split_rows = sqlx::query_as::<_, (Uuid, Uuid, Uuid, String, String, Decimal, Option<String>, String, Option<chrono::DateTime<chrono::Utc>>)>(
        r#"SELECT s.id, s.transaction_id, s.account_id, a.name, a.account_type, s.amount, s.memo, s.reconciled_state, s.reconciled_date
           FROM splits s
           JOIN accounts a ON s.account_id = a.id
           WHERE s.transaction_id = $1
           ORDER BY s.amount DESC"#
    )
    .bind(id)
    .fetch_all(&pool)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Failed to fetch splits: {}", e) })),
        )
    })?;

    let splits = split_rows
        .into_iter()
        .map(|(s_id, tx_id, acc_id, acc_name, acc_type, amount, memo, rec_state, rec_date)| SplitWithAccount {
            id: s_id,
            transaction_id: tx_id,
            account_id: acc_id,
            account_name: acc_name,
            account_type: acc_type,
            amount,
            memo,
            reconciled_state: rec_state,
            reconciled_date: rec_date,
        })
        .collect();

    let result = TransactionWithSplits {
        id: tx.id,
        post_date: tx.post_date,
        enter_date: tx.enter_date,
        num: tx.num,
        description: tx.description,
        notes: tx.notes,
        splits,
        created_at: tx.created_at,
        updated_at: tx.updated_at,
    };

    Ok(Json(result))
}

pub async fn create_transaction(
    auth: AuthUser,
    State(pool): State<PgPool>,
    Json(payload): Json<CreateTransactionRequest>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    // 1. Text length validation
    if payload.description.trim().is_empty() || payload.description.len() > 255 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "Transaction description must be between 1 and 255 characters" })),
        ));
    }
    if let Some(ref num) = payload.num {
        if num.len() > 50 {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(json!({ "error": "Transaction num/reference cannot exceed 50 characters" })),
            ));
        }
    }

    // 2. Double-entry validation: Between 2 and 100 splits
    if payload.splits.len() < 2 || payload.splits.len() > 100 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "A double-entry transaction must have between 2 and 100 splits" })),
        ));
    }

    // 3. Sum of amounts must be exactly zero, and scale must not exceed 2 decimal places
    let mut sum = Decimal::ZERO;
    for split in &payload.splits {
        if split.amount.scale() > 2 {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(json!({ "error": format!("Split amount {} cannot have more than 2 decimal places", split.amount) })),
            ));
        }
        if let Some(ref rec) = split.reconciled_state {
            let r = rec.to_lowercase();
            if r != "n" && r != "c" && r != "y" {
                return Err((
                    StatusCode::BAD_REQUEST,
                    Json(json!({ "error": format!("Invalid reconciled_state '{}': must be 'n', 'c', or 'y'", rec) })),
                ));
            }
        }
        sum += split.amount;
    }

    if sum != Decimal::ZERO {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": format!("Transaction is not balanced! Sum of splits is {}, must be 0.00", sum)
            })),
        ));
    }

    // 3. Verify accounts exist, belong to this user, and are not placeholders
    let account_ids: Vec<Uuid> = payload.splits.iter().map(|s| s.account_id).collect();
    let accounts: Vec<(Uuid, String, bool)> = sqlx::query_as(
        "SELECT id, name, placeholder FROM accounts WHERE id = ANY($1) AND user_id = $2",
    )
    .bind(&account_ids)
    .bind(auth.id)
    .fetch_all(&pool)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Database error: {}", e) })),
        )
    })?;

    let account_map: HashMap<Uuid, (String, bool)> = accounts
        .into_iter()
        .map(|(id, name, placeholder)| (id, (name, placeholder)))
        .collect();

    for split in &payload.splits {
        match account_map.get(&split.account_id) {
            None => {
                return Err((
                    StatusCode::BAD_REQUEST,
                    Json(json!({ "error": format!("Account {} does not exist or does not belong to you", split.account_id) })),
                ));
            }
            Some((name, placeholder)) => {
                if *placeholder {
                    return Err((
                        StatusCode::BAD_REQUEST,
                        Json(json!({ "error": format!("Cannot post transactions to placeholder account '{}'", name) })),
                    ));
                }
            }
        }
    }

    // 4. Execute atomic transaction in PostgreSQL
    let mut tx = pool.begin().await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Failed to start transaction: {}", e) })),
        )
    })?;

    let tx_row = sqlx::query_as::<_, TransactionRow>(
        r#"INSERT INTO transactions (post_date, num, description, notes, user_id)
           VALUES ($1, $2, $3, $4, $5)
           RETURNING id, post_date, enter_date, num, description, notes, created_at, updated_at"#
    )
    .bind(payload.post_date)
    .bind(payload.num.map(|s| s.trim().to_string()).filter(|s| !s.is_empty()))
    .bind(payload.description.trim())
    .bind(payload.notes)
    .bind(auth.id)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Failed to insert transaction: {}", e) })),
        )
    })?;

    let mut created_splits = Vec::new();
    for s in payload.splits {
        let rec_state = s.reconciled_state.unwrap_or_else(|| "n".to_string());
        let rec_date = if rec_state == "y" { Some(chrono::Utc::now()) } else { None };

        let split_row = sqlx::query_as::<_, SplitRow>(
            r#"INSERT INTO splits (transaction_id, account_id, amount, memo, reconciled_state, reconciled_date)
               VALUES ($1, $2, $3, $4, $5, $6)
               RETURNING id, transaction_id, account_id, amount, memo, reconciled_state, reconciled_date"#
        )
        .bind(tx_row.id)
        .bind(s.account_id)
        .bind(s.amount)
        .bind(s.memo)
        .bind(&rec_state)
        .bind(rec_date)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": format!("Failed to insert split: {}", e) })),
            )
        })?;

        created_splits.push(split_row);
    }

    tx.commit().await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Failed to commit transaction: {}", e) })),
        )
    })?;

    // Format output
    let splits_with_acc = created_splits
        .into_iter()
        .map(|s| {
            let (name, _) = account_map.get(&s.account_id).cloned().unwrap_or_default();
            SplitWithAccount {
                id: s.id,
                transaction_id: s.transaction_id,
                account_id: s.account_id,
                account_name: name,
                account_type: String::new(),
                amount: s.amount,
                memo: s.memo,
                reconciled_state: s.reconciled_state,
                reconciled_date: s.reconciled_date,
            }
        })
        .collect();

    let result = TransactionWithSplits {
        id: tx_row.id,
        post_date: tx_row.post_date,
        enter_date: tx_row.enter_date,
        num: tx_row.num,
        description: tx_row.description,
        notes: tx_row.notes,
        splits: splits_with_acc,
        created_at: tx_row.created_at,
        updated_at: tx_row.updated_at,
    };

    Ok((StatusCode::CREATED, Json(result)))
}

pub async fn update_transaction(
    auth: AuthUser,
    State(pool): State<PgPool>,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateTransactionRequest>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let existing = sqlx::query_as::<_, TransactionRow>(
        "SELECT id, post_date, enter_date, num, description, notes, created_at, updated_at
         FROM transactions WHERE id = $1 AND user_id = $2",
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
            Json(json!({ "error": "Transaction not found" })),
        )
    })?;

    let post_date = payload.post_date.unwrap_or(existing.post_date);
    let num = payload.num.or(existing.num);
    let description = payload.description.unwrap_or(existing.description);
    let notes = payload.notes.or(existing.notes);

    let mut tx = pool.begin().await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Failed to begin transaction: {}", e) })),
        )
    })?;

    let _updated_tx = sqlx::query_as::<_, TransactionRow>(
        r#"UPDATE transactions
           SET post_date = $1, num = $2, description = $3, notes = $4, updated_at = NOW()
           WHERE id = $5 AND user_id = $6
           RETURNING id, post_date, enter_date, num, description, notes, created_at, updated_at"#
    )
    .bind(post_date)
    .bind(num)
    .bind(description)
    .bind(notes)
    .bind(id)
    .bind(auth.id)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Failed to update transaction: {}", e) })),
        )
    })?;

    if let Some(new_splits) = payload.splits {
        if new_splits.len() < 2 || new_splits.len() > 100 {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(json!({ "error": "A double-entry transaction must have between 2 and 100 splits" })),
            ));
        }

        let mut sum = Decimal::ZERO;
        for s in &new_splits {
            if s.amount.scale() > 2 {
                return Err((
                    StatusCode::BAD_REQUEST,
                    Json(json!({ "error": format!("Split amount {} cannot have more than 2 decimal places", s.amount) })),
                ));
            }
            if let Some(ref rec) = s.reconciled_state {
                let r = rec.to_lowercase();
                if r != "n" && r != "c" && r != "y" {
                    return Err((
                        StatusCode::BAD_REQUEST,
                        Json(json!({ "error": format!("Invalid reconciled_state '{}': must be 'n', 'c', or 'y'", rec) })),
                    ));
                }
            }
            sum += s.amount;
        }

        if sum != Decimal::ZERO {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": format!("Transaction is not balanced! Sum is {}, must be 0.00", sum)
                })),
            ));
        }

        // Verify all accounts belong to user
        let mut unique_account_ids: Vec<Uuid> = new_splits.iter().map(|s| s.account_id).collect();
        unique_account_ids.sort();
        unique_account_ids.dedup();

        let valid_acc_count = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM accounts WHERE id = ANY($1) AND user_id = $2 AND placeholder = false",
        )
        .bind(&unique_account_ids)
        .bind(auth.id)
        .fetch_one(&mut *tx)
        .await
        .unwrap_or(0);

        if (valid_acc_count as usize) < unique_account_ids.len() {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(json!({ "error": "One or more accounts do not belong to you or are placeholder categories" })),
            ));
        }

        // Fetch existing splits for this transaction to preserve reconciled state and IDs
        let existing_splits = sqlx::query_as::<_, SplitRow>(
            "SELECT id, transaction_id, account_id, amount, memo, reconciled_state, reconciled_date
             FROM splits WHERE transaction_id = $1",
        )
        .bind(id)
        .fetch_all(&mut *tx)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": format!("Failed to fetch existing splits: {}", e) })),
            )
        })?;

        // Match new splits to existing splits to preserve state
        let mut matched_existing: Vec<Option<SplitRow>> = vec![None; new_splits.len()];
        let mut used_existing_ids: std::collections::HashSet<Uuid> = std::collections::HashSet::new();

        // Pass 1: Match by explicit split id
        for (i, s) in new_splits.iter().enumerate() {
            if let Some(split_id) = s.id {
                if let Some(existing_s) = existing_splits.iter().find(|e| e.id == split_id && !used_existing_ids.contains(&e.id)) {
                    matched_existing[i] = Some(existing_s.clone());
                    used_existing_ids.insert(existing_s.id);
                }
            }
        }

        // Pass 2: Match by account_id and amount for remaining unmatched splits
        for (i, s) in new_splits.iter().enumerate() {
            if matched_existing[i].is_none() {
                if let Some(existing_s) = existing_splits.iter().find(|e| {
                    !used_existing_ids.contains(&e.id) && e.account_id == s.account_id && e.amount == s.amount
                }) {
                    matched_existing[i] = Some(existing_s.clone());
                    used_existing_ids.insert(existing_s.id);
                }
            }
        }

        // Pass 3: Match by account_id only for remaining unmatched splits
        for (i, s) in new_splits.iter().enumerate() {
            if matched_existing[i].is_none() {
                if let Some(existing_s) = existing_splits.iter().find(|e| {
                    !used_existing_ids.contains(&e.id) && e.account_id == s.account_id
                }) {
                    matched_existing[i] = Some(existing_s.clone());
                    used_existing_ids.insert(existing_s.id);
                }
            }
        }

        // Delete any existing splits that are no longer part of this transaction
        let remaining_split_ids: Vec<Uuid> = matched_existing
            .iter()
            .filter_map(|m| m.as_ref().map(|e| e.id))
            .collect();

        if remaining_split_ids.is_empty() {
            sqlx::query("DELETE FROM splits WHERE transaction_id = $1")
                .bind(id)
                .execute(&mut *tx)
                .await
                .map_err(|e| {
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!({ "error": format!("Failed to clear splits: {}", e) })),
                    )
                })?;
        } else {
            sqlx::query("DELETE FROM splits WHERE transaction_id = $1 AND NOT (id = ANY($2))")
                .bind(id)
                .bind(&remaining_split_ids)
                .execute(&mut *tx)
                .await
                .map_err(|e| {
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!({ "error": format!("Failed to remove deleted splits: {}", e) })),
                    )
                })?;
        }

        // Update or insert splits
        for (i, s) in new_splits.into_iter().enumerate() {
            let (rec_state, rec_date) = match (&s.reconciled_state, &matched_existing[i]) {
                (Some(req_state), Some(existing_s)) => {
                    let norm = req_state.to_lowercase();
                    let date = if norm == "y" {
                        existing_s.reconciled_date.or_else(|| Some(chrono::Utc::now()))
                    } else {
                        None
                    };
                    (norm, date)
                }
                (Some(req_state), None) => {
                    let norm = req_state.to_lowercase();
                    let date = if norm == "y" { Some(chrono::Utc::now()) } else { None };
                    (norm, date)
                }
                (None, Some(existing_s)) => {
                    // Carry over existing reconciled state and date
                    (existing_s.reconciled_state.clone(), existing_s.reconciled_date)
                }
                (None, None) => ("n".to_string(), None),
            };

            if let Some(existing_s) = &matched_existing[i] {
                // Update split in place
                sqlx::query(
                    r#"UPDATE splits
                       SET account_id = $1, amount = $2, memo = $3, reconciled_state = $4, reconciled_date = $5
                       WHERE id = $6"#,
                )
                .bind(s.account_id)
                .bind(s.amount)
                .bind(s.memo)
                .bind(rec_state)
                .bind(rec_date)
                .bind(existing_s.id)
                .execute(&mut *tx)
                .await
                .map_err(|e| {
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!({ "error": format!("Failed to update split: {}", e) })),
                    )
                })?;
            } else {
                // Insert new split
                let split_id = s.id.unwrap_or_else(Uuid::new_v4);
                sqlx::query(
                    r#"INSERT INTO splits (id, transaction_id, account_id, amount, memo, reconciled_state, reconciled_date)
                       VALUES ($1, $2, $3, $4, $5, $6, $7)"#,
                )
                .bind(split_id)
                .bind(id)
                .bind(s.account_id)
                .bind(s.amount)
                .bind(s.memo)
                .bind(rec_state)
                .bind(rec_date)
                .execute(&mut *tx)
                .await
                .map_err(|e| {
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!({ "error": format!("Failed to insert new split: {}", e) })),
                    )
                })?;
            }
        }
    }

    tx.commit().await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Failed to commit update: {}", e) })),
        )
    })?;

    // Return updated transaction
    get_transaction(auth, State(pool), Path(id)).await
}

pub async fn delete_transaction(
    auth: AuthUser,
    State(pool): State<PgPool>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let deleted = sqlx::query("DELETE FROM transactions WHERE id = $1 AND user_id = $2")
        .bind(id)
        .bind(auth.id)
        .execute(&pool)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": format!("Database error: {}", e) })),
            )
        })?;

    if deleted.rows_affected() == 0 {
        return Err((
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "Transaction not found" })),
        ));
    }

    Ok(Json(json!({ "message": "Transaction deleted successfully" })))
}

/// Account Register (Ledger View)
/// Returns all entries for this account with chronological running balance!
pub async fn get_account_register(
    auth: AuthUser,
    State(pool): State<PgPool>,
    Path(account_id): Path<Uuid>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    // 1. Get account details ensuring ownership
    let account = sqlx::query_as::<_, (String, String)>(
        "SELECT name, account_type FROM accounts WHERE id = $1 AND user_id = $2",
    )
    .bind(account_id)
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

    let (_, account_type) = account;

    // 2. Fetch all splits for this account ordered chronologically
    let rows = sqlx::query_as::<_, (Uuid, Uuid, NaiveDate, chrono::DateTime<chrono::Utc>, Option<String>, String, Option<String>, Option<String>, String, Decimal)>(
        r#"SELECT s.id, t.id, t.post_date, t.enter_date, t.num, t.description, t.notes, s.memo, s.reconciled_state, s.amount
           FROM splits s
           JOIN transactions t ON s.transaction_id = t.id
           WHERE s.account_id = $1 AND t.user_id = $2
           ORDER BY t.post_date ASC, t.enter_date ASC, s.id ASC"#
    )
    .bind(account_id)
    .bind(auth.id)
    .fetch_all(&pool)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Failed to fetch register splits: {}", e) })),
        )
    })?;

    if rows.is_empty() {
        return Ok(Json(Vec::<RegisterEntry>::new()));
    }

    let tx_ids: Vec<Uuid> = rows.iter().map(|r| r.1).collect();

    // Fetch all splits for these transactions to show transfer accounts and multi-split breakdowns
    let all_tx_splits = sqlx::query_as::<_, (Uuid, Uuid, Uuid, String, String, Decimal, Option<String>, String, Option<chrono::DateTime<chrono::Utc>>)>(
        r#"SELECT s.id, s.transaction_id, s.account_id, a.name, a.account_type, s.amount, s.memo, s.reconciled_state, s.reconciled_date
           FROM splits s
           JOIN accounts a ON s.account_id = a.id
           WHERE s.transaction_id = ANY($1)
           ORDER BY s.amount DESC"#
    )
    .bind(&tx_ids)
    .fetch_all(&pool)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Failed to fetch all splits: {}", e) })),
        )
    })?;

    let mut splits_by_tx: HashMap<Uuid, Vec<SplitWithAccount>> = HashMap::new();
    for (s_id, tx_id, acc_id, acc_name, acc_type, amount, memo, rec_state, rec_date) in all_tx_splits {
        splits_by_tx.entry(tx_id).or_default().push(SplitWithAccount {
            id: s_id,
            transaction_id: tx_id,
            account_id: acc_id,
            account_name: acc_name,
            account_type: acc_type,
            amount,
            memo,
            reconciled_state: rec_state,
            reconciled_date: rec_date,
        });
    }

    let is_asset = is_asset_type(&account_type);
    let is_expense = is_expense_type(&account_type);

    let mut running_sum = Decimal::ZERO;
    let mut register_entries = Vec::new();

    for (split_id, tx_id, post_date, _enter_date, num, description, notes, memo, reconciled_state, amount) in rows {
        running_sum += amount;
        let display_running_balance = calculate_display_balance(&account_type, running_sum);

        let tx_splits = splits_by_tx.get(&tx_id).cloned().unwrap_or_default();

        // Determine transfer account string
        let other_splits: Vec<&SplitWithAccount> = tx_splits.iter().filter(|s| s.account_id != account_id).collect();
        let transfer_account_name = if other_splits.is_empty() {
            "Self Transfer".to_string()
        } else if other_splits.len() == 1 {
            other_splits[0].account_name.clone()
        } else {
            "-- Split Transaction --".to_string()
        };

        // Determine Debit vs Credit columns
        let (debit, credit) = if is_asset || is_expense {
            if amount >= Decimal::ZERO {
                (Some(amount), None)
            } else {
                (None, Some(-amount))
            }
        } else {
            if amount < Decimal::ZERO {
                (None, Some(-amount))
            } else {
                (Some(amount), None)
            }
        };

        register_entries.push(RegisterEntry {
            transaction_id: tx_id,
            split_id,
            post_date,
            num,
            description,
            notes,
            transfer_account_name,
            memo,
            reconciled_state,
            amount,
            debit,
            credit,
            running_balance: display_running_balance,
            all_splits: tx_splits,
        });
    }

    Ok(Json(register_entries))
}
