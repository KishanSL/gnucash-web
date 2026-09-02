use axum::{
    extract::{Multipart, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use chrono::{DateTime, NaiveDate, Utc};
use rusqlite::Connection;
use rust_decimal::Decimal;
use serde::Deserialize;
use serde_json::json;
use sqlx::PgPool;
use std::collections::{HashMap, HashSet};
use std::io::Write;
use tempfile::NamedTempFile;
use uuid::Uuid;

#[derive(Debug, Deserialize)]
pub struct ImportQuery {
    pub wipe: Option<bool>,
    pub default_currency: Option<String>,
}

fn parse_gnucash_guid(guid_str: &str) -> Uuid {
    let clean = guid_str.trim().replace('-', "");
    Uuid::parse_str(&clean).unwrap_or_else(|_| {
        Uuid::new_v5(&Uuid::NAMESPACE_OID, guid_str.as_bytes())
    })
}

fn parse_gnucash_date(date_str: &str) -> NaiveDate {
    let s = date_str.trim();

    // Try YYYY-MM-DD
    if let Ok(d) = NaiveDate::parse_from_str(&s[..10.min(s.len())], "%Y-%m-%d") {
        return d;
    }

    // Try YYYYMMDDHHMMSS (standard GnuCash SQLite)
    if s.len() >= 8 {
        if let Ok(d) = NaiveDate::parse_from_str(&s[..8], "%Y%m%d") {
            return d;
        }
    }

    // Try epoch seconds if numeric
    if let Ok(epoch) = s.parse::<i64>() {
        if let Some(dt) = DateTime::from_timestamp(epoch, 0) {
            return dt.date_naive();
        }
    }

    Utc::now().date_naive()
}

fn normalize_account_type(raw_type: &str) -> String {
    let upper = raw_type.trim().to_uppercase();
    match upper.as_str() {
        "BANK" => "bank".to_string(),
        "CASH" => "cash".to_string(),
        "CREDIT" | "CREDITLINE" => "credit".to_string(),
        "LIABILITY" => "liability".to_string(),
        "EQUITY" => "equity".to_string(),
        "INCOME" => "income".to_string(),
        "EXPENSE" => "expense".to_string(),
        "RECEIVABLE" => "receivable".to_string(),
        "PAYABLE" => "payable".to_string(),
        "STOCK" | "MUTUAL" | "ASSET" => "asset".to_string(),
        _ => "asset".to_string(),
    }
}

pub async fn import_gnucash_sqlite(
    State(pool): State<PgPool>,
    Query(query): Query<ImportQuery>,
    mut multipart: Multipart,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let wipe = query.wipe.unwrap_or(true);
    let default_curr = query.default_currency.unwrap_or_else(|| "INR".to_string());

    // 1. Read file bytes from multipart form
    let mut file_bytes = Vec::new();

    while let Some(field) = multipart.next_field().await.map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": format!("Multipart error: {}", e) })),
        )
    })? {
        let field_name = field.name().unwrap_or_default().to_string();
        if field_name == "file" || field_name == "database" || field_name.is_empty() {
            let data = field.bytes().await.map_err(|e| {
                (
                    StatusCode::BAD_REQUEST,
                    Json(json!({ "error": format!("Failed to read file bytes: {}", e) })),
                )
            })?;
            file_bytes = data.to_vec();
            break;
        }
    }

    if file_bytes.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "No file uploaded. Please upload a .gnucash or SQLite database file." })),
        ));
    }

    // 2. Write to temporary file for SQLite connection
    let mut temp_file = NamedTempFile::new().map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Failed to create temporary file: {}", e) })),
        )
    })?;

    temp_file.write_all(&file_bytes).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Failed to write to temp file: {}", e) })),
        )
    })?;

    let temp_path = temp_file.path().to_path_buf();

    // 3. Process the SQLite database in a blocking task
    let default_curr_clone = default_curr.clone();
    let import_result = tokio::task::spawn_blocking(move || -> Result<(Vec<RawAccount>, Vec<RawTransaction>, Vec<RawSplit>), String> {
        let conn = Connection::open(&temp_path).map_err(|e| format!("Invalid SQLite file: {}", e))?;

        // Verify GnuCash tables exist
        let mut check_stmt = conn.prepare("SELECT name FROM sqlite_master WHERE type='table' AND name IN ('accounts', 'transactions', 'splits')")
            .map_err(|e| format!("SQLite query failed: {}", e))?;
        let table_names: Vec<String> = check_stmt.query_map([], |row| row.get(0))
            .map_err(|e| format!("Failed to read tables: {}", e))?
            .filter_map(|r| r.ok())
            .collect();

        if !table_names.contains(&"accounts".to_string()) || !table_names.contains(&"transactions".to_string()) || !table_names.contains(&"splits".to_string()) {
            return Err("Uploaded database is not a valid GnuCash SQLite file (missing accounts, transactions, or splits tables).".to_string());
        }

        // Fetch commodities if present
        let mut commodity_map: HashMap<String, String> = HashMap::new();
        if let Ok(mut stmt) = conn.prepare("SELECT guid, mnemonic FROM commodities") {
            if let Ok(rows) = stmt.query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            }) {
                for r in rows.flatten() {
                    commodity_map.insert(r.0, r.1);
                }
            }
        }

        // Read Accounts
        let mut acc_stmt = conn.prepare(
            "SELECT guid, name, account_type, commodity_guid, parent_guid, code, description, hidden, placeholder FROM accounts"
        ).map_err(|e| format!("Failed to prepare accounts query: {}", e))?;

        let mut raw_accounts = Vec::new();
        let mut root_guids = HashSet::new();

        let acc_rows = acc_stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, Option<String>>(4)?,
                row.get::<_, Option<String>>(5)?,
                row.get::<_, Option<String>>(6)?,
                row.get::<_, Option<i64>>(7)?,
                row.get::<_, Option<i64>>(8)?,
            ))
        }).map_err(|e| format!("Failed to query accounts: {}", e))?;

        for r in acc_rows.flatten() {
            let (guid, name, acc_type, commodity_guid, parent_guid, code, desc, hidden, placeholder) = r;
            if acc_type.trim().to_uppercase() == "ROOT" {
                root_guids.insert(guid);
                continue;
            }

            let currency = commodity_guid
                .and_then(|cg| commodity_map.get(&cg).cloned())
                .unwrap_or_else(|| default_curr_clone.clone());

            raw_accounts.push(RawAccount {
                guid,
                name,
                account_type: normalize_account_type(&acc_type),
                parent_guid,
                code: code.filter(|s| !s.trim().is_empty()),
                description: desc.filter(|s| !s.trim().is_empty()),
                hidden: hidden.map(|h| h != 0).unwrap_or(false),
                placeholder: placeholder.map(|p| p != 0).unwrap_or(false),
                currency,
            });
        }

        // Clean up parent_guid if parent was a ROOT account
        for acc in &mut raw_accounts {
            if let Some(pg) = &acc.parent_guid {
                if root_guids.contains(pg) {
                    acc.parent_guid = None;
                }
            }
        }

        // Read Transactions
        let mut tx_stmt = conn.prepare(
            "SELECT guid, post_date, num, description FROM transactions"
        ).map_err(|e| format!("Failed to prepare transactions query: {}", e))?;

        let mut raw_transactions = Vec::new();
        let tx_rows = tx_stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, Option<String>>(3)?,
            ))
        }).map_err(|e| format!("Failed to query transactions: {}", e))?;

        for r in tx_rows.flatten() {
            let (guid, post_date_str, num, desc) = r;
            let post_date = parse_gnucash_date(&post_date_str);
            raw_transactions.push(RawTransaction {
                guid,
                post_date,
                num: num.filter(|s| !s.trim().is_empty()),
                description: desc.unwrap_or_else(|| "GnuCash Transaction".to_string()),
            });
        }

        // Read Splits
        let mut split_stmt = conn.prepare(
            "SELECT guid, tx_guid, account_guid, memo, reconcile_state, value_num, value_denom FROM splits"
        ).map_err(|e| format!("Failed to prepare splits query: {}", e))?;

        let mut raw_splits = Vec::new();
        let split_rows = split_stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, Option<String>>(4)?,
                row.get::<_, i64>(5)?,
                row.get::<_, i64>(6)?,
            ))
        }).map_err(|e| format!("Failed to query splits: {}", e))?;

        for r in split_rows.flatten() {
            let (guid, tx_guid, account_guid, memo, rec_state, val_num, val_denom) = r;
            let denom = if val_denom == 0 { 100 } else { val_denom };
            let amount = Decimal::from(val_num) / Decimal::from(denom);

            raw_splits.push(RawSplit {
                guid,
                tx_guid,
                account_guid,
                memo: memo.filter(|s| !s.trim().is_empty()),
                reconciled_state: rec_state.unwrap_or_else(|| "n".to_string()),
                amount,
            });
        }

        Ok((raw_accounts, raw_transactions, raw_splits))
    }).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Worker task failed: {}", e) })),
        )
    })?.map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": e })),
        )
    })?;

    let (raw_accounts, raw_transactions, raw_splits) = import_result;

    // 4. Save to PostgreSQL
    let mut tx = pool.begin().await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Database transaction error: {}", e) })),
        )
    })?;

    if wipe {
        sqlx::query("DELETE FROM splits").execute(&mut *tx).await.map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))))?;
        sqlx::query("DELETE FROM transactions").execute(&mut *tx).await.map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))))?;
        sqlx::query("DELETE FROM accounts").execute(&mut *tx).await.map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))))?;
    }

    // Map of GnuCash GUID string -> Postgres UUID
    let mut account_guid_to_uuid: HashMap<String, Uuid> = HashMap::new();
    for acc in &raw_accounts {
        account_guid_to_uuid.insert(acc.guid.clone(), parse_gnucash_guid(&acc.guid));
    }

    // Insert Accounts (First insert parents, then children to satisfy foreign keys)
    // We can do this in order of dependency depth
    let mut inserted_acc_ids: HashSet<Uuid> = HashSet::new();
    let mut remaining = raw_accounts;
    let mut pass = 0;

    while pass < 10 && !remaining.is_empty() {
        pass += 1;
        let mut next_remaining = Vec::new();
        for acc in remaining {
            let acc_id = *account_guid_to_uuid.get(&acc.guid).unwrap();
            let parent_id = acc.parent_guid.as_ref().and_then(|pg| account_guid_to_uuid.get(pg).copied());

            let can_insert = match parent_id {
                None => true,
                Some(pid) => inserted_acc_ids.contains(&pid),
            };

            if can_insert {
                sqlx::query(
                    r#"INSERT INTO accounts (id, name, code, account_type, parent_id, currency, description, placeholder, hidden)
                       VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
                       ON CONFLICT (id) DO UPDATE SET
                           name = EXCLUDED.name, code = EXCLUDED.code, account_type = EXCLUDED.account_type,
                           parent_id = EXCLUDED.parent_id, currency = EXCLUDED.currency, description = EXCLUDED.description,
                           placeholder = EXCLUDED.placeholder, hidden = EXCLUDED.hidden"#
                )
                .bind(acc_id)
                .bind(&acc.name)
                .bind(&acc.code)
                .bind(&acc.account_type)
                .bind(parent_id)
                .bind(&acc.currency)
                .bind(&acc.description)
                .bind(acc.placeholder)
                .bind(acc.hidden)
                .execute(&mut *tx)
                .await
                .map_err(|e| {
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!({ "error": format!("Failed to insert account '{}': {}", acc.name, e) })),
                    )
                })?;

                inserted_acc_ids.insert(acc_id);
            } else {
                next_remaining.push(acc);
            }
        }
        remaining = next_remaining;
    }

    // If any circular or unresolved parents remain, insert with parent_id = None
    for acc in remaining {
        let acc_id = *account_guid_to_uuid.get(&acc.guid).unwrap();
        sqlx::query(
            r#"INSERT INTO accounts (id, name, code, account_type, parent_id, currency, description, placeholder, hidden)
               VALUES ($1, $2, $3, $4, NULL, $5, $6, $7, $8)
               ON CONFLICT (id) DO NOTHING"#
        )
        .bind(acc_id)
        .bind(&acc.name)
        .bind(&acc.code)
        .bind(&acc.account_type)
        .bind(&acc.currency)
        .bind(&acc.description)
        .bind(acc.placeholder)
        .bind(acc.hidden)
        .execute(&mut *tx)
        .await
        .ok();
        inserted_acc_ids.insert(acc_id);
    }

    // Map of Transaction GUID -> Postgres UUID
    let mut tx_guid_to_uuid: HashMap<String, Uuid> = HashMap::new();
    let mut tx_count = 0;

    for t in &raw_transactions {
        let tx_id = parse_gnucash_guid(&t.guid);
        tx_guid_to_uuid.insert(t.guid.clone(), tx_id);

        sqlx::query(
            r#"INSERT INTO transactions (id, post_date, num, description)
               VALUES ($1, $2, $3, $4)
               ON CONFLICT (id) DO UPDATE SET
                   post_date = EXCLUDED.post_date, num = EXCLUDED.num, description = EXCLUDED.description"#
        )
        .bind(tx_id)
        .bind(t.post_date)
        .bind(&t.num)
        .bind(&t.description)
        .execute(&mut *tx)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": format!("Failed to insert transaction: {}", e) })),
            )
        })?;

        tx_count += 1;
    }

    // Insert Splits
    let mut split_count = 0;
    for s in &raw_splits {
        let tx_id = match tx_guid_to_uuid.get(&s.tx_guid) {
            Some(id) => *id,
            None => continue, // orphaned split
        };

        let acc_id = match account_guid_to_uuid.get(&s.account_guid) {
            Some(id) => *id,
            None => continue, // orphaned split
        };

        let split_id = parse_gnucash_guid(&s.guid);
        let rec_state = match s.reconciled_state.to_lowercase().as_str() {
            "y" => "y",
            "c" => "c",
            _ => "n",
        };

        sqlx::query(
            r#"INSERT INTO splits (id, transaction_id, account_id, amount, memo, reconciled_state)
               VALUES ($1, $2, $3, $4, $5, $6)
               ON CONFLICT (id) DO NOTHING"#
        )
        .bind(split_id)
        .bind(tx_id)
        .bind(acc_id)
        .bind(s.amount)
        .bind(&s.memo)
        .bind(rec_state)
        .execute(&mut *tx)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": format!("Failed to insert split: {}", e) })),
            )
        })?;

        split_count += 1;
    }

    tx.commit().await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Failed to commit imported data: {}", e) })),
        )
    })?;

    Ok(Json(json!({
        "success": true,
        "accounts_imported": inserted_acc_ids.len(),
        "transactions_imported": tx_count,
        "splits_imported": split_count,
        "message": format!(
            "Successfully imported GnuCash SQLite file ({} accounts, {} transactions, {} splits).",
            inserted_acc_ids.len(), tx_count, split_count
        )
    })))
}

struct RawAccount {
    guid: String,
    name: String,
    account_type: String,
    parent_guid: Option<String>,
    code: Option<String>,
    description: Option<String>,
    hidden: bool,
    placeholder: bool,
    currency: String,
}

struct RawTransaction {
    guid: String,
    post_date: NaiveDate,
    num: Option<String>,
    description: String,
}

struct RawSplit {
    guid: String,
    tx_guid: String,
    account_guid: String,
    memo: Option<String>,
    reconciled_state: String,
    amount: Decimal,
}
