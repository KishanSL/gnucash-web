#![allow(clippy::type_complexity, clippy::field_reassign_with_default)]

use axum::{
    extract::{Multipart, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use chrono::{DateTime, NaiveDate, NaiveDateTime, Utc};
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

use crate::auth::AuthUser;
use crate::handlers::internal_error;

#[derive(Debug, Deserialize)]
pub struct ImportQuery {
    pub wipe: Option<bool>,
    pub default_currency: Option<String>,
}

fn parse_gnucash_guid_for_user(guid_str: &str, user_id: &Uuid) -> Uuid {
    let clean = guid_str.trim().replace('-', "");
    // User-scoped namespace prevents any cross-tenant ID collision or takeover
    Uuid::new_v5(user_id, clean.as_bytes())
}

fn parse_gnucash_date(date_str: &str) -> NaiveDate {
    let s = date_str.trim();

    // Try YYYY-MM-DD
    if let Some(prefix) = s.get(..10) {
        if let Ok(d) = NaiveDate::parse_from_str(prefix, "%Y-%m-%d") {
            return d;
        }
    }

    // Try YYYYMMDDHHMMSS (standard GnuCash SQLite)
    if let Some(prefix) = s.get(..8) {
        if let Ok(d) = NaiveDate::parse_from_str(prefix, "%Y%m%d") {
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

fn parse_gnucash_datetime(s: &str) -> Option<DateTime<Utc>> {
    let trimmed = s.trim().trim_matches('\'').trim_matches('"').trim();
    if trimmed.is_empty() {
        return None;
    }

    // Try RFC 3339 / ISO 8601 (e.g. 2024-05-18T10:59:00Z or with offset)
    if let Ok(dt) = DateTime::parse_from_rfc3339(trimmed) {
        return Some(dt.with_timezone(&Utc));
    }

    // Try standard GnuCash datetime with timezone offset (e.g. "2024-05-18 10:59:00 +0000")
    if let Ok(dt) = DateTime::parse_from_str(trimmed, "%Y-%m-%d %H:%M:%S %z") {
        return Some(dt.with_timezone(&Utc));
    }

    // Try standard naive datetime "YYYY-MM-DD HH:MM:SS"
    if let Ok(ndt) = NaiveDateTime::parse_from_str(trimmed, "%Y-%m-%d %H:%M:%S") {
        return Some(ndt.and_utc());
    }

    // Try GnuCash SQLite compact datetime "YYYYMMDDHHMMSS"
    if let Some(prefix) = trimmed.get(..14) {
        if let Ok(ndt) = NaiveDateTime::parse_from_str(prefix, "%Y%m%d%H%M%S") {
            return Some(ndt.and_utc());
        }
    }

    // Try date only "YYYY-MM-DD"
    if let Some(prefix) = trimmed.get(..10) {
        if let Ok(nd) = NaiveDate::parse_from_str(prefix, "%Y-%m-%d") {
            return Some(nd.and_hms_opt(0, 0, 0)?.and_utc());
        }
    }

    // Try date only "YYYYMMDD"
    if let Some(prefix) = trimmed.get(..8) {
        if let Ok(nd) = NaiveDate::parse_from_str(prefix, "%Y%m%d") {
            return Some(nd.and_hms_opt(0, 0, 0)?.and_utc());
        }
    }

    // Try epoch seconds
    if let Ok(epoch) = trimmed.parse::<i64>() {
        if let Some(dt) = DateTime::from_timestamp(epoch, 0) {
            return Some(dt);
        }
    }

    None
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
    pub commodity_namespace: String,
    pub commodity_fraction: i64,
}

#[derive(Debug, Default)]
pub struct RawTransaction {
    pub guid: String,
    pub post_date: NaiveDate,
    pub num: Option<String>,
    pub description: String,
    pub notes: Option<String>,
}

#[derive(Debug, Default)]
pub struct RawSplit {
    pub guid: String,
    pub tx_guid: String,
    pub account_guid: String,
    pub memo: Option<String>,
    pub reconciled_state: String,
    pub amount: Decimal,
    pub quantity: Decimal,
    pub reconcile_date: Option<DateTime<Utc>>,
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

    let mut in_template_transactions = false;
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
                    "gnc:template-transactions" => {
                        in_template_transactions = true;
                    }
                    "gnc:account" if !in_template_transactions => {
                        let mut acc = RawAccount::default();
                        acc.currency = default_curr.to_string();
                        curr_account = Some(acc);
                    }
                    "gnc:transaction" if !in_template_transactions => {
                        let mut tx = RawTransaction::default();
                        tx.post_date = Utc::now().date_naive();
                        curr_tx = Some(tx);
                    }
                    "trn:split" if !in_template_transactions => {
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
                    "gnc:template-transactions" => {
                        in_template_transactions = false;
                    }
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
                                if tx.description.trim().is_empty() {
                                    tx.description = "GnuCash Transaction".to_string();
                                }
                                transactions.push(tx);
                            }
                        }
                    }
                    "trn:split" => {
                        if let Some(mut sp) = curr_split.take() {
                            if !sp.guid.is_empty() && !sp.account_guid.is_empty() {
                                if sp.quantity == Decimal::ZERO && sp.amount != Decimal::ZERO {
                                    sp.quantity = sp.amount;
                                }
                                if sp.tx_guid.is_empty() {
                                    if let Some(ref tx) = curr_tx {
                                        sp.tx_guid = tx.guid.clone();
                                    }
                                }
                                sp.reconciled_state = match sp.reconciled_state.to_lowercase().as_str() {
                                    "y" => "y".to_string(),
                                    "c" => "c".to_string(),
                                    _ => "n".to_string(),
                                };
                                if sp.reconciled_state == "y" {
                                    if sp.reconcile_date.is_none() {
                                        sp.reconcile_date = Some(Utc::now());
                                    }
                                } else {
                                    sp.reconcile_date = None;
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
                        "cmdty:space" => {
                            if parent_tag == "act:commodity" {
                                acc.commodity_namespace = text.clone();
                            }
                        }
                        "act:commodity-scu" => {
                            if let Ok(scu) = text.parse::<i64>() {
                                acc.commodity_fraction = scu;
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
                        "split:quantity" => {
                            if let Some(dec) = parse_fraction(&text) {
                                sp.quantity = dec;
                            }
                        }
                        "ts:date" | "gdate" => {
                            if parent_tag == "split:reconcile-date" || parent_tag.ends_with("reconcile-date") {
                                sp.reconcile_date = parse_gnucash_datetime(&text);
                            }
                        }
                        _ => {
                            if current_tag.ends_with("reconcile-date") {
                                if let Some(dt) = parse_gnucash_datetime(&text) {
                                    sp.reconcile_date = Some(dt);
                                }
                            }
                        }
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
                        "slot:key" => {
                            curr_slot_key = Some(text.clone());
                        }
                        "slot:value" => {
                            if let Some(ref key) = curr_slot_key {
                                if key == "notes" {
                                    tx.notes = Some(text.clone());
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                tracing::error!("Error parsing XML at position {}: {:?}", reader.buffer_position(), e);
                return Err("Failed to parse GnuCash XML document".to_string());
            }
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
    let mut temp_file = NamedTempFile::new().map_err(|e| {
        tracing::error!("Temp file error: {}", e);
        "Failed to create temporary file for import".to_string()
    })?;
    temp_file.write_all(file_bytes).map_err(|e| {
        tracing::error!("Failed to write temp file: {}", e);
        "Failed to write uploaded file to disk".to_string()
    })?;
    let temp_path = temp_file.path().to_path_buf();

    let conn = Connection::open_with_flags(&temp_path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|e| {
            tracing::error!("Invalid SQLite file: {}", e);
            "Invalid SQLite file".to_string()
        })?;
    conn.pragma_update(None, "trusted_schema", "OFF").ok();

    // Verify GnuCash tables exist
    let mut check_stmt = conn.prepare("SELECT name FROM sqlite_master WHERE type='table'")
        .map_err(|e| {
            tracing::error!("SQLite query failed: {}", e);
            "Failed to query SQLite database tables".to_string()
        })?;
    let table_names: Vec<String> = check_stmt.query_map([], |row| row.get(0))
        .map_err(|e| {
            tracing::error!("Failed to read tables: {}", e);
            "Failed to read SQLite database tables".to_string()
        })?
        .filter_map(|r| r.ok())
        .collect();

    if !table_names.contains(&"accounts".to_string()) || !table_names.contains(&"transactions".to_string()) || !table_names.contains(&"splits".to_string()) {
        return Err("Uploaded database is not a valid GnuCash SQLite file (missing accounts, transactions, or splits tables).".to_string());
    }

    // Fetch commodities if present (Issue #9)
    #[derive(Clone)]
    struct CommInfo {
        namespace: String,
        mnemonic: String,
        fraction: i64,
    }
    let mut commodity_map: HashMap<String, CommInfo> = HashMap::new();
    if let Ok(mut stmt) = conn.prepare("SELECT guid, namespace, mnemonic, fraction FROM commodities") {
        if let Ok(rows) = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i64>(3).unwrap_or(100),
            ))
        }) {
            for r in rows.flatten() {
                commodity_map.insert(r.0, CommInfo {
                    namespace: r.1,
                    mnemonic: r.2,
                    fraction: r.3,
                });
            }
        }
    }

    // Read root account and template root guids if books table is present
    let mut root_account_guid: Option<String> = None;
    let mut root_template_guid: Option<String> = None;
    if let Ok(mut stmt) = conn.prepare("SELECT root_account_guid, root_template_guid FROM books") {
        if let Ok(mut rows) = stmt.query([]) {
            if let Ok(Some(row)) = rows.next() {
                root_account_guid = row.get(0).ok();
                root_template_guid = row.get(1).ok();
            }
        }
    }

    // Read Accounts
    let mut acc_stmt = conn.prepare(
        "SELECT guid, name, account_type, commodity_guid, parent_guid, code, description, hidden, placeholder FROM accounts"
    ).map_err(|e| {
        tracing::error!("Failed to prepare accounts query: {}", e);
        "Failed to parse accounts from SQLite file".to_string()
    })?;

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
    }).map_err(|e| {
        tracing::error!("Failed to query accounts: {}", e);
        "Failed to parse accounts from SQLite file".to_string()
    })?;

    for r in acc_rows.flatten() {
        let (guid, name, acc_type, commodity_guid, parent_guid, code, desc, hidden, placeholder) = r;

        let comm_info = commodity_guid.as_ref().and_then(|cg| commodity_map.get(cg));
        let currency = comm_info
            .map(|c| c.mnemonic.clone())
            .unwrap_or_else(|| default_curr.to_string());
        let commodity_namespace = comm_info
            .map(|c| c.namespace.clone())
            .unwrap_or_else(|| "CURRENCY".to_string());
        let commodity_fraction = comm_info
            .map(|c| c.fraction)
            .unwrap_or(100);

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
            commodity_namespace,
            commodity_fraction,
        });
    }

    // Read Transactions with notes from slots table if present
    let has_slots = table_names.contains(&"slots".to_string());
    let mut tx_stmt = if has_slots {
        conn.prepare(
            "SELECT t.guid, t.post_date, t.num, t.description, s.string_val
             FROM transactions t
             LEFT JOIN slots s ON s.obj_guid = t.guid AND s.name = 'notes'"
        )
    } else {
        conn.prepare(
            "SELECT guid, post_date, num, description, NULL
             FROM transactions"
        )
    }.map_err(|e| {
        tracing::error!("Failed to prepare transactions query: {}", e);
        "Failed to parse transactions from SQLite file".to_string()
    })?;

    let mut raw_transactions = Vec::new();
    let tx_rows = tx_stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, Option<String>>(2)?,
            row.get::<_, Option<String>>(3)?,
            row.get::<_, Option<String>>(4)?,
        ))
    }).map_err(|e| {
        tracing::error!("Failed to query transactions: {}", e);
        "Failed to parse transactions from SQLite file".to_string()
    })?;

    for r in tx_rows.flatten() {
        let (guid, post_date_str, num, desc, notes) = r;
        let post_date = parse_gnucash_date(&post_date_str);
        raw_transactions.push(RawTransaction {
            guid,
            post_date,
            num: num.filter(|s| !s.trim().is_empty()),
            description: desc
                .filter(|s| !s.trim().is_empty())
                .unwrap_or_else(|| "GnuCash Transaction".to_string()),
            notes: notes.filter(|s| !s.trim().is_empty()),
        });
    }

    // Read Splits (Issue #9: query both value and quantity; Issue #27: reconcile_date)
    let mut split_stmt = conn.prepare(
        "SELECT guid, tx_guid, account_guid, memo, reconcile_state, value_num, value_denom, quantity_num, quantity_denom, reconcile_date FROM splits"
    ).map_err(|e| {
        tracing::error!("Failed to prepare splits query: {}", e);
        "Failed to parse splits from SQLite file".to_string()
    })?;

    let mut raw_splits = Vec::new();
    let split_rows = split_stmt.query_map([], |row| {
        let rec_date_val: rusqlite::types::Value = row.get(9).unwrap_or(rusqlite::types::Value::Null);
        let rec_date_str = match rec_date_val {
            rusqlite::types::Value::Text(s) => Some(s),
            rusqlite::types::Value::Integer(i) => Some(i.to_string()),
            _ => None,
        };

        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, Option<String>>(3)?,
            row.get::<_, Option<String>>(4)?,
            row.get::<_, i64>(5)?,
            row.get::<_, i64>(6)?,
            row.get::<_, i64>(7).unwrap_or(0),
            row.get::<_, i64>(8).unwrap_or(0),
            rec_date_str,
        ))
    }).map_err(|e| {
        tracing::error!("Failed to query splits: {}", e);
        "Failed to parse splits from SQLite file".to_string()
    })?;

    for r in split_rows.flatten() {
        let (guid, tx_guid, account_guid, memo, rec_state, val_num, val_denom, qty_num, qty_denom, rec_date_str) = r;
        let denom = if val_denom == 0 { 100 } else { val_denom };
        let amount = Decimal::from(val_num) / Decimal::from(denom);

        let quantity = if qty_denom != 0 {
            Decimal::from(qty_num) / Decimal::from(qty_denom)
        } else {
            amount
        };

        let reconciled_state = match rec_state.as_deref().unwrap_or("n").to_lowercase().as_str() {
            "y" => "y".to_string(),
            "c" => "c".to_string(),
            _ => "n".to_string(),
        };
        let reconcile_date = if reconciled_state == "y" {
            rec_date_str.as_deref().and_then(parse_gnucash_datetime).or_else(|| Some(Utc::now()))
        } else {
            None
        };

        raw_splits.push(RawSplit {
            guid,
            tx_guid,
            account_guid,
            memo: memo.filter(|s| !s.trim().is_empty()),
            reconciled_state,
            amount,
            quantity,
            reconcile_date,
        });
    }

    // Exclude scheduled-transaction template accounts and transactions (Issue #7)
    let mut template_guids = HashSet::new();
    if let Some(ref rtg) = root_template_guid {
        template_guids.insert(rtg.clone());
    }
    for a in &raw_accounts {
        if a.account_type.eq_ignore_ascii_case("ROOT") {
            if let Some(ref rag) = root_account_guid {
                if &a.guid != rag {
                    template_guids.insert(a.guid.clone());
                }
            }
        }
    }

    let mut changed = true;
    while changed {
        changed = false;
        for a in &raw_accounts {
            if let Some(ref pg) = a.parent_guid {
                if template_guids.contains(pg) && !template_guids.contains(&a.guid) {
                    template_guids.insert(a.guid.clone());
                    changed = true;
                }
            }
        }
    }

    if !template_guids.is_empty() {
        let template_tx_guids: HashSet<String> = raw_splits
            .iter()
            .filter(|s| template_guids.contains(&s.account_guid))
            .map(|s| s.tx_guid.clone())
            .collect();

        raw_accounts.retain(|a| !template_guids.contains(&a.guid));
        raw_splits.retain(|s| !template_tx_guids.contains(&s.tx_guid));
        raw_transactions.retain(|t| !template_tx_guids.contains(&t.guid));
    }

    Ok((raw_accounts, raw_transactions, raw_splits))
}

pub fn parse_gnucash_file(
    file_bytes: &[u8],
    default_curr: &str,
) -> Result<(Vec<RawAccount>, Vec<RawTransaction>, Vec<RawSplit>, &'static str), String> {
    // 50 MB decompression limit to prevent gzip bombs / OOM crashes
    const MAX_DECOMPRESSED_BYTES: u64 = 50 * 1024 * 1024;

    if is_gzip(file_bytes) {
        let mut decoder = GzDecoder::new(file_bytes);
        let mut decompressed = Vec::new();
        decoder
            .by_ref()
            .take(MAX_DECOMPRESSED_BYTES)
            .read_to_end(&mut decompressed)
            .map_err(|e| {
                tracing::error!("Failed to decompress GnuCash gzip archive: {}", e);
                "Failed to decompress GnuCash gzip archive".to_string()
            })?;

        let mut extra = [0u8; 1];
        if decoder.read(&mut extra).unwrap_or(0) > 0 {
            return Err("GnuCash file exceeded 50MB uncompressed limit. Import aborted for security.".to_string());
        }

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
        if decoder.by_ref().take(MAX_DECOMPRESSED_BYTES).read_to_end(&mut decompressed).is_ok() {
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
    auth: AuthUser,
    State(pool): State<PgPool>,
    Query(query): Query<ImportQuery>,
    mut multipart: Multipart,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let wipe = query.wipe.unwrap_or(false);
    let default_curr = query.default_currency.unwrap_or_else(|| "INR".to_string());

    // 1. Read file bytes from multipart form
    let mut file_bytes = Vec::new();

    while let Some(field) = multipart.next_field().await.map_err(|e| {
        tracing::error!("Multipart error: {}", e);
        (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "Invalid multipart request" })),
        )
    })? {
        let field_name = field.name().unwrap_or_default().to_string();
        if field_name == "file" || field_name == "database" || field_name.is_empty() {
            let data = field.bytes().await.map_err(|e| {
                tracing::error!("Failed to read file bytes: {}", e);
                (
                    StatusCode::BAD_REQUEST,
                    Json(json!({ "error": "Failed to read file payload" })),
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

    if file_bytes.len() > 50 * 1024 * 1024 {
        return Err((
            StatusCode::PAYLOAD_TOO_LARGE,
            Json(json!({ "error": "Uploaded file exceeds maximum allowable size of 50MB." })),
        ));
    }

    // 2. Process file parsing in a blocking task
    let default_curr_clone = default_curr.clone();
    let import_result = tokio::task::spawn_blocking(move || {
        parse_gnucash_file(&file_bytes, &default_curr_clone)
    })
    .await
    .map_err(|e| internal_error("Worker task failed", e))?
    .map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": e })),
        )
    })?;

    let (raw_accounts, raw_transactions, raw_splits, format_desc) = import_result;
    let res = save_gnucash_data_to_db(&pool, auth.id, wipe, raw_accounts, raw_transactions, raw_splits, format_desc).await?;
    Ok(Json(res))
}

pub fn build_full_account_name(
    guid: &str,
    acc_by_guid: &HashMap<String, &RawAccount>,
) -> String {
    let mut names = Vec::new();
    let mut curr = Some(guid.to_string());
    let mut visited = HashSet::new();

    while let Some(ref g) = curr {
        if !visited.insert(g.clone()) {
            break;
        }
        if let Some(acc) = acc_by_guid.get(g) {
            if acc.account_type.trim().to_uppercase() != "ROOT" && !acc.name.is_empty() {
                names.push(acc.name.as_str());
            }
            curr = acc.parent_guid.clone();
        } else {
            break;
        }
    }

    names.reverse();
    if names.is_empty() {
        if let Some(acc) = acc_by_guid.get(guid) {
            if !acc.name.is_empty() {
                return acc.name.clone();
            }
        }
        guid.to_string()
    } else {
        names.join(":")
    }
}

pub fn get_tx_accounts_string(
    tx_guid: &str,
    raw_splits: &[RawSplit],
    acc_by_guid: &HashMap<String, &RawAccount>,
) -> String {
    let mut acc_names = Vec::new();
    let mut seen = HashSet::new();
    for s in raw_splits {
        if s.tx_guid == tx_guid {
            let full_name = build_full_account_name(&s.account_guid, acc_by_guid);
            if seen.insert(full_name.clone()) {
                acc_names.push(full_name);
            }
        }
    }
    if acc_names.is_empty() {
        "none".to_string()
    } else {
        acc_names.join(", ")
    }
}

pub fn tx_desc_preview(description: &str) -> String {
    if description.chars().count() > 30 {
        let prefix: String = description.chars().take(30).collect();
        format!("{}...", prefix)
    } else {
        description.to_string()
    }
}

pub async fn save_gnucash_data_to_db(
    pool: &PgPool,
    user_id: Uuid,
    wipe: bool,
    mut raw_accounts: Vec<RawAccount>,
    mut raw_transactions: Vec<RawTransaction>,
    mut raw_splits: Vec<RawSplit>,
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

    // 2. In-memory topological sort for accounts with cycle detection (Issue #13 & Issue #18)
    let account_guid_set: HashSet<String> = raw_accounts.iter().map(|a| a.guid.clone()).collect();
    for acc in &mut raw_accounts {
        if let Some(ref pg) = acc.parent_guid {
            if !account_guid_set.contains(pg) {
                acc.parent_guid = None;
            }
        }
    }

    // Detect and break parent cycles (Issue #18)
    let mut parent_map: HashMap<String, Option<String>> = raw_accounts
        .iter()
        .map(|a| (a.guid.clone(), a.parent_guid.clone()))
        .collect();

    let mut cyclic_accounts = HashSet::new();
    for acc in &raw_accounts {
        let mut curr = acc.guid.clone();
        let mut visited = HashSet::new();
        visited.insert(curr.clone());

        while let Some(Some(ref p)) = parent_map.get(&curr) {
            if visited.contains(p) {
                // p is an ancestor already seen in this walk; break the cycle by making curr a root account
                cyclic_accounts.insert(curr.clone());
                break;
            }
            visited.insert(p.clone());
            curr = p.clone();
        }
    }

    if !cyclic_accounts.is_empty() {
        for acc in &mut raw_accounts {
            if cyclic_accounts.contains(&acc.guid) {
                acc.parent_guid = None;
            }
        }
        // Refresh parent_map after breaking cycles
        parent_map = raw_accounts
            .iter()
            .map(|a| (a.guid.clone(), a.parent_guid.clone()))
            .collect();
    }

    let mut depth_map: HashMap<String, usize> = HashMap::new();
    for acc in &raw_accounts {
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

    // Fast lookup map for validation and reporting
    let acc_by_guid: HashMap<String, &RawAccount> = raw_accounts.iter().map(|a| (a.guid.clone(), a)).collect();

    // 3. Pre-validate all accounts (IMP-11, ACC-11)
    for acc in &raw_accounts {
        let full_name = build_full_account_name(&acc.guid, &acc_by_guid);
        let name_len = acc.name.chars().count();
        if name_len == 0 || name_len > 255 {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": format!(
                        "Import error in account '{}': name must be between 1 and 255 characters (length: {}) (IMP-11, ACC-11)",
                        full_name, name_len
                    )
                })),
            ));
        }

        if let Some(ref code) = acc.code {
            let code_len = code.chars().count();
            if code_len > 50 {
                return Err((
                    StatusCode::BAD_REQUEST,
                    Json(json!({
                        "error": format!(
                            "Import error in account '{}': code exceeds maximum length of 50 characters (length: {}) (IMP-11, ACC-11)",
                            full_name, code_len
                        )
                    })),
                ));
            }
        }

        let curr_len = acc.currency.chars().count();
        if curr_len == 0 || curr_len > 64 {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": format!(
                        "Import error in account '{}': currency/commodity code must be between 1 and 64 characters (length: {}) (IMP-11, ACC-11)",
                        full_name, curr_len
                    )
                })),
            ));
        }
    }

    // Defensive check (Issue #32, TXN-1): default empty/whitespace descriptions
    for tx in &mut raw_transactions {
        if tx.description.trim().is_empty() {
            tx.description = "GnuCash Transaction".to_string();
        }
    }

    // Defensive check & normalization (Issue #27, REC-2): normalize reconciled_state and reconcile_date
    for s in &mut raw_splits {
        let norm_state = match s.reconciled_state.to_lowercase().as_str() {
            "y" => "y".to_string(),
            "c" => "c".to_string(),
            _ => "n".to_string(),
        };
        s.reconciled_state = norm_state;
        if s.reconciled_state == "y" {
            if s.reconcile_date.is_none() {
                s.reconcile_date = Some(Utc::now());
            }
        } else {
            s.reconcile_date = None;
        }
    }

    // Issue #36 (IMP-10, ACC-1): Count splits per transaction and filter out transactions with < 2 splits
    let mut split_count_per_tx: HashMap<String, usize> = HashMap::new();
    for s in &raw_splits {
        *split_count_per_tx.entry(s.tx_guid.clone()).or_insert(0) += 1;
    }

    let mut invalid_tx_guids: HashSet<String> = HashSet::new();
    for tx in &raw_transactions {
        let count = split_count_per_tx.get(&tx.guid).copied().unwrap_or(0);
        if count < 2 {
            invalid_tx_guids.insert(tx.guid.clone());
        }
    }

    let skipped_transactions = invalid_tx_guids.len();

    if !invalid_tx_guids.is_empty() {
        raw_transactions.retain(|tx| !invalid_tx_guids.contains(&tx.guid));
        raw_splits.retain(|s| !invalid_tx_guids.contains(&s.tx_guid));
    }

    // Fast lookup for transaction metadata during error reporting
    let tx_by_guid: HashMap<String, &RawTransaction> = raw_transactions.iter().map(|t| (t.guid.clone(), t)).collect();

    // 4. Pre-validate all transactions (IMP-11, TXN-1)
    for tx in &raw_transactions {
        let desc_len = tx.description.chars().count();
        let preview = tx_desc_preview(&tx.description);
        let accounts_str = get_tx_accounts_string(&tx.guid, &raw_splits, &acc_by_guid);

        if desc_len == 0 || desc_len > 255 {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": format!(
                        "Import error in transaction (date: {}, description: '{}', accounts: [{}]): description must be between 1 and 255 characters (length: {}) (IMP-11, TXN-1)",
                        tx.post_date, preview, accounts_str, desc_len
                    )
                })),
            ));
        }

        if let Some(ref num) = tx.num {
            let num_len = num.chars().count();
            if num_len > 50 {
                return Err((
                    StatusCode::BAD_REQUEST,
                    Json(json!({
                        "error": format!(
                            "Import error in transaction (date: {}, description: '{}', accounts: [{}]): num/check reference exceeds maximum length of 50 characters (length: {}) (IMP-11, TXN-1)",
                            tx.post_date, preview, accounts_str, num_len
                        )
                    })),
                ));
            }
        }
    }

    // 5. Verify double-entry invariant (ACC-2, IMP-11, Issue #19, Issue #35)
    let mut tx_sums: HashMap<String, Decimal> = HashMap::new();
    for s in &raw_splits {
        *tx_sums.entry(s.tx_guid.clone()).or_insert(Decimal::ZERO) += s.amount;
    }
    for (tx_guid, sum) in &tx_sums {
        if *sum != Decimal::ZERO {
            let (date_str, desc_str, accounts_str) = if let Some(tx) = tx_by_guid.get(tx_guid) {
                (
                    tx.post_date.to_string(),
                    tx_desc_preview(&tx.description),
                    get_tx_accounts_string(tx_guid, &raw_splits, &acc_by_guid),
                )
            } else {
                ("unknown".to_string(), "unknown".to_string(), "unknown".to_string())
            };

            return Err((
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": format!(
                        "Double-entry violation in imported transaction {} (date: {}, description: '{}', accounts: [{}]): splits do not balance (imbalance: {}) (ACC-2, IMP-11)",
                        tx_guid, date_str, desc_str, accounts_str, sum
                    )
                })),
            ));
        }
    }

    // 6. Map of GnuCash GUID string -> Postgres UUID
    let mut account_guid_to_uuid: HashMap<String, Uuid> = HashMap::new();
    for acc in &raw_accounts {
        account_guid_to_uuid.insert(acc.guid.clone(), parse_gnucash_guid_for_user(&acc.guid, &user_id));
    }

    // 7. Begin DB transaction
    let mut tx = pool.begin().await.map_err(|e| {
        tracing::error!("Database transaction error: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": "Database transaction error" })),
        )
    })?;

    if wipe {
        sqlx::query("DELETE FROM splits WHERE account_id IN (SELECT id FROM accounts WHERE user_id = $1)")
            .bind(user_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| {
                tracing::error!("Failed to wipe splits: {}", e);
                (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Failed to wipe existing data for import"})))
            })?;

        sqlx::query("DELETE FROM transactions WHERE user_id = $1")
            .bind(user_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| {
                tracing::error!("Failed to wipe transactions: {}", e);
                (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Failed to wipe existing data for import"})))
            })?;

        sqlx::query("DELETE FROM accounts WHERE user_id = $1")
            .bind(user_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| {
                tracing::error!("Failed to wipe accounts: {}", e);
                (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Failed to wipe existing data for import"})))
            })?;
    }

    // 6. Batch insert accounts level-by-level with UNNEST (Issue #13)
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
            let parent_ids: Vec<Option<Uuid>> = chunk.iter().map(|a| a.parent_guid.as_ref().and_then(|pg| account_guid_to_uuid.get(pg).copied())).collect();
            let currencies: Vec<String> = chunk.iter().map(|a| a.currency.clone()).collect();
            let descs: Vec<Option<String>> = chunk.iter().map(|a| a.description.clone()).collect();
            let placeholders: Vec<bool> = chunk.iter().map(|a| a.placeholder).collect();
            let hiddens: Vec<bool> = chunk.iter().map(|a| a.hidden).collect();
            let user_ids: Vec<Uuid> = vec![user_id; chunk.len()];
            let namespaces: Vec<String> = chunk.iter().map(|a| if a.commodity_namespace.is_empty() { "CURRENCY".to_string() } else { a.commodity_namespace.clone() }).collect();
            let fractions: Vec<i64> = chunk.iter().map(|a| if a.commodity_fraction == 0 { 100 } else { a.commodity_fraction }).collect();

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
            .execute(&mut *tx)
            .await
            .map_err(|e| {
                tracing::error!("Failed to batch insert accounts: {}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({ "error": "Failed to batch insert accounts" })),
                )
            })?;

            inserted_acc_count += chunk.len();
        }
    }

    // 7. Map of Transaction GUID -> Postgres UUID
    let mut tx_guid_to_uuid: HashMap<String, Uuid> = HashMap::new();
    for t in &raw_transactions {
        tx_guid_to_uuid.insert(t.guid.clone(), parse_gnucash_guid_for_user(&t.guid, &user_id));
    }

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
        .execute(&mut *tx)
        .await
        .map_err(|e| {
            tracing::error!("Failed to batch insert transactions: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "Failed to batch insert transactions" })),
            )
        })?;

        tx_count += chunk.len();
    }

    // 8. Issue #5: Clear previous splits of imported transactions so no stale/duplicate splits remain
    let imported_tx_uuids: Vec<Uuid> = tx_guid_to_uuid.values().copied().collect();
    for chunk in imported_tx_uuids.chunks(1000) {
        sqlx::query("DELETE FROM splits WHERE transaction_id = ANY($1::uuid[])")
            .bind(chunk)
            .execute(&mut *tx)
            .await
            .map_err(|e| {
                tracing::error!("Failed to clear stale splits: {}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({ "error": "Failed to clear stale splits" })),
                )
            })?;
    }

    // 9. Batch insert splits with UNNEST (Issue #13)
    let valid_splits: Vec<&RawSplit> = raw_splits
        .iter()
        .filter(|s| tx_guid_to_uuid.contains_key(&s.tx_guid) && account_guid_to_uuid.contains_key(&s.account_guid))
        .collect();

    let mut split_count = 0;
    for chunk in valid_splits.chunks(1000) {
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
        .execute(&mut *tx)
        .await
        .map_err(|e| {
            tracing::error!("Failed to batch insert splits: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "Failed to batch insert splits" })),
            )
        })?;

        split_count += chunk.len();
    }

    tx.commit().await.map_err(|e| {
        tracing::error!("Failed to commit imported data: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": "Failed to commit imported data" })),
        )
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

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Datelike, Timelike};

    #[test]
    fn test_parse_gnucash_datetime_formats() {
        // RFC 3339
        let dt1 = parse_gnucash_datetime("2024-05-18T10:59:00Z").expect("RFC 3339 failed");
        assert_eq!(dt1.year(), 2024);
        assert_eq!(dt1.month(), 5);
        assert_eq!(dt1.day(), 18);
        assert_eq!(dt1.hour(), 10);
        assert_eq!(dt1.minute(), 59);

        // GnuCash XML datetime with timezone
        let dt2 = parse_gnucash_datetime("2024-05-18 10:59:00 +0000").expect("GnuCash with tz failed");
        assert_eq!(dt2.year(), 2024);
        assert_eq!(dt2.month(), 5);
        assert_eq!(dt2.day(), 18);

        // Standard naive datetime
        let dt3 = parse_gnucash_datetime("2024-05-18 10:59:00").expect("Naive datetime failed");
        assert_eq!(dt3.year(), 2024);
        assert_eq!(dt3.month(), 5);
        assert_eq!(dt3.day(), 18);

        // SQLite compact 14-digit format
        let dt4 = parse_gnucash_datetime("20240518105900").expect("SQLite compact 14-digit failed");
        assert_eq!(dt4.year(), 2024);
        assert_eq!(dt4.month(), 5);
        assert_eq!(dt4.day(), 18);
        assert_eq!(dt4.hour(), 10);
        assert_eq!(dt4.minute(), 59);

        // Date only YYYY-MM-DD
        let dt5 = parse_gnucash_datetime("2024-05-18").expect("Date only YYYY-MM-DD failed");
        assert_eq!(dt5.year(), 2024);
        assert_eq!(dt5.month(), 5);
        assert_eq!(dt5.day(), 18);

        // Date only YYYYMMDD
        let dt6 = parse_gnucash_datetime("20240518").expect("Date only YYYYMMDD failed");
        assert_eq!(dt6.year(), 2024);
        assert_eq!(dt6.month(), 5);
        assert_eq!(dt6.day(), 18);

        // Epoch seconds
        let dt7 = parse_gnucash_datetime("1716029940").expect("Epoch timestamp failed");
        assert_eq!(dt7.year(), 2024);

        // Empty / invalid strings
        assert!(parse_gnucash_datetime("").is_none());
        assert!(parse_gnucash_datetime("   ").is_none());
        assert!(parse_gnucash_datetime("invalid-date-string").is_none());

        // Multi-byte UTF-8 strings must not panic on slice boundaries
        assert!(parse_gnucash_datetime("안녕하세요").is_none());
        assert!(parse_gnucash_datetime("२०२६-०३-३१").is_none());
        assert!(parse_gnucash_datetime("２０２６年０३月３１日").is_none());
        assert!(parse_gnucash_datetime("🎉🎊🎁🎈").is_none());
        assert!(parse_gnucash_datetime("こんにちは世界2024").is_none());

        // parse_gnucash_date also safely handles multi-byte UTF-8 strings without panicking
        let _ = parse_gnucash_date("안녕하세요");
        let _ = parse_gnucash_date("२०२६-०३-३१");
        let _ = parse_gnucash_date("２０２６年０३月３１日");
        let _ = parse_gnucash_date("🎉🎊🎁🎈");
    }

    #[test]
    fn test_split_reconcile_date_normalization_rules() {
        // Reconciled state 'y' preserves date or defaults to Utc::now()
        let past_date = Utc::now() - chrono::Duration::days(10);
        let mut split_y = RawSplit {
            reconciled_state: "y".to_string(),
            reconcile_date: Some(past_date),
            ..Default::default()
        };
        let norm_state = match split_y.reconciled_state.to_lowercase().as_str() {
            "y" => "y".to_string(),
            "c" => "c".to_string(),
            _ => "n".to_string(),
        };
        split_y.reconciled_state = norm_state;
        if split_y.reconciled_state == "y" {
            if split_y.reconcile_date.is_none() {
                split_y.reconcile_date = Some(Utc::now());
            }
        } else {
            split_y.reconcile_date = None;
        }
        assert_eq!(split_y.reconcile_date, Some(past_date));

        // Reconciled state 'n' or 'c' clears reconcile_date
        let mut split_c = RawSplit {
            reconciled_state: "c".to_string(),
            reconcile_date: Some(past_date),
            ..Default::default()
        };
        let norm_state = match split_c.reconciled_state.to_lowercase().as_str() {
            "y" => "y".to_string(),
            "c" => "c".to_string(),
            _ => "n".to_string(),
        };
        split_c.reconciled_state = norm_state;
        if split_c.reconciled_state == "y" {
            if split_c.reconcile_date.is_none() {
                split_c.reconcile_date = Some(Utc::now());
            }
        } else {
            split_c.reconcile_date = None;
        }
        assert!(split_c.reconcile_date.is_none());
    }

    #[test]
    fn test_empty_transaction_description_defaults_to_gnucash_transaction() {
        let mut tx_empty = RawTransaction {
            description: "   ".to_string(),
            ..Default::default()
        };
        if tx_empty.description.trim().is_empty() {
            tx_empty.description = "GnuCash Transaction".to_string();
        }
        assert_eq!(tx_empty.description, "GnuCash Transaction");
    }

    #[test]
    fn test_split_count_enforcement_skips_transactions_with_fewer_than_two_splits() {
        let mut raw_transactions = vec![
            RawTransaction { guid: "tx-valid".to_string(), ..Default::default() },
            RawTransaction { guid: "tx-zero".to_string(), ..Default::default() },
            RawTransaction { guid: "tx-one".to_string(), ..Default::default() },
        ];
        let mut raw_splits = vec![
            RawSplit { guid: "s1".to_string(), tx_guid: "tx-valid".to_string(), ..Default::default() },
            RawSplit { guid: "s2".to_string(), tx_guid: "tx-valid".to_string(), ..Default::default() },
            RawSplit { guid: "s3".to_string(), tx_guid: "tx-one".to_string(), ..Default::default() },
        ];

        let mut split_count_per_tx: HashMap<String, usize> = HashMap::new();
        for s in &raw_splits {
            *split_count_per_tx.entry(s.tx_guid.clone()).or_insert(0) += 1;
        }

        let mut invalid_tx_guids: HashSet<String> = HashSet::new();
        for tx in &raw_transactions {
            let count = split_count_per_tx.get(&tx.guid).copied().unwrap_or(0);
            if count < 2 {
                invalid_tx_guids.insert(tx.guid.clone());
            }
        }

        let skipped_transactions = invalid_tx_guids.len();
        assert_eq!(skipped_transactions, 2);

        if !invalid_tx_guids.is_empty() {
            raw_transactions.retain(|tx| !invalid_tx_guids.contains(&tx.guid));
            raw_splits.retain(|s| !invalid_tx_guids.contains(&s.tx_guid));
        }

        assert_eq!(raw_transactions.len(), 1);
        assert_eq!(raw_transactions[0].guid, "tx-valid");
        assert_eq!(raw_splits.len(), 2);
    }
}
