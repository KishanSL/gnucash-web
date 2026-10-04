use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use chrono::{NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::Deserialize;
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

use crate::auth::AuthUser;
use crate::models::{
    calculate_display_balance, is_asset_type, is_expense_type, Account, AccountRow,
    FinishReconcileRequest, ReconcileData, ReconcileSplitItem,
};

#[derive(Debug, Deserialize)]
pub struct ReconcileQuery {
    pub statement_date: Option<NaiveDate>,
}

pub async fn get_reconcile_info(
    auth: AuthUser,
    State(pool): State<PgPool>,
    Path(account_id): Path<Uuid>,
    Query(query): Query<ReconcileQuery>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let statement_date = query.statement_date.unwrap_or_else(|| Utc::now().date_naive());

    // 1. Fetch account ensuring ownership
    let acc_row = sqlx::query_as::<_, AccountRow>(
        "SELECT id, name, code, account_type, parent_id, currency, description, placeholder, hidden, created_at, updated_at
         FROM accounts WHERE id = $1 AND user_id = $2",
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

    // 2. Fetch starting balance (sum of splits that are already reconciled 'y')
    let start_row: Option<(Decimal,)> = sqlx::query_as(
        r#"SELECT COALESCE(SUM(amount), 0)
           FROM splits
           WHERE account_id = $1 AND reconciled_state = 'y'"#
    )
    .bind(account_id)
    .fetch_optional(&pool)
    .await
    .unwrap_or(None);

    let raw_starting_balance = start_row.map(|r| r.0).unwrap_or(Decimal::ZERO);
    let starting_balance = calculate_display_balance(&acc_row.account_type, raw_starting_balance);

    // Total direct balance
    let total_row: Option<(Decimal,)> = sqlx::query_as(
        "SELECT COALESCE(SUM(amount), 0) FROM splits WHERE account_id = $1",
    )
    .bind(account_id)
    .fetch_optional(&pool)
    .await
    .unwrap_or(None);

    let raw_total = total_row.map(|r| r.0).unwrap_or(Decimal::ZERO);
    let ending_balance = calculate_display_balance(&acc_row.account_type, raw_total);

    // 3. Fetch unreconciled splits (reconciled_state != 'y') up to statement date
    let unreconciled_splits = sqlx::query_as::<_, (Uuid, Uuid, NaiveDate, Option<String>, String, Decimal, String)>(
        r#"SELECT s.id, t.id, t.post_date, t.num, t.description, s.amount, s.reconciled_state
           FROM splits s
           JOIN transactions t ON s.transaction_id = t.id
           WHERE s.account_id = $1 AND s.reconciled_state != 'y' AND t.post_date <= $2 AND t.user_id = $3
           ORDER BY t.post_date ASC, t.enter_date ASC"#
    )
    .bind(account_id)
    .bind(statement_date)
    .bind(auth.id)
    .fetch_all(&pool)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Database error: {}", e) })),
        )
    })?;

    let is_asset = is_asset_type(&acc_row.account_type);
    let is_expense = is_expense_type(&acc_row.account_type);

    let mut deposits = Vec::new();
    let mut withdrawals = Vec::new();

    for (s_id, tx_id, post_date, num, description, amount, rec_state) in unreconciled_splits {
        let display_amount = amount.abs();
        let item = ReconcileSplitItem {
            split_id: s_id,
            transaction_id: tx_id,
            post_date,
            num,
            description,
            amount,
            display_amount,
            reconciled_state: rec_state,
        };

        if is_asset || is_expense {
            if amount >= Decimal::ZERO {
                deposits.push(item);
            } else {
                withdrawals.push(item);
            }
        } else {
            if amount < Decimal::ZERO {
                withdrawals.push(item);
            } else {
                deposits.push(item);
            }
        }
    }

    let account = Account {
        id: acc_row.id,
        name: acc_row.name,
        code: acc_row.code,
        account_type: acc_row.account_type,
        parent_id: acc_row.parent_id,
        currency: acc_row.currency,
        description: acc_row.description,
        placeholder: acc_row.placeholder,
        hidden: acc_row.hidden,
        balance: raw_total,
        total_balance: raw_total,
        display_balance: ending_balance,
        created_at: acc_row.created_at,
        updated_at: acc_row.updated_at,
    };

    let data = ReconcileData {
        account,
        statement_date,
        starting_balance,
        ending_balance,
        deposits,
        withdrawals,
    };

    Ok(Json(data))
}

pub async fn finish_reconcile(
    auth: AuthUser,
    State(pool): State<PgPool>,
    Path(account_id): Path<Uuid>,
    Json(payload): Json<FinishReconcileRequest>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    // Verify account belongs to user
    let acc_exists = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM accounts WHERE id = $1 AND user_id = $2",
    )
    .bind(account_id)
    .bind(auth.id)
    .fetch_one(&pool)
    .await
    .unwrap_or(0);

    if acc_exists == 0 {
        return Err((
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "Account not found" })),
        ));
    }

    if payload.reconciled_split_ids.is_empty() {
        return Ok(Json(json!({ "message": "No splits were selected to reconcile." })));
    }

    let result = sqlx::query(
        r#"UPDATE splits
           SET reconciled_state = 'y', reconciled_date = $1
           WHERE account_id = $2 AND id = ANY($3)"#
    )
    .bind(payload.statement_date.and_hms_opt(23, 59, 59).map(|dt| dt.and_utc()))
    .bind(account_id)
    .bind(&payload.reconciled_split_ids)
    .execute(&pool)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Failed to reconcile splits: {}", e) })),
        )
    })?;

    Ok(Json(json!({
        "message": format!("Successfully reconciled {} splits.", result.rows_affected())
    })))
}
