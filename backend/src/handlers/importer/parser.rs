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

/// Normalizes a GnuCash reconcile state to `n`/`c`/`y`. Only reconciled (`y`) splits keep a
/// reconcile date, defaulting to now when the file has none (REC-1, DATA-3).
fn normalize_reconcile(raw_state: &str, date: Option<DateTime<Utc>>) -> (String, Option<DateTime<Utc>>) {
    match raw_state.to_lowercase().as_str() {
        "y" => ("y".to_string(), date.or_else(|| Some(Utc::now()))),
        "c" => ("c".to_string(), None),
        _ => ("n".to_string(), None),
    }
}

/// Removes scheduled-transaction template accounts (and every transaction touching them)
/// from a parsed book (Issue #7).
fn remove_template_entries(
    accounts: &mut Vec<RawAccount>,
    transactions: &mut Vec<RawTransaction>,
    splits: &mut Vec<RawSplit>,
    root_account_guid: Option<&str>,
    root_template_guid: Option<&str>,
) {
    let mut template_guids = HashSet::new();
    if let Some(rtg) = root_template_guid {
        template_guids.insert(rtg.to_string());
    }
    if let Some(rag) = root_account_guid {
        for a in accounts.iter() {
            if a.account_type.eq_ignore_ascii_case("ROOT") && a.guid != rag {
                template_guids.insert(a.guid.clone());
            }
        }
    }

    let mut changed = true;
    while changed {
        changed = false;
        for a in accounts.iter() {
            if let Some(ref pg) = a.parent_guid {
                if template_guids.contains(pg) && !template_guids.contains(&a.guid) {
                    template_guids.insert(a.guid.clone());
                    changed = true;
                }
            }
        }
    }

    if template_guids.is_empty() {
        return;
    }

    let template_tx_guids: HashSet<String> = splits
        .iter()
        .filter(|s| template_guids.contains(&s.account_guid))
        .map(|s| s.tx_guid.clone())
        .collect();

    accounts.retain(|a| !template_guids.contains(&a.guid));
    splits.retain(|s| !template_tx_guids.contains(&s.tx_guid));
    transactions.retain(|t| !template_tx_guids.contains(&t.guid));
}

// ---------------------------------------------------------------------------
// XML
// ---------------------------------------------------------------------------

fn apply_account_field(
    acc: &mut RawAccount,
    slot_key: &mut Option<String>,
    tag: &str,
    parent_tag: &str,
    text: &str,
) {
    match tag {
        "act:id" => acc.guid = text.to_string(),
        "act:name" => acc.name = text.to_string(),
        "act:type" => acc.account_type = text.to_string(),
        "act:description" => acc.description = Some(text.to_string()),
        "act:code" => acc.code = Some(text.to_string()),
        "act:parent" => acc.parent_guid = Some(text.to_string()),
        "cmdty:id" => {
            if parent_tag == "act:commodity" && !text.eq_ignore_ascii_case("template") {
                acc.currency = text.to_string();
            }
        }
        "cmdty:space" => {
            if parent_tag == "act:commodity" {
                acc.commodity_namespace = text.to_string();
            }
        }
        "act:commodity-scu" => {
            if let Ok(scu) = text.parse::<i64>() {
                acc.commodity_fraction = scu;
            }
        }
        "slot:key" => *slot_key = Some(text.to_string()),
        "slot:value" => match slot_key.as_deref() {
            Some("placeholder") => acc.placeholder = text.eq_ignore_ascii_case("true"),
            Some("hidden") => acc.hidden = text.eq_ignore_ascii_case("true"),
            Some("notes") if acc.description.is_none() => acc.description = Some(text.to_string()),
            _ => {}
        },
        _ => {}
    }
}

fn apply_split_field(sp: &mut RawSplit, tag: &str, parent_tag: &str, text: &str) {
    match tag {
        "split:id" => sp.guid = text.to_string(),
        "split:account" => sp.account_guid = text.to_string(),
        "split:memo" => sp.memo = Some(text.to_string()),
        "split:reconciled-state" => sp.reconciled_state = text.to_lowercase(),
        "split:value" => {
            if let Some(dec) = parse_fraction(text) {
                sp.amount = dec;
            }
        }
        "split:quantity" => {
            if let Some(dec) = parse_fraction(text) {
                sp.quantity = dec;
            }
        }
        "ts:date" | "gdate" => {
            if parent_tag == "split:reconcile-date" || parent_tag.ends_with("reconcile-date") {
                sp.reconcile_date = parse_gnucash_datetime(text);
            }
        }
        _ => {
            if tag.ends_with("reconcile-date") {
                if let Some(dt) = parse_gnucash_datetime(text) {
                    sp.reconcile_date = Some(dt);
                }
            }
        }
    }
}

fn apply_transaction_field(
    tx: &mut RawTransaction,
    slot_key: &mut Option<String>,
    tag: &str,
    parent_tag: &str,
    text: &str,
) {
    match tag {
        "trn:id" => tx.guid = text.to_string(),
        "trn:description" => tx.description = text.to_string(),
        "trn:num" => tx.num = Some(text.to_string()),
        "ts:date" => {
            if parent_tag == "trn:date-posted" {
                tx.post_date = parse_gnucash_date(text);
            }
        }
        "gdate" => {
            if let Ok(d) = NaiveDate::parse_from_str(text, "%Y-%m-%d") {
                tx.post_date = d;
            }
        }
        "slot:key" => *slot_key = Some(text.to_string()),
        "slot:value" if slot_key.as_deref() == Some("notes") => tx.notes = Some(text.to_string()),
        _ => {}
    }
}

/// Element-level state of the streaming GnuCash XML parser.
#[derive(Default)]
struct XmlParseState {
    accounts: Vec<RawAccount>,
    transactions: Vec<RawTransaction>,
    splits: Vec<RawSplit>,
    tag_stack: Vec<String>,
    in_template_transactions: bool,
    curr_account: Option<RawAccount>,
    curr_tx: Option<RawTransaction>,
    curr_split: Option<RawSplit>,
    curr_slot_key: Option<String>,
}

impl XmlParseState {
    fn start_element(&mut self, name: String, default_curr: &str) {
        match name.as_str() {
            "gnc:template-transactions" => self.in_template_transactions = true,
            "gnc:account" if !self.in_template_transactions => {
                self.curr_account = Some(RawAccount {
                    currency: default_curr.to_string(),
                    ..Default::default()
                });
            }
            "gnc:transaction" if !self.in_template_transactions => {
                self.curr_tx = Some(RawTransaction {
                    post_date: Utc::now().date_naive(),
                    ..Default::default()
                });
            }
            "trn:split" if !self.in_template_transactions => {
                let tx_guid = self.curr_tx.as_ref().map(|tx| tx.guid.clone()).unwrap_or_default();
                self.curr_split = Some(RawSplit {
                    tx_guid,
                    reconciled_state: "n".to_string(),
                    ..Default::default()
                });
            }
            _ => {}
        }
        self.tag_stack.push(name);
    }

    fn end_element(&mut self, tag: &str, text: &str) {
        let parent_tag = if self.tag_stack.len() >= 2 {
            self.tag_stack[self.tag_stack.len() - 2].clone()
        } else {
            String::new()
        };

        if let Some(acc) = self.curr_account.as_mut() {
            apply_account_field(acc, &mut self.curr_slot_key, tag, &parent_tag, text);
        }
        if let Some(sp) = self.curr_split.as_mut() {
            apply_split_field(sp, tag, &parent_tag, text);
        } else if let Some(tx) = self.curr_tx.as_mut() {
            apply_transaction_field(tx, &mut self.curr_slot_key, tag, &parent_tag, text);
        }

        self.close_container(tag);
        self.tag_stack.pop();
    }

    fn close_container(&mut self, tag: &str) {
        match tag {
            "gnc:template-transactions" => self.in_template_transactions = false,
            "gnc:account" => {
                if let Some(mut acc) = self.curr_account.take() {
                    if !acc.guid.is_empty() {
                        acc.account_type = normalize_account_type(&acc.account_type);
                        self.accounts.push(acc);
                    }
                }
            }
            "gnc:transaction" => {
                if let Some(mut tx) = self.curr_tx.take() {
                    if !tx.guid.is_empty() {
                        if tx.description.trim().is_empty() {
                            tx.description = "GnuCash Transaction".to_string();
                        }
                        self.transactions.push(tx);
                    }
                }
            }
            "trn:split" => {
                if let Some(mut sp) = self.curr_split.take() {
                    if !sp.guid.is_empty() && !sp.account_guid.is_empty() {
                        if sp.quantity == Decimal::ZERO && sp.amount != Decimal::ZERO {
                            sp.quantity = sp.amount;
                        }
                        if sp.tx_guid.is_empty() {
                            if let Some(ref tx) = self.curr_tx {
                                sp.tx_guid = tx.guid.clone();
                            }
                        }
                        let (state, date) = normalize_reconcile(&sp.reconciled_state, sp.reconcile_date);
                        sp.reconciled_state = state;
                        sp.reconcile_date = date;
                        self.splits.push(sp);
                    }
                }
            }
            "slot" => self.curr_slot_key = None,
            _ => {}
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

    let mut state = XmlParseState::default();
    let mut curr_text = String::new();
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                let name = e.name().as_ref().to_string();
                curr_text.clear();
                state.start_element(name, default_curr);
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
                let text = curr_text.trim().to_string();
                state.end_element(&name, &text);
                curr_text.clear();
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

    Ok((state.accounts, state.transactions, state.splits))
}

// ---------------------------------------------------------------------------
// SQLite
// ---------------------------------------------------------------------------

/// Writes the upload to a temporary file and opens it read-only. The returned temp file
/// must outlive the connection.
fn open_sqlite(file_bytes: &[u8]) -> Result<(NamedTempFile, Connection), String> {
    let mut temp_file = NamedTempFile::new().map_err(|e| {
        tracing::error!("Temp file error: {}", e);
        "Failed to create temporary file for import".to_string()
    })?;
    temp_file.write_all(file_bytes).map_err(|e| {
        tracing::error!("Failed to write temp file: {}", e);
        "Failed to write uploaded file to disk".to_string()
    })?;

    let conn = Connection::open_with_flags(temp_file.path(), rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|e| {
            tracing::error!("Invalid SQLite file: {}", e);
            "Invalid SQLite file".to_string()
        })?;
    conn.pragma_update(None, "trusted_schema", "OFF").ok();
    Ok((temp_file, conn))
}

fn read_sqlite_table_names(conn: &Connection) -> Result<Vec<String>, String> {
    let mut check_stmt = conn.prepare("SELECT name FROM sqlite_master WHERE type='table'")
        .map_err(|e| {
            tracing::error!("SQLite query failed: {}", e);
            "Failed to query SQLite database tables".to_string()
        })?;
    let table_names = check_stmt.query_map([], |row| row.get(0))
        .map_err(|e| {
            tracing::error!("Failed to read tables: {}", e);
            "Failed to read SQLite database tables".to_string()
        })?
        .filter_map(|r| r.ok())
        .collect();
    Ok(table_names)
}

/// Commodities, if present (Issue #9).
fn read_sqlite_commodities(conn: &Connection) -> HashMap<String, CommInfo> {
    let mut commodity_map = HashMap::new();
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
    commodity_map
}

/// Root account and template root guids, if the books table is present.
fn read_sqlite_book_roots(conn: &Connection) -> (Option<String>, Option<String>) {
    if let Ok(mut stmt) = conn.prepare("SELECT root_account_guid, root_template_guid FROM books") {
        if let Ok(mut rows) = stmt.query([]) {
            if let Ok(Some(row)) = rows.next() {
                return (row.get(0).ok(), row.get(1).ok());
            }
        }
    }
    (None, None)
}

fn read_sqlite_accounts(
    conn: &Connection,
    commodity_map: &HashMap<String, CommInfo>,
    default_curr: &str,
) -> Result<Vec<RawAccount>, String> {
    let mut acc_stmt = conn.prepare(
        "SELECT guid, name, account_type, commodity_guid, parent_guid, code, description, hidden, placeholder FROM accounts"
    ).map_err(|e| {
        tracing::error!("Failed to prepare accounts query: {}", e);
        "Failed to parse accounts from SQLite file".to_string()
    })?;

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

    let mut raw_accounts = Vec::new();
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
    Ok(raw_accounts)
}

/// Transactions, with notes from the slots table when present.
fn read_sqlite_transactions(conn: &Connection, has_slots: bool) -> Result<Vec<RawTransaction>, String> {
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

    let mut raw_transactions = Vec::new();
    for r in tx_rows.flatten() {
        let (guid, post_date_str, num, desc, notes) = r;
        raw_transactions.push(RawTransaction {
            guid,
            post_date: parse_gnucash_date(&post_date_str),
            num: num.filter(|s| !s.trim().is_empty()),
            description: desc
                .filter(|s| !s.trim().is_empty())
                .unwrap_or_else(|| "GnuCash Transaction".to_string()),
            notes: notes.filter(|s| !s.trim().is_empty()),
        });
    }
    Ok(raw_transactions)
}

/// Splits with both value and quantity (Issue #9) and reconcile date (Issue #27).
fn read_sqlite_splits(conn: &Connection) -> Result<Vec<RawSplit>, String> {
    let mut split_stmt = conn.prepare(
        "SELECT guid, tx_guid, account_guid, memo, reconcile_state, value_num, value_denom, quantity_num, quantity_denom, reconcile_date FROM splits"
    ).map_err(|e| {
        tracing::error!("Failed to prepare splits query: {}", e);
        "Failed to parse splits from SQLite file".to_string()
    })?;

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

    let mut raw_splits = Vec::new();
    for r in split_rows.flatten() {
        let (guid, tx_guid, account_guid, memo, rec_state, val_num, val_denom, qty_num, qty_denom, rec_date_str) = r;
        let denom = if val_denom == 0 { 100 } else { val_denom };
        let amount = Decimal::from(val_num) / Decimal::from(denom);

        let quantity = if qty_denom != 0 {
            Decimal::from(qty_num) / Decimal::from(qty_denom)
        } else {
            amount
        };

        // Only parse the date for reconciled splits, as before
        let is_reconciled = rec_state.as_deref().unwrap_or("n").eq_ignore_ascii_case("y");
        let parsed_date = if is_reconciled {
            rec_date_str.as_deref().and_then(parse_gnucash_datetime)
        } else {
            None
        };
        let (reconciled_state, reconcile_date) =
            normalize_reconcile(rec_state.as_deref().unwrap_or("n"), parsed_date);

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
    Ok(raw_splits)
}

pub fn parse_gnucash_sqlite(
    file_bytes: &[u8],
    default_curr: &str,
) -> Result<ParsedGnuCashData, String> {
    let (_temp_file, conn) = open_sqlite(file_bytes)?;

    let table_names = read_sqlite_table_names(&conn)?;
    let has_table = |name: &str| table_names.iter().any(|t| t == name);
    if !has_table("accounts") || !has_table("transactions") || !has_table("splits") {
        return Err("Uploaded database is not a valid GnuCash SQLite file (missing accounts, transactions, or splits tables).".to_string());
    }

    let commodity_map = read_sqlite_commodities(&conn);
    let (root_account_guid, root_template_guid) = read_sqlite_book_roots(&conn);

    let mut raw_accounts = read_sqlite_accounts(&conn, &commodity_map, default_curr)?;
    let mut raw_transactions = read_sqlite_transactions(&conn, has_table("slots"))?;
    let mut raw_splits = read_sqlite_splits(&conn)?;

    remove_template_entries(
        &mut raw_accounts,
        &mut raw_transactions,
        &mut raw_splits,
        root_account_guid.as_deref(),
        root_template_guid.as_deref(),
    );

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
