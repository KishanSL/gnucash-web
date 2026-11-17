use axum::http::StatusCode;
use axum::Json;
use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde_json::json;
use sqlx::{PgPool, Postgres, Transaction};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

use super::models::{parse_gnucash_guid_for_user, RawAccount, RawSplit, RawTransaction};
use super::validator::{
    filter_and_validate_transactions, normalize_and_validate_splits, validate_accounts,
    verify_double_entry,
};

type PersistResult<T> = Result<T, (StatusCode, Json<serde_json::Value>)>;

fn server_err(msg: &str) -> (StatusCode, Json<serde_json::Value>) {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({ "error": msg })),
    )
}

/// Detects and breaks parent cycles (Issue #18) and sorts accounts topologically by hierarchy depth (Issue #13).
fn break_cycles_and_sort_accounts(raw_accounts: &mut Vec<RawAccount>) -> HashMap<String, usize> {
    // 1. Clean up parent_guid if parent is ROOT account
    let root_guids: HashSet<String> = raw_accounts
        .iter()
        .filter(|a| a.account_type.trim().to_uppercase() == "ROOT")
        .map(|a| a.guid.clone())
        .collect();

    raw_accounts.retain(|a| a.account_type.trim().to_uppercase() != "ROOT");

    for acc in raw_accounts.iter_mut() {
        if let Some(ref pg) = acc.parent_guid {
            if root_guids.contains(pg) {
                acc.parent_guid = None;
            }
        }
    }

    // 2. Clear dangling parents not in account list
    let account_guid_set: HashSet<String> = raw_accounts.iter().map(|a| a.guid.clone()).collect();
    for acc in raw_accounts.iter_mut() {
        if let Some(ref pg) = acc.parent_guid {
            if !account_guid_set.contains(pg) {
                acc.parent_guid = None;
            }
        }
    }

    // 3. Detect and break parent cycles (Issue #18)
    let mut parent_map: HashMap<String, Option<String>> = raw_accounts
        .iter()
        .map(|a| (a.guid.clone(), a.parent_guid.clone()))
        .collect();

    let mut cyclic_accounts = HashSet::new();
    for acc in raw_accounts.iter() {
        let mut curr = acc.guid.clone();
        let mut visited = HashSet::new();
        visited.insert(curr.clone());

        while let Some(Some(ref p)) = parent_map.get(&curr) {
            if visited.contains(p) {
                cyclic_accounts.insert(curr.clone());
                break;
            }
            visited.insert(p.clone());
            curr = p.clone();
        }
    }

    if !cyclic_accounts.is_empty() {
        for acc in raw_accounts.iter_mut() {
            if cyclic_accounts.contains(&acc.guid) {
                acc.parent_guid = None;
            }
        }
        parent_map = raw_accounts
            .iter()
            .map(|a| (a.guid.clone(), a.parent_guid.clone()))
            .collect();
    }

    let mut depth_map: HashMap<String, usize> = HashMap::new();
    for acc in raw_accounts.iter() {
        let mut curr = acc.guid.clone();
        let mut depth = 0;
        let mut visited = HashSet::new();
        visited.insert(curr.clone());

        while let Some(Some(ref p)) = parent_map.get(&curr) {
            if visited.contains(p) {
                break;
            }
            visited.insert(p.clone());
            depth += 1;
            curr = p.clone();
        }
        depth_map.insert(acc.guid.clone(), depth);
    }

    raw_accounts.sort_by_key(|a| *depth_map.get(&a.guid).unwrap_or(&0));
    depth_map
}

async fn wipe_user_records(tx: &mut Transaction<'_, Postgres>, user_id: Uuid) -> PersistResult<()> {
    sqlx::query("DELETE FROM splits WHERE account_id IN (SELECT id FROM accounts WHERE user_id = $1)")
        .bind(user_id)
        .execute(&mut **tx)
        .await
        .map_err(|e| {
            tracing::error!("Failed to wipe splits: {}", e);
            server_err("Failed to wipe existing data for import")
        })?;

    sqlx::query("DELETE FROM transactions WHERE user_id = $1")
        .bind(user_id)
        .execute(&mut **tx)
        .await
        .map_err(|e| {
            tracing::error!("Failed to wipe transactions: {}", e);
            server_err("Failed to wipe existing data for import")
        })?;

    sqlx::query("DELETE FROM accounts WHERE user_id = $1")
        .bind(user_id)
        .execute(&mut **tx)
        .await
        .map_err(|e| {
            tracing::error!("Failed to wipe accounts: {}", e);
            server_err("Failed to wipe existing data for import")
        })?;

    Ok(())
}

async fn batch_insert_accounts(
    tx: &mut Transaction<'_, Postgres>,
    raw_accounts: &[RawAccount],
    depth_map: &HashMap<String, usize>,
    account_guid_to_uuid: &HashMap<String, Uuid>,
    user_id: Uuid,
) -> PersistResult<usize> {
    let max_depth = depth_map.values().copied().max().unwrap_or(0);
    let mut inserted_acc_count = 0;

    for level in 0..=max_depth {
        let level_accounts: Vec<&RawAccount> = raw_accounts
            .iter()
            .filter(|a| *depth_map.get(&a.guid).unwrap_or(&0) == level)
            .collect();

        if level_accounts.is_empty() {
            continue;
        }

        for chunk in level_accounts.chunks(500) {
            let ids: Vec<Uuid> = chunk.iter().map(|a| *account_guid_to_uuid.get(&a.guid).unwrap()).collect();
            let names: Vec<String> = chunk.iter().map(|a| a.name.clone()).collect();
            let codes: Vec<Option<String>> = chunk.iter().map(|a| a.code.clone()).collect();
            let types: Vec<String> = chunk.iter().map(|a| a.account_type.clone()).collect();
            let parent_ids: Vec<Option<Uuid>> = chunk
                .iter()
                .map(|a| a.parent_guid.as_ref().and_then(|pg| account_guid_to_uuid.get(pg).copied()))
                .collect();
            let currencies: Vec<String> = chunk.iter().map(|a| a.currency.clone()).collect();
            let descs: Vec<Option<String>> = chunk.iter().map(|a| a.description.clone()).collect();
            let placeholders: Vec<bool> = chunk.iter().map(|a| a.placeholder).collect();
            let hiddens: Vec<bool> = chunk.iter().map(|a| a.hidden).collect();
            let user_ids: Vec<Uuid> = vec![user_id; chunk.len()];
            let namespaces: Vec<String> = chunk
                .iter()
                .map(|a| if a.commodity_namespace.is_empty() { "CURRENCY".to_string() } else { a.commodity_namespace.clone() })
                .collect();
            let fractions: Vec<i64> = chunk
                .iter()
                .map(|a| if a.commodity_fraction == 0 { 100 } else { a.commodity_fraction })
                .collect();

            sqlx::query(
                r#"INSERT INTO accounts (id, name, code, account_type, parent_id, currency, description, placeholder, hidden, user_id, commodity_namespace, commodity_fraction)
                   SELECT * FROM UNNEST(
                       $1::uuid[],
                       $2::varchar[],
                       $3::varchar[],
                       $4::varchar[],
                       $5::uuid[],
                       $6::varchar[],
                       $7::text[],
                       $8::boolean[],
                       $9::boolean[],
                       $10::uuid[],
                       $11::varchar[],
                       $12::bigint[]
                   )
                   ON CONFLICT (id) DO UPDATE SET
                       name = EXCLUDED.name,
                       code = EXCLUDED.code,
                       account_type = EXCLUDED.account_type,
                       parent_id = EXCLUDED.parent_id,
                       currency = EXCLUDED.currency,
                       description = EXCLUDED.description,
                       placeholder = EXCLUDED.placeholder,
                       hidden = EXCLUDED.hidden,
                       commodity_namespace = EXCLUDED.commodity_namespace,
                       commodity_fraction = EXCLUDED.commodity_fraction
                   WHERE accounts.user_id = EXCLUDED.user_id"#
            )
            .bind(&ids)
            .bind(&names)
            .bind(&codes)
            .bind(&types)
            .bind(&parent_ids)
            .bind(&currencies)
            .bind(&descs)
            .bind(&placeholders)
            .bind(&hiddens)
            .bind(&user_ids)
            .bind(&namespaces)
            .bind(&fractions)
            .execute(&mut **tx)
            .await
            .map_err(|e| {
                tracing::error!("Failed to batch insert accounts: {}", e);
                server_err("Failed to batch insert accounts")
            })?;

            inserted_acc_count += chunk.len();
        }
    }

    Ok(inserted_acc_count)
}

async fn batch_insert_transactions_and_splits(
    tx: &mut Transaction<'_, Postgres>,
    raw_transactions: &[RawTransaction],
    raw_splits: &[RawSplit],
    tx_guid_to_uuid: &HashMap<String, Uuid>,
    account_guid_to_uuid: &HashMap<String, Uuid>,
    user_id: Uuid,
) -> PersistResult<(usize, usize)> {
    let mut tx_count = 0;
    for chunk in raw_transactions.chunks(1000) {
        let ids: Vec<Uuid> = chunk.iter().map(|t| *tx_guid_to_uuid.get(&t.guid).unwrap()).collect();
        let post_dates: Vec<NaiveDate> = chunk.iter().map(|t| t.post_date).collect();
        let nums: Vec<Option<String>> = chunk.iter().map(|t| t.num.clone()).collect();
        let descs: Vec<String> = chunk.iter().map(|t| t.description.clone()).collect();
        let notes: Vec<Option<String>> = chunk.iter().map(|t| t.notes.clone()).collect();
        let user_ids: Vec<Uuid> = vec![user_id; chunk.len()];

        sqlx::query(
            r#"INSERT INTO transactions (id, post_date, num, description, notes, user_id)
               SELECT * FROM UNNEST(
                   $1::uuid[],
                   $2::date[],
                   $3::varchar[],
                   $4::varchar[],
                   $5::text[],
                   $6::uuid[]
               )
               ON CONFLICT (id) DO UPDATE SET
                   post_date = EXCLUDED.post_date,
                   num = EXCLUDED.num,
                   description = EXCLUDED.description,
                   notes = EXCLUDED.notes
               WHERE transactions.user_id = EXCLUDED.user_id"#
        )
        .bind(&ids)
        .bind(&post_dates)
        .bind(&nums)
        .bind(&descs)
        .bind(&notes)
        .bind(&user_ids)
        .execute(&mut **tx)
        .await
        .map_err(|e| {
            tracing::error!("Failed to batch insert transactions: {}", e);
            server_err("Failed to batch insert transactions")
        })?;

        tx_count += chunk.len();
    }

    // Issue #5: Clear previous splits of imported transactions so no stale/duplicate splits remain
    let imported_tx_uuids: Vec<Uuid> = tx_guid_to_uuid.values().copied().collect();
    for chunk in imported_tx_uuids.chunks(1000) {
        sqlx::query("DELETE FROM splits WHERE transaction_id = ANY($1::uuid[])")
            .bind(chunk)
            .execute(&mut **tx)
            .await
            .map_err(|e| {
                tracing::error!("Failed to clear stale splits: {}", e);
                server_err("Failed to clear stale splits")
            })?;
    }

    // Batch insert splits with UNNEST (Issue #13, Issue #39)
    let mut split_count = 0;
    for chunk in raw_splits.chunks(1000) {
        let ids: Vec<Uuid> = chunk.iter().map(|s| parse_gnucash_guid_for_user(&s.guid, &user_id)).collect();
        let tx_ids: Vec<Uuid> = chunk.iter().map(|s| *tx_guid_to_uuid.get(&s.tx_guid).unwrap()).collect();
        let acc_ids: Vec<Uuid> = chunk.iter().map(|s| *account_guid_to_uuid.get(&s.account_guid).unwrap()).collect();
        let amounts: Vec<Decimal> = chunk.iter().map(|s| s.amount).collect();
        let quantities: Vec<Decimal> = chunk.iter().map(|s| s.quantity).collect();
        let memos: Vec<Option<String>> = chunk.iter().map(|s| s.memo.clone()).collect();
        let rec_states: Vec<String> = chunk.iter().map(|s| {
            match s.reconciled_state.to_lowercase().as_str() {
                "y" => "y".to_string(),
                "c" => "c".to_string(),
                _ => "n".to_string(),
            }
        }).collect();
        let rec_dates: Vec<Option<DateTime<Utc>>> = chunk.iter().map(|s| s.reconcile_date).collect();

        sqlx::query(
            r#"INSERT INTO splits (id, transaction_id, account_id, amount, quantity, memo, reconciled_state, reconciled_date)
               SELECT * FROM UNNEST(
                   $1::uuid[],
                   $2::uuid[],
                   $3::uuid[],
                   $4::numeric[],
                   $5::numeric[],
                   $6::varchar[],
                   $7::varchar[],
                   $8::timestamptz[]
               )
               ON CONFLICT (id) DO UPDATE SET
                   account_id = EXCLUDED.account_id,
                   amount = EXCLUDED.amount,
                   quantity = EXCLUDED.quantity,
                   memo = EXCLUDED.memo,
                   reconciled_state = EXCLUDED.reconciled_state,
                   reconciled_date = EXCLUDED.reconciled_date"#
        )
        .bind(&ids)
        .bind(&tx_ids)
        .bind(&acc_ids)
        .bind(&amounts)
        .bind(&quantities)
        .bind(&memos)
        .bind(&rec_states)
        .bind(&rec_dates)
        .execute(&mut **tx)
        .await
        .map_err(|e| {
            tracing::error!("Failed to batch insert splits: {}", e);
            server_err("Failed to batch insert splits")
        })?;

        split_count += chunk.len();
    }

    Ok((tx_count, split_count))
}

pub async fn save_gnucash_data_to_db(
    pool: &PgPool,
    user_id: Uuid,
    wipe: bool,
    mut raw_accounts: Vec<RawAccount>,
    mut raw_transactions: Vec<RawTransaction>,
    mut raw_splits: Vec<RawSplit>,
    format_desc: &str,
) -> PersistResult<serde_json::Value> {
    // Record all account names (including ROOT accounts) for error reporting (IMP-11)
    let initial_account_names: HashMap<String, String> = raw_accounts
        .iter()
        .map(|a| (a.guid.clone(), a.name.clone()))
        .collect();

    // 1. In-memory topological sort with parent cycle detection (Issue #13 & Issue #18)
    let depth_map = break_cycles_and_sort_accounts(&mut raw_accounts);

    // Fast lookup map for validation and reporting
    let acc_by_guid: HashMap<String, &RawAccount> = raw_accounts.iter().map(|a| (a.guid.clone(), a)).collect();

    // 2. Pre-validate accounts (IMP-11, ACC-11)
    validate_accounts(&raw_accounts, &acc_by_guid)?;

    // 3. Defensive check: default empty/whitespace descriptions (Issue #32, TXN-1)
    for tx in raw_transactions.iter_mut() {
        if tx.description.trim().is_empty() {
            tx.description = "GnuCash Transaction".to_string();
        }
    }

    // 4. Normalize reconciled states and pre-validate splits (ACC-2, IMP-4, IMP-10, IMP-11, REC-2)
    normalize_and_validate_splits(
        &mut raw_splits,
        &raw_transactions,
        &acc_by_guid,
        &initial_account_names,
    )?;

    // 5. Filter out transactions with < 2 splits and validate lengths (Issue #36, IMP-10, IMP-11, TXN-1)
    let skipped_transactions = filter_and_validate_transactions(
        &mut raw_transactions,
        &mut raw_splits,
        &acc_by_guid,
    )?;

    // 6. Verify double-entry balance invariant (ACC-2, IMP-11, Issue #19, Issue #35)
    verify_double_entry(&raw_splits, &raw_transactions, &acc_by_guid)?;

    // 7. Map GnuCash GUID string -> Postgres user-scoped UUID v5
    let mut account_guid_to_uuid: HashMap<String, Uuid> = HashMap::new();
    for acc in &raw_accounts {
        account_guid_to_uuid.insert(acc.guid.clone(), parse_gnucash_guid_for_user(&acc.guid, &user_id));
    }

    let mut tx_guid_to_uuid: HashMap<String, Uuid> = HashMap::new();
    for t in &raw_transactions {
        tx_guid_to_uuid.insert(t.guid.clone(), parse_gnucash_guid_for_user(&t.guid, &user_id));
    }

    // 8. Execute atomic DB transaction
    let mut tx = pool.begin().await.map_err(|e| {
        tracing::error!("Database transaction error: {}", e);
        server_err("Database transaction error")
    })?;

    if wipe {
        wipe_user_records(&mut tx, user_id).await?;
    }

    let inserted_acc_count = batch_insert_accounts(
        &mut tx,
        &raw_accounts,
        &depth_map,
        &account_guid_to_uuid,
        user_id,
    ).await?;

    let (tx_count, split_count) = batch_insert_transactions_and_splits(
        &mut tx,
        &raw_transactions,
        &raw_splits,
        &tx_guid_to_uuid,
        &account_guid_to_uuid,
        user_id,
    ).await?;

    tx.commit().await.map_err(|e| {
        tracing::error!("Failed to commit imported data: {}", e);
        server_err("Failed to commit imported data")
    })?;

    Ok(json!({
        "success": true,
        "accounts_imported": inserted_acc_count,
        "transactions_imported": tx_count,
        "splits_imported": split_count,
        "skipped_transactions": skipped_transactions,
        "format": format_desc,
        "message": format!(
            "Successfully imported {} file ({} accounts, {} transactions, {} splits).",
            format_desc, inserted_acc_count, tx_count, split_count
        )
    }))
}
