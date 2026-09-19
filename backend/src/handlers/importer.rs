use axum::{
    extract::{Multipart, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use chrono::{DateTime, NaiveDate, Utc};
use flate2::read::GzDecoder;
use quick_xml::events::Event;
use quick_xml::reader::Reader;
use rusqlite::Connection;
use rust_decimal::Decimal;
use serde::Deserialize;
use serde_json::json;
use sqlx::PgPool;
use std::collections::{HashMap, HashSet};
use std::io::{Read, Write};
use tempfile::NamedTempFile;
use uuid::Uuid;

#[derive(Debug, Deserialize)]
pub struct ImportQuery {
    pub wipe: Option<bool>,
    pub default_currency: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ImportPathPayload {
    pub path: String,
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
    if s.len() >= 10 {
        if let Ok(d) = NaiveDate::parse_from_str(&s[..10], "%Y-%m-%d") {
            return d;
        }
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
        "ROOT" => "root".to_string(),
        _ => "asset".to_string(),
    }
}

fn parse_fraction(val: &str) -> Option<Decimal> {
    let s = val.trim();
    if let Some((num_str, den_str)) = s.split_once('/') {
        let num: i64 = num_str.trim().parse().ok()?;
        let den: i64 = den_str.trim().parse().ok()?;
        if den == 0 {
            return None;
        }
        let num_dec = Decimal::from(num);
        let den_dec = Decimal::from(den);
        Some(num_dec / den_dec)
    } else {
        s.parse::<Decimal>().ok()
    }
}

fn is_gzip(bytes: &[u8]) -> bool {
    bytes.len() >= 2 && bytes[0] == 0x1f && bytes[1] == 0x8b
}

fn is_sqlite(bytes: &[u8]) -> bool {
    bytes.starts_with(b"SQLite format 3")
}

fn is_xml(bytes: &[u8]) -> bool {
    let s = String::from_utf8_lossy(&bytes[..bytes.len().min(120)]);
    s.trim_start().starts_with("<?xml") || s.contains("<gnc-v2") || s.contains("<gnc:")
}

#[derive(Debug, Default)]
pub struct RawAccount {
    pub guid: String,
    pub name: String,
    pub account_type: String,
    pub parent_guid: Option<String>,
    pub code: Option<String>,
    pub description: Option<String>,
    pub hidden: bool,
    pub placeholder: bool,
    pub currency: String,
}

#[derive(Debug, Default)]
pub struct RawTransaction {
    pub guid: String,
    pub post_date: NaiveDate,
    pub num: Option<String>,
    pub description: String,
}

#[derive(Debug, Default)]
pub struct RawSplit {
    pub guid: String,
    pub tx_guid: String,
    pub account_guid: String,
    pub memo: Option<String>,
    pub reconciled_state: String,
    pub amount: Decimal,
}

fn parse_gnucash_xml(
    xml_bytes: &[u8],
    default_curr: &str,
) -> Result<(Vec<RawAccount>, Vec<RawTransaction>, Vec<RawSplit>), String> {
    let mut reader = Reader::from_reader(xml_bytes);
    reader.config_mut().trim_text(true);

    let mut accounts = Vec::new();
    let mut transactions = Vec::new();
    let mut splits = Vec::new();

    let mut tag_stack: Vec<String> = Vec::new();

    let mut curr_account: Option<RawAccount> = None;
    let mut curr_tx: Option<RawTransaction> = None;
    let mut curr_split: Option<RawSplit> = None;
    let mut curr_slot_key: Option<String> = None;

    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                let name = e.name().as_ref().to_string();
                match name.as_str() {
                    "gnc:account" => {
                        let mut acc = RawAccount::default();
                        acc.currency = default_curr.to_string();
                        curr_account = Some(acc);
                    }
                    "gnc:transaction" => {
                        let mut tx = RawTransaction::default();
                        tx.post_date = Utc::now().date_naive();
                        curr_tx = Some(tx);
                    }
                    "trn:split" => {
                        let mut sp = RawSplit::default();
                        sp.reconciled_state = "n".to_string();
                        if let Some(ref tx) = curr_tx {
                            sp.tx_guid = tx.guid.clone();
                        }
                        curr_split = Some(sp);
                    }
                    _ => {}
                }
                tag_stack.push(name);
            }
            Ok(Event::End(e)) => {
                let name = e.name().as_ref().to_string();
                match name.as_str() {
                    "gnc:account" => {
                        if let Some(mut acc) = curr_account.take() {
                            if !acc.guid.is_empty() {
                                acc.account_type = normalize_account_type(&acc.account_type);
                                accounts.push(acc);
                            }
                        }
                    }
                    "gnc:transaction" => {
                        if let Some(mut tx) = curr_tx.take() {
                            if !tx.guid.is_empty() {
                                if tx.description.is_empty() {
                                    tx.description = "GnuCash Transaction".to_string();
                                }
                                transactions.push(tx);
                            }
                        }
                    }
                    "trn:split" => {
                        if let Some(mut sp) = curr_split.take() {
                            if !sp.guid.is_empty() && !sp.account_guid.is_empty() {
                                if sp.tx_guid.is_empty() {
                                    if let Some(ref tx) = curr_tx {
                                        sp.tx_guid = tx.guid.clone();
                                    }
                                }
                                splits.push(sp);
                            }
                        }
                    }
                    "slot" => {
                        curr_slot_key = None;
                    }
                    _ => {}
                }
                tag_stack.pop();
            }
            Ok(Event::Text(e)) => {
                let text = e.as_ref().trim().to_string();
                if text.is_empty() {
                    continue;
                }

                let current_tag = tag_stack.last().map(|s| s.as_str()).unwrap_or("");
                let parent_tag = if tag_stack.len() >= 2 {
                    tag_stack[tag_stack.len() - 2].as_str()
                } else {
                    ""
                };

                // Account fields
                if let Some(ref mut acc) = curr_account {
                    match current_tag {
                        "act:id" => acc.guid = text.clone(),
                        "act:name" => acc.name = text.clone(),
                        "act:type" => acc.account_type = text.clone(),
                        "act:description" => acc.description = Some(text.clone()),
                        "act:code" => acc.code = Some(text.clone()),
                        "act:parent" => acc.parent_guid = Some(text.clone()),
                        "cmdty:id" => {
                            if parent_tag == "act:commodity" && !text.eq_ignore_ascii_case("template") {
                                acc.currency = text.clone();
                            }
                        }
                        "slot:key" => {
                            curr_slot_key = Some(text.clone());
                        }
                        "slot:value" => {
                            if let Some(ref key) = curr_slot_key {
                                if key == "placeholder" {
                                    acc.placeholder = text.eq_ignore_ascii_case("true");
                                } else if key == "hidden" {
                                    acc.hidden = text.eq_ignore_ascii_case("true");
                                } else if key == "notes" && acc.description.is_none() {
                                    acc.description = Some(text.clone());
                                }
                            }
                        }
                        _ => {}
                    }
                }

                // Split fields (nested inside a transaction)
                if let Some(ref mut sp) = curr_split {
                    match current_tag {
                        "split:id" => sp.guid = text.clone(),
                        "split:account" => sp.account_guid = text.clone(),
                        "split:memo" => sp.memo = Some(text.clone()),
                        "split:reconciled-state" => sp.reconciled_state = text.to_lowercase(),
                        "split:value" => {
                            if let Some(dec) = parse_fraction(&text) {
                                sp.amount = dec;
                            }
                        }
                        _ => {}
                    }
                } else if let Some(ref mut tx) = curr_tx {
                    // Transaction fields
                    match current_tag {
                        "trn:id" => tx.guid = text.clone(),
                        "trn:description" => tx.description = text.clone(),
                        "trn:num" => tx.num = Some(text.clone()),
                        "ts:date" => {
                            if parent_tag == "trn:date-posted" {
                                tx.post_date = parse_gnucash_date(&text);
                            }
                        }
                        "gdate" => {
                            if let Ok(d) = NaiveDate::parse_from_str(&text, "%Y-%m-%d") {
                                tx.post_date = d;
                            }
                        }
                        _ => {}
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(format!("Error parsing XML at position {}: {:?}", reader.buffer_position(), e)),
            _ => {}
        }
        buf.clear();
    }

    Ok((accounts, transactions, splits))
}

fn parse_gnucash_sqlite(
    file_bytes: &[u8],
    default_curr: &str,
) -> Result<(Vec<RawAccount>, Vec<RawTransaction>, Vec<RawSplit>), String> {
    let mut temp_file = NamedTempFile::new().map_err(|e| format!("Temp file error: {}", e))?;
    temp_file.write_all(file_bytes).map_err(|e| format!("Failed to write temp file: {}", e))?;
    let temp_path = temp_file.path().to_path_buf();

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

        let currency = commodity_guid
            .and_then(|cg| commodity_map.get(&cg).cloned())
            .unwrap_or_else(|| default_curr.to_string());

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
}

pub fn parse_gnucash_file(
    file_bytes: &[u8],
    default_curr: &str,
) -> Result<(Vec<RawAccount>, Vec<RawTransaction>, Vec<RawSplit>, &'static str), String> {
    if is_gzip(file_bytes) {
        let mut decoder = GzDecoder::new(file_bytes);
        let mut decompressed = Vec::new();
        decoder
            .read_to_end(&mut decompressed)
            .map_err(|e| format!("Failed to decompress GnuCash gzip archive: {}", e))?;
        let (acc, tx, sp) = parse_gnucash_xml(&decompressed, default_curr)?;
        Ok((acc, tx, sp, "GnuCash (compressed XML)"))
    } else if is_xml(file_bytes) {
        let (acc, tx, sp) = parse_gnucash_xml(file_bytes, default_curr)?;
        Ok((acc, tx, sp, "GnuCash (plain XML)"))
    } else if is_sqlite(file_bytes) {
        let (acc, tx, sp) = parse_gnucash_sqlite(file_bytes, default_curr)?;
        Ok((acc, tx, sp, "GnuCash (SQLite3 database)"))
    } else {
        // Fallback checks
        let mut decoder = GzDecoder::new(file_bytes);
        let mut decompressed = Vec::new();
        if decoder.read_to_end(&mut decompressed).is_ok() {
            if let Ok((acc, tx, sp)) = parse_gnucash_xml(&decompressed, default_curr) {
                return Ok((acc, tx, sp, "GnuCash (compressed XML)"));
            }
        }
        if let Ok((acc, tx, sp)) = parse_gnucash_sqlite(file_bytes, default_curr) {
            return Ok((acc, tx, sp, "GnuCash (SQLite3 database)"));
        }
        if let Ok((acc, tx, sp)) = parse_gnucash_xml(file_bytes, default_curr) {
            return Ok((acc, tx, sp, "GnuCash (plain XML)"));
        }
        Err("Unsupported file format. Please upload a valid .gnucash file (standard compressed XML or SQLite database).".to_string())
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
            Json(json!({ "error": "No file uploaded. Please upload a .gnucash file." })),
        ));
    }

    // 2. Process file parsing in a blocking task
    let default_curr_clone = default_curr.clone();
    let import_result = tokio::task::spawn_blocking(move || {
        parse_gnucash_file(&file_bytes, &default_curr_clone)
    })
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Worker task failed: {}", e) })),
        )
    })?
    .map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": e })),
        )
    })?;

    let (raw_accounts, raw_transactions, raw_splits, format_desc) = import_result;
    let res = save_gnucash_data_to_db(&pool, wipe, raw_accounts, raw_transactions, raw_splits, format_desc).await?;
    Ok(Json(res))
}

pub async fn import_from_path(
    State(pool): State<PgPool>,
    Json(payload): Json<ImportPathPayload>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let wipe = payload.wipe.unwrap_or(true);
    let default_curr = payload.default_currency.unwrap_or_else(|| "INR".to_string());

    let mut file_path = payload.path.trim().to_string();
    if file_path.starts_with("~/") {
        if let Ok(home) = std::env::var("HOME") {
            file_path = format!("{}{}", home, &file_path[1..]);
        }
    }

    let path_buf = std::path::PathBuf::from(&file_path);
    if !path_buf.exists() || !path_buf.is_file() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": format!("File not found at path: {}", file_path) })),
        ));
    }

    let file_bytes = std::fs::read(&path_buf).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Failed to read file: {}", e) })),
        )
    })?;

    let default_curr_clone = default_curr.clone();
    let import_result = tokio::task::spawn_blocking(move || {
        parse_gnucash_file(&file_bytes, &default_curr_clone)
    })
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Worker task failed: {}", e) })),
        )
    })?
    .map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": e })),
        )
    })?;

    let (raw_accounts, raw_transactions, raw_splits, format_desc) = import_result;
    let res = save_gnucash_data_to_db(&pool, wipe, raw_accounts, raw_transactions, raw_splits, format_desc).await?;
    Ok(Json(res))
}

pub async fn list_local_gnucash_files() -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let mut files = Vec::new();
    let home = std::env::var("HOME").unwrap_or_else(|_| "/home/kishan".to_string());

    let search_dirs = vec![
        format!("{}/codebase", home),
        home.clone(),
        std::env::current_dir().map(|p| p.to_string_lossy().to_string()).unwrap_or_default(),
    ];

    let mut seen_paths = HashSet::new();

    for dir_path in search_dirs {
        if dir_path.is_empty() {
            continue;
        }
        let dir = std::path::Path::new(&dir_path);
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.is_file() {
                    let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("");
                    if ext.eq_ignore_ascii_case("gnucash") {
                        let path_str = p.to_string_lossy().to_string();
                        if seen_paths.insert(path_str.clone()) {
                            let metadata = entry.metadata().ok();
                            let size_bytes = metadata.as_ref().map(|m| m.len()).unwrap_or(0);
                            let filename = p.file_name().and_then(|n| n.to_str()).unwrap_or("").to_string();
                            files.push(json!({
                                "path": path_str,
                                "filename": filename,
                                "size_bytes": size_bytes,
                            }));
                        }
                    }
                }
            }
        }
    }

    Ok(Json(json!({ "files": files })))
}

pub async fn save_gnucash_data_to_db(
    pool: &PgPool,
    wipe: bool,
    mut raw_accounts: Vec<RawAccount>,
    raw_transactions: Vec<RawTransaction>,
    raw_splits: Vec<RawSplit>,
    format_desc: &str,
) -> Result<serde_json::Value, (StatusCode, Json<serde_json::Value>)> {
    // 1. Clean up parent_guid if parent is ROOT account
    let root_guids: HashSet<String> = raw_accounts
        .iter()
        .filter(|a| a.account_type.trim().to_uppercase() == "ROOT")
        .map(|a| a.guid.clone())
        .collect();

    raw_accounts.retain(|a| a.account_type.trim().to_uppercase() != "ROOT");

    for acc in &mut raw_accounts {
        if let Some(ref pg) = acc.parent_guid {
            if root_guids.contains(pg) {
                acc.parent_guid = None;
            }
        }
    }

    // 2. Save to PostgreSQL transaction
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

    Ok(json!({
        "success": true,
        "accounts_imported": inserted_acc_ids.len(),
        "transactions_imported": tx_count,
        "splits_imported": split_count,
        "format": format_desc,
        "message": format!(
            "Successfully imported {} file ({} accounts, {} transactions, {} splits).",
            format_desc, inserted_acc_ids.len(), tx_count, split_count
        )
    }))
}
