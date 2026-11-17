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
use crate::handlers::internal_error;
use crate::models::{
    calculate_display_balance, is_asset_type, is_expense_type, RegisterEntry, RegisterEntryRow, SplitWithAccount,
};

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
    .map_err(|e| internal_error("Failed to fetch account for register", e))?
    .ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "Account not found" })),
        )
    })?;

    let (_, account_type) = account;

    // 2. Fetch all splits for this account ordered chronologically
    let rows = sqlx::query_as::<_, RegisterEntryRow>(
        r#"SELECT t.id as tx_id, s.id as split_id, t.post_date, t.num, t.description, t.notes, s.memo, s.reconciled_state, s.amount
           FROM splits s
           JOIN transactions t ON s.transaction_id = t.id
           WHERE s.account_id = $1 AND t.user_id = $2
           ORDER BY t.post_date ASC, t.enter_date ASC, s.id ASC"#
    )
    .bind(account_id)
    .bind(auth.id)
    .fetch_all(&pool)
    .await
    .map_err(|e| internal_error("Failed to fetch register splits", e))?;

    if rows.is_empty() {
        return Ok(Json(Vec::<RegisterEntry>::new()));
    }

    let tx_ids: Vec<Uuid> = rows.iter().map(|r| r.tx_id).collect();

    // Fetch all splits for these transactions to show transfer accounts and multi-split breakdowns
    let all_tx_splits = sqlx::query_as::<_, SplitWithAccount>(
        r#"SELECT s.id, s.transaction_id, s.account_id, a.name as account_name, a.account_type, s.amount, s.memo, s.reconciled_state, s.reconciled_date
           FROM splits s
           JOIN accounts a ON s.account_id = a.id
           WHERE s.transaction_id = ANY($1)
           ORDER BY s.amount DESC"#
    )
    .bind(&tx_ids)
    .fetch_all(&pool)
    .await
    .map_err(|e| internal_error("Failed to fetch all splits for register", e))?;

    let mut splits_by_tx: HashMap<Uuid, Vec<SplitWithAccount>> = HashMap::new();
    for row in all_tx_splits {
        splits_by_tx.entry(row.transaction_id).or_default().push(row);
    }

    let is_asset = is_asset_type(&account_type);
    let is_expense = is_expense_type(&account_type);

    let mut running_sum = Decimal::ZERO;
    let mut register_entries = Vec::new();

    for row in rows {
        running_sum += row.amount;
        let display_running_balance = calculate_display_balance(&account_type, running_sum);

        let tx_splits = splits_by_tx.get(&row.tx_id).cloned().unwrap_or_default();

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
            if row.amount >= Decimal::ZERO {
                (Some(row.amount), None)
            } else {
                (None, Some(-row.amount))
            }
        } else {
            if row.amount < Decimal::ZERO {
                (None, Some(-row.amount))
            } else {
                (Some(row.amount), None)
            }
        };

        register_entries.push(RegisterEntry {
            transaction_id: row.tx_id,
            split_id: row.split_id,
            post_date: row.post_date,
            num: row.num,
            description: row.description,
            notes: row.notes,
            transfer_account_name,
            memo: row.memo,
            reconciled_state: row.reconciled_state,
            amount: row.amount,
            debit,
            credit,
            running_balance: display_running_balance,
            all_splits: tx_splits,
        });
    }

    Ok(Json(register_entries))
}
