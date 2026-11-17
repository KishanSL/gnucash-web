//! Writes a GnuCash 5.17-compatible SQLite book from the exported rows (EXP-*).

use rusqlite::{params, Connection, Transaction};
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use std::collections::HashMap;
use tempfile::NamedTempFile;
use uuid::Uuid;

use super::currency::{resolve_book_currency, resolve_currency_metadata, resolve_transaction_currencies, CurrencyMeta};
use super::schema::GNUCASH_SCHEMA_SQL;
use super::{AccountExportRow, SplitExportRow, TransactionExportRow};
use crate::handlers::{internal_error, ApiError};

/// GnuCash GUID text form: 32 lowercase hex characters without dashes.
fn guid(id: Uuid) -> String {
    id.simple().to_string()
}

/// Builds the whole SQLite book in a temp file inside a single SQLite transaction and returns its bytes.
pub fn write_gnucash_sqlite(
    accounts: &[AccountExportRow],
    transactions: &[TransactionExportRow],
    splits: &[SplitExportRow],
) -> Result<Vec<u8>, ApiError> {
    let temp_file = NamedTempFile::new().map_err(|e| internal_error("Temp file error", e))?;
    let temp_path = temp_file.path().to_path_buf();

    let mut conn = Connection::open(&temp_path).map_err(|e| internal_error("SQLite connection failed", e))?;
    conn.pragma_update(None, "journal_mode", "OFF").ok();
    conn.pragma_update(None, "synchronous", "OFF").ok();

    let sqlite_tx = conn.transaction().map_err(|e| internal_error("Failed to start SQLite transaction", e))?;
    sqlite_tx
        .execute_batch(GNUCASH_SCHEMA_SQL)
        .map_err(|e| internal_error("Failed to initialize GnuCash tables", e))?;

    let root_account_guid = insert_book(&sqlite_tx)?;

    let (currency_meta, currencies) = resolve_currency_metadata(accounts);
    let commodity_map = insert_commodities(&sqlite_tx, &currencies, &currency_meta)?;
    let book_currency = resolve_book_currency(accounts, &currency_meta);
    let default_commodity_guid = default_commodity_guid(&commodity_map, &currencies, &currency_meta, &book_currency);

    insert_accounts(&sqlite_tx, accounts, &root_account_guid, &commodity_map, &default_commodity_guid, &currency_meta)?;

    // Only choose a commodity whose namespace is "CURRENCY" as transaction currency (INV-5, SCOPE-1, Issue #26)
    let account_currency_map: HashMap<Uuid, String> = accounts.iter().map(|a| (a.id, a.currency.clone())).collect();
    let tx_currency_map = resolve_transaction_currencies(splits, &account_currency_map, &currency_meta);
    insert_transactions(&sqlite_tx, transactions, &tx_currency_map, &book_currency, &commodity_map, &default_commodity_guid)?;

    insert_splits(&sqlite_tx, splits, accounts, &currency_meta)?;

    sqlite_tx.commit().map_err(|e| internal_error("Failed to commit SQLite export", e))?;
    drop(conn);

    std::fs::read(&temp_path).map_err(|e| internal_error("Failed to read generated SQLite file", e))
}

/// Inserts the book with its ISO-8601 dates feature slot; returns the root account GUID.
fn insert_book(tx: &Transaction) -> Result<String, ApiError> {
    let book_guid = guid(Uuid::new_v4());
    let root_account_guid = guid(Uuid::new_v4());
    let root_template_guid = guid(Uuid::new_v4());

    tx.execute(
        "INSERT INTO books (guid, root_account_guid, root_template_guid) VALUES (?1, ?2, ?3)",
        params![book_guid, root_account_guid, root_template_guid],
    ).map_err(|e| internal_error("Failed to insert book", e))?;

    tx.execute(
        "INSERT INTO slots (obj_guid, name, slot_type, string_val)
         VALUES (?1, 'features/ISO-8601 formatted date strings in SQLite3 databases.', 4, 'Use ISO formatted date-time strings in SQLite3 databases (requires at least GnuCash 2.6.20)')",
        params![book_guid],
    ).map_err(|e| internal_error("Failed to insert book feature slot", e))?;

    Ok(root_account_guid)
}

/// Inserts one commodity per currency/fund mnemonic; returns mnemonic -> commodity GUID.
fn insert_commodities(
    tx: &Transaction,
    currencies: &[String],
    currency_meta: &CurrencyMeta,
) -> Result<HashMap<String, String>, ApiError> {
    let mut commodity_map = HashMap::new();
    for curr in currencies {
        let comm_guid = guid(Uuid::new_v5(&Uuid::NAMESPACE_DNS, curr.as_bytes()));
        commodity_map.insert(curr.clone(), comm_guid.clone());

        let (namespace, fraction) = currency_meta.get(curr).cloned().unwrap_or_else(|| {
            let is_iso = curr.len() == 3 && curr.chars().all(|c| c.is_ascii_alphabetic());
            (if is_iso { "CURRENCY".to_string() } else { "FUND".to_string() }, 100)
        });
        let quote_source = if namespace == "CURRENCY" { "currency" } else { "financequote" };

        tx.execute(
            "INSERT INTO commodities (guid, namespace, mnemonic, fullname, cusip, fraction, quote_flag, quote_source)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![comm_guid, namespace, curr, curr, "", fraction, 1, quote_source],
        ).map_err(|e| internal_error("Failed to insert commodity", e))?;
    }
    Ok(commodity_map)
}

/// Commodity GUID used when an account or transaction currency has none: the book currency,
/// else the first CURRENCY commodity, else INR, else a fresh GUID.
fn default_commodity_guid(
    commodity_map: &HashMap<String, String>,
    currencies: &[String],
    currency_meta: &CurrencyMeta,
    book_currency: &str,
) -> String {
    commodity_map
        .get(book_currency)
        .cloned()
        .or_else(|| {
            currencies.iter()
                .find(|c| currency_meta.get(*c).map(|(ns, _)| ns == "CURRENCY").unwrap_or(false))
                .and_then(|c| commodity_map.get(c))
                .cloned()
        })
        .unwrap_or_else(|| {
            commodity_map.get("INR").cloned().unwrap_or_else(|| guid(Uuid::new_v4()))
        })
}

/// Commodity fraction (SCU) for an account: its stored fraction, else its commodity's.
fn account_fraction(acc: &AccountExportRow, currency_meta: &CurrencyMeta) -> i64 {
    acc.commodity_fraction.unwrap_or_else(|| {
        currency_meta.get(&acc.currency).map(|(_, f)| *f).unwrap_or(100)
    })
}

/// Inserts the root account and every user account under it.
fn insert_accounts(
    tx: &Transaction,
    accounts: &[AccountExportRow],
    root_account_guid: &str,
    commodity_map: &HashMap<String, String>,
    default_commodity_guid: &str,
    currency_meta: &CurrencyMeta,
) -> Result<(), ApiError> {
    tx.execute(
        "INSERT INTO accounts (guid, name, account_type, commodity_guid, commodity_scu, non_std_scu, parent_guid, code, description, hidden, placeholder)
         VALUES (?1, ?2, ?3, NULL, ?4, ?5, NULL, NULL, NULL, ?6, ?7)",
        params![root_account_guid, "Root Account", "ROOT", 100, 0, 0, 0],
    ).map_err(|e| internal_error("Failed to insert root account", e))?;

    for acc in accounts {
        let parent_guid = acc.parent_id.map(guid).unwrap_or_else(|| root_account_guid.to_string());
        let comm_guid = commodity_map.get(&acc.currency).cloned().unwrap_or_else(|| default_commodity_guid.to_string());

        tx.execute(
            "INSERT INTO accounts (guid, name, account_type, commodity_guid, commodity_scu, non_std_scu, parent_guid, code, description, hidden, placeholder)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                guid(acc.id),
                acc.name,
                acc.account_type.to_uppercase(),
                comm_guid,
                account_fraction(acc, currency_meta),
                0,
                parent_guid,
                acc.code,
                acc.description,
                if acc.hidden { 1 } else { 0 },
                if acc.placeholder { 1 } else { 0 },
            ],
        ).map_err(|e| internal_error("Failed to insert account", e))?;
    }
    Ok(())
}

/// Inserts transactions (posted at 10:59 to stay on the same date in any timezone) and their notes slots.
fn insert_transactions(
    tx: &Transaction,
    transactions: &[TransactionExportRow],
    tx_currency_map: &HashMap<Uuid, String>,
    book_currency: &str,
    commodity_map: &HashMap<String, String>,
    default_commodity_guid: &str,
) -> Result<(), ApiError> {
    for t in transactions {
        let tx_guid = guid(t.id);
        let tx_curr = tx_currency_map.get(&t.id).map(String::as_str).unwrap_or(book_currency);
        let comm_guid = commodity_map.get(tx_curr).cloned().unwrap_or_else(|| default_commodity_guid.to_string());

        tx.execute(
            "INSERT INTO transactions (guid, currency_guid, num, post_date, enter_date, description)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                tx_guid,
                comm_guid,
                t.num.clone().unwrap_or_default(),
                t.post_date.format("%Y%m%d105900").to_string(),
                t.enter_date.format("%Y%m%d%H%M%S").to_string(),
                t.description,
            ],
        ).map_err(|e| internal_error("Failed to insert transaction", e))?;

        if let Some(notes) = t.notes.as_deref().map(str::trim).filter(|n| !n.is_empty()) {
            tx.execute(
                "INSERT INTO slots (obj_guid, name, slot_type, string_val) VALUES (?1, 'notes', 4, ?2)",
                params![tx_guid, notes],
            ).map_err(|e| internal_error("Failed to insert transaction notes slot", e))?;
        }
    }
    Ok(())
}

/// Inserts splits: value in cents (denominator 100), quantity in the account's commodity fraction.
fn insert_splits(
    tx: &Transaction,
    splits: &[SplitExportRow],
    accounts: &[AccountExportRow],
    currency_meta: &CurrencyMeta,
) -> Result<(), ApiError> {
    let account_fraction_map: HashMap<Uuid, i64> =
        accounts.iter().map(|a| (a.id, account_fraction(a, currency_meta))).collect();

    for sp in splits {
        let val_num = (sp.amount * Decimal::from(100)).round().to_i64().unwrap_or(0);
        let qty_denom = account_fraction_map.get(&sp.account_id).copied().unwrap_or(100);
        let qty = sp.quantity.unwrap_or(sp.amount);
        let qty_num = (qty * Decimal::from(qty_denom)).round().to_i64().unwrap_or(0);
        let rec_date_str = sp.reconciled_date.map(|d| d.format("%Y%m%d%H%M%S").to_string());

        tx.execute(
            "INSERT INTO splits (guid, tx_guid, account_guid, memo, action, reconcile_state, reconcile_date, value_num, value_denom, quantity_num, quantity_denom, lot_guid)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, NULL)",
            params![
                guid(sp.id),
                guid(sp.transaction_id),
                guid(sp.account_id),
                sp.memo.clone().unwrap_or_default(),
                "",
                sp.reconciled_state,
                rec_date_str,
                val_num,
                100i64,
                qty_num,
                qty_denom,
            ],
        ).map_err(|e| internal_error("Failed to insert split", e))?;
    }
    Ok(())
}
