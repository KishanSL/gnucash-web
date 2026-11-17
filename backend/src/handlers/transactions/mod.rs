mod register;
mod split_sync;
mod validation;

pub use register::get_account_register;

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use chrono::NaiveDate;
use serde::Deserialize;
use serde_json::json;
use sqlx::PgPool;
use std::collections::HashMap;
use uuid::Uuid;

use crate::auth::AuthUser;
use crate::handlers::{internal_error, ApiError};
use crate::models::{
    CreateTransactionRequest, SplitRow, SplitWithAccount, TransactionRow, TransactionWithSplits,
    UpdateTransactionRequest,
};
use split_sync::{match_existing_splits, resolve_reconcile_state};
use validation::{validate_description, validate_num, validate_splits, verify_split_accounts};

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
        .map_err(|e| internal_error("Failed to list transactions", e))?;

    if rows.is_empty() {
        return Ok(Json(Vec::<TransactionWithSplits>::new()));
    }

    let tx_ids: Vec<Uuid> = rows.iter().map(|r| r.id).collect();

    // Fetch splits for all returned transactions
    let split_rows = sqlx::query_as::<_, SplitWithAccount>(
        r#"SELECT s.id, s.transaction_id, s.account_id, a.name as account_name, a.account_type, s.amount, s.memo, s.reconciled_state, s.reconciled_date
           FROM splits s
           JOIN accounts a ON s.account_id = a.id
           WHERE s.transaction_id = ANY($1)
           ORDER BY s.amount DESC"#
    )
    .bind(&tx_ids)
    .fetch_all(&pool)
    .await
    .map_err(|e| internal_error("Failed to fetch splits", e))?;

    let mut splits_by_tx: HashMap<Uuid, Vec<SplitWithAccount>> = HashMap::new();
    for row in split_rows {
        splits_by_tx.entry(row.transaction_id).or_default().push(row);
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
    .map_err(|e| internal_error("Failed to get transaction", e))?
    .ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "Transaction not found" })),
        )
    })?;

    let splits = sqlx::query_as::<_, SplitWithAccount>(
        r#"SELECT s.id, s.transaction_id, s.account_id, a.name as account_name, a.account_type, s.amount, s.memo, s.reconciled_state, s.reconciled_date
           FROM splits s
           JOIN accounts a ON s.account_id = a.id
           WHERE s.transaction_id = $1
           ORDER BY s.amount DESC"#
    )
    .bind(id)
    .fetch_all(&pool)
    .await
    .map_err(|e| internal_error("Failed to fetch splits", e))?;

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
) -> Result<impl IntoResponse, ApiError> {
    let description = validate_description(&payload.description)?;
    let num = validate_num(payload.num)?;
    validate_splits(&payload.splits)?;

    let mut tx = pool.begin().await.map_err(|e| internal_error("Failed to start transaction", e))?;
    let account_names = verify_split_accounts(&mut tx, auth.id, &payload.splits).await?;

    let tx_row = sqlx::query_as::<_, TransactionRow>(
        r#"INSERT INTO transactions (post_date, num, description, notes, user_id)
           VALUES ($1, $2, $3, $4, $5)
           RETURNING id, post_date, enter_date, num, description, notes, created_at, updated_at"#
    )
    .bind(payload.post_date)
    .bind(num)
    .bind(description)
    .bind(payload.notes)
    .bind(auth.id)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| internal_error("Failed to insert transaction", e))?;

    let mut created_splits = Vec::new();
    for s in payload.splits {
        let (rec_state, rec_date) = resolve_reconcile_state(&s, None);

        let split_row = sqlx::query_as::<_, SplitRow>(
            r#"INSERT INTO splits (transaction_id, account_id, amount, quantity, memo, reconciled_state, reconciled_date)
               VALUES ($1, $2, $3, $3, $4, $5, $6)
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
        .map_err(|e| internal_error("Failed to insert split", e))?;

        created_splits.push(split_row);
    }

    tx.commit().await.map_err(|e| internal_error("Failed to commit transaction", e))?;

    // Format output
    let splits_with_acc = created_splits
        .into_iter()
        .map(|s| SplitWithAccount {
            id: s.id,
            transaction_id: s.transaction_id,
            account_id: s.account_id,
            account_name: account_names.get(&s.account_id).cloned().unwrap_or_default(),
            account_type: String::new(),
            amount: s.amount,
            memo: s.memo,
            reconciled_state: s.reconciled_state,
            reconciled_date: s.reconciled_date,
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
) -> Result<impl IntoResponse, ApiError> {
    let existing = sqlx::query_as::<_, TransactionRow>(
        "SELECT id, post_date, enter_date, num, description, notes, created_at, updated_at
         FROM transactions WHERE id = $1 AND user_id = $2",
    )
    .bind(id)
    .bind(auth.id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| internal_error("Failed to fetch transaction for update", e))?
    .ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "Transaction not found" })),
        )
    })?;

    let post_date = payload.post_date.unwrap_or(existing.post_date);
    let num = match payload.num {
        Some(n) => validate_num(n)?,
        None => existing.num,
    };
    let description = match payload.description {
        Some(ref d) => validate_description(d)?,
        None => existing.description,
    };
    let notes = match payload.notes {
        Some(n) => n.map(|s| s.trim().to_string()).filter(|s| !s.is_empty()),
        None => existing.notes,
    };
    if let Some(ref new_splits) = payload.splits {
        validate_splits(new_splits)?;
    }

    let mut tx = pool.begin().await.map_err(|e| internal_error("Failed to begin transaction for update", e))?;

    sqlx::query(
        r#"UPDATE transactions
           SET post_date = $1, num = $2, description = $3, notes = $4, updated_at = NOW()
           WHERE id = $5 AND user_id = $6"#
    )
    .bind(post_date)
    .bind(num)
    .bind(description)
    .bind(notes)
    .bind(id)
    .bind(auth.id)
    .execute(&mut *tx)
    .await
    .map_err(|e| internal_error("Failed to update transaction", e))?;

    if let Some(new_splits) = payload.splits {
        verify_split_accounts(&mut tx, auth.id, &new_splits).await?;

        // Fetch existing splits for this transaction to preserve reconciled state and IDs
        let existing_splits = sqlx::query_as::<_, SplitRow>(
            "SELECT id, transaction_id, account_id, amount, memo, reconciled_state, reconciled_date
             FROM splits WHERE transaction_id = $1",
        )
        .bind(id)
        .fetch_all(&mut *tx)
        .await
        .map_err(|e| internal_error("Failed to fetch existing splits", e))?;

        let matched_existing = match_existing_splits(&new_splits, &existing_splits);

        // Delete any existing splits that are no longer part of this transaction
        let remaining_split_ids: Vec<Uuid> = matched_existing
            .iter()
            .filter_map(|m| m.as_ref().map(|e| e.id))
            .collect();

        sqlx::query("DELETE FROM splits WHERE transaction_id = $1 AND NOT (id = ANY($2))")
            .bind(id)
            .bind(&remaining_split_ids)
            .execute(&mut *tx)
            .await
            .map_err(|e| internal_error("Failed to remove deleted splits", e))?;

        // Update or insert splits
        for (s, matched) in new_splits.into_iter().zip(matched_existing) {
            let (rec_state, rec_date) = resolve_reconcile_state(&s, matched.as_ref());

            if let Some(existing_s) = matched {
                // Update split in place
                sqlx::query(
                    r#"UPDATE splits
                       SET account_id = $1, amount = $2, quantity = $2, memo = $3, reconciled_state = $4, reconciled_date = $5
                       WHERE id = $6"#,
                )
                .bind(s.account_id)
                .bind(s.amount)
                .bind(s.memo)
                .bind(&rec_state)
                .bind(rec_date)
                .bind(existing_s.id)
                .execute(&mut *tx)
                .await
                .map_err(|e| internal_error("Failed to update split", e))?;
            } else {
                // Issue #17: Never use client-supplied split id as primary key on insert; always generate fresh UUID
                sqlx::query(
                    r#"INSERT INTO splits (id, transaction_id, account_id, amount, quantity, memo, reconciled_state, reconciled_date)
                       VALUES ($1, $2, $3, $4, $4, $5, $6, $7)"#,
                )
                .bind(Uuid::new_v4())
                .bind(id)
                .bind(s.account_id)
                .bind(s.amount)
                .bind(s.memo)
                .bind(&rec_state)
                .bind(rec_date)
                .execute(&mut *tx)
                .await
                .map_err(|e| internal_error("Failed to insert new split", e))?;
            }
        }
    }

    tx.commit().await.map_err(|e| internal_error("Failed to commit update", e))?;

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
        .map_err(|e| internal_error("Failed to delete transaction", e))?;

    if deleted.rows_affected() == 0 {
        return Err((
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "Transaction not found" })),
        ));
    }

    Ok(Json(json!({ "message": "Transaction deleted successfully" })))
}
