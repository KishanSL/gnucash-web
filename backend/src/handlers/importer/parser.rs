use chrono::{DateTime, NaiveDate, NaiveDateTime, Utc};
use flate2::read::GzDecoder;
use quick_xml::events::Event;
use quick_xml::reader::Reader;
use rusqlite::Connection;
use rust_decimal::Decimal;
use std::collections::{HashMap, HashSet};
use std::io::{Read, Write};
use tempfile::NamedTempFile;

use super::models::{CommInfo, ParsedGnuCashData, ParsedGnuCashFile, RawAccount, RawSplit, RawTransaction};

pub fn parse_gnucash_date(date_str: &str) -> NaiveDate {
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

pub fn parse_gnucash_datetime(s: &str) -> Option<DateTime<Utc>> {
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

pub fn normalize_account_type(raw_type: &str) -> String {
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

pub fn parse_fraction(val: &str) -> Option<Decimal> {
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

pub fn is_gzip(bytes: &[u8]) -> bool {
    bytes.len() >= 2 && bytes[0] == 0x1f && bytes[1] == 0x8b
}

pub fn is_sqlite(bytes: &[u8]) -> bool {
    bytes.starts_with(b"SQLite format 3")
}

pub fn is_xml(bytes: &[u8]) -> bool {
    let s = String::from_utf8_lossy(&bytes[..bytes.len().min(120)]);
    s.trim_start().starts_with("<?xml") || s.contains("<gnc-v2") || s.contains("<gnc:")
}

pub fn resolve_entity_or_char_ref(s: &str) -> Option<String> {
    match s {
        "amp" => Some("&".to_string()),
        "lt" => Some("<".to_string()),
        "gt" => Some(">".to_string()),
        "apos" => Some("'".to_string()),
        "quot" => Some("\"".to_string()),
        _ => {
            if let Some(hex_str) = s.strip_prefix("#x").or_else(|| s.strip_prefix("#X")) {
                let code = u32::from_str_radix(hex_str, 16).ok()?;
                char::from_u32(code).map(|c| c.to_string())
            } else if let Some(dec_str) = s.strip_prefix('#') {
                let code = dec_str.parse::<u32>().ok()?;
                char::from_u32(code).map(|c| c.to_string())
            } else {
                None
            }
        }
    }
}

pub fn parse_gnucash_xml(
    xml_bytes: &[u8],
    default_curr: &str,
) -> Result<ParsedGnuCashData, String> {
    let mut reader = Reader::from_reader(xml_bytes);
    // Note: Do not enable trim_text(true) here as it strips whitespace around entity boundaries (Issue #23)
    reader.config_mut().trim_text(false);

    let mut accounts = Vec::new();
    let mut transactions = Vec::new();
    let mut splits = Vec::new();

    let mut tag_stack: Vec<String> = Vec::new();

    let mut in_template_transactions = false;
    let mut curr_account: Option<RawAccount> = None;
    let mut curr_tx: Option<RawTransaction> = None;
    let mut curr_split: Option<RawSplit> = None;
    let mut curr_slot_key: Option<String> = None;

    let mut curr_text = String::new();
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                let name = e.name().as_ref().to_string();
                curr_text.clear();
                match name.as_str() {
                    "gnc:template-transactions" => {
                        in_template_transactions = true;
                    }
                    "gnc:account" if !in_template_transactions => {
                        curr_account = Some(RawAccount {
                            currency: default_curr.to_string(),
                            ..Default::default()
                        });
                    }
                    "gnc:transaction" if !in_template_transactions => {
                        curr_tx = Some(RawTransaction {
                            post_date: Utc::now().date_naive(),
                            ..Default::default()
                        });
                    }
                    "trn:split" if !in_template_transactions => {
                        let tx_guid = curr_tx.as_ref().map(|tx| tx.guid.clone()).unwrap_or_default();
                        curr_split = Some(RawSplit {
                            tx_guid,
                            reconciled_state: "n".to_string(),
                            ..Default::default()
                        });
                    }
                    _ => {}
                }
                tag_stack.push(name);
            }
            Ok(Event::Text(e)) => {
                curr_text.push_str(e.as_ref());
            }
            Ok(Event::GeneralRef(e)) => {
                if let Some(resolved) = resolve_entity_or_char_ref(e.as_ref()) {
                    curr_text.push_str(&resolved);
                } else {
                    curr_text.push('&');
                    curr_text.push_str(e.as_ref());
                    curr_text.push(';');
                }
            }
            Ok(Event::CData(e)) => {
                curr_text.push_str(e.as_ref());
            }
            Ok(Event::End(e)) => {
                let name = e.name().as_ref().to_string();
                let current_tag = name.as_str();
                let parent_tag = if tag_stack.len() >= 2 {
                    tag_stack[tag_stack.len() - 2].as_str()
                } else {
                    ""
                };
                let text = curr_text.trim().to_string();

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

                // Split fields
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

                // Handle closing containers
                match current_tag {
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

                curr_text.clear();
                tag_stack.pop();
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

pub fn parse_gnucash_sqlite(
    file_bytes: &[u8],
    default_curr: &str,
) -> Result<ParsedGnuCashData, String> {
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
            parent_guid: parent_guid.filter(|p| !p.trim().is_empty()),
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
) -> Result<ParsedGnuCashFile, String> {
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
