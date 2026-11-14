use axum::{
    extract::State,
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::IntoResponse,
    Json,
};
use chrono::Utc;
use rusqlite::{params, Connection};
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use serde_json::json;
use sqlx::PgPool;
use std::collections::HashMap;
use tempfile::NamedTempFile;
use uuid::Uuid;

use crate::auth::AuthUser;

#[derive(Debug, sqlx::FromRow)]
struct AccountExportRow {
    id: Uuid,
    name: String,
    code: Option<String>,
    account_type: String,
    parent_id: Option<Uuid>,
    currency: String,
    description: Option<String>,
    placeholder: bool,
    hidden: bool,
    commodity_namespace: Option<String>,
    commodity_fraction: Option<i64>,
}

#[derive(Debug, sqlx::FromRow)]
struct TransactionExportRow {
    id: Uuid,
    post_date: chrono::NaiveDate,
    enter_date: chrono::DateTime<chrono::Utc>,
    num: Option<String>,
    description: String,
    notes: Option<String>,
}

#[derive(Debug, sqlx::FromRow)]
struct SplitExportRow {
    id: Uuid,
    transaction_id: Uuid,
    account_id: Uuid,
    amount: Decimal,
    quantity: Option<Decimal>,
    memo: Option<String>,
    reconciled_state: String,
    reconciled_date: Option<chrono::DateTime<chrono::Utc>>,
}

/// Exports all accounts, transactions, and splits for the authenticated user as a standard GnuCash SQLite (.gnucash) file.
/// Filename format: username_yyyyMMddhhmmss.gnucash
pub async fn export_gnucash_sqlite(
    auth: AuthUser,
    State(pool): State<PgPool>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    // 1. Fetch user accounts
    let accounts = sqlx::query_as::<_, AccountExportRow>(
        r#"SELECT id, name, code, account_type, parent_id, currency, description, placeholder, hidden, commodity_namespace, commodity_fraction
           FROM accounts
           WHERE user_id = $1
           ORDER BY code ASC NULLS LAST, name ASC"#
    )
    .bind(auth.id)
    .fetch_all(&pool)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Failed to fetch accounts: {}", e) })),
        )
    })?;

    // 2. Fetch user transactions
    let transactions = sqlx::query_as::<_, TransactionExportRow>(
        r#"SELECT id, post_date, enter_date, num, description, notes
           FROM transactions
           WHERE user_id = $1
           ORDER BY post_date ASC, enter_date ASC"#
    )
    .bind(auth.id)
    .fetch_all(&pool)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Failed to fetch transactions: {}", e) })),
        )
    })?;

    // 3. Fetch user splits
    let splits = sqlx::query_as::<_, SplitExportRow>(
        r#"SELECT s.id, s.transaction_id, s.account_id, s.amount, s.quantity, s.memo, s.reconciled_state, s.reconciled_date
           FROM splits s
           JOIN transactions t ON s.transaction_id = t.id
           WHERE t.user_id = $1
           ORDER BY s.id ASC"#
    )
    .bind(auth.id)
    .fetch_all(&pool)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Failed to fetch splits: {}", e) })),
        )
    })?;

    // 4. Create SQLite database in temp file via background blocking task (with single transaction)
    let file_bytes = tokio::task::spawn_blocking(move || -> Result<Vec<u8>, (StatusCode, Json<serde_json::Value>)> {
        let temp_file = NamedTempFile::new().map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": format!("Temp file error: {}", e) })),
            )
        })?;
        let temp_path = temp_file.path().to_path_buf();

        let mut conn = Connection::open(&temp_path).map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": format!("SQLite connection failed: {}", e) })),
            )
        })?;

        conn.pragma_update(None, "journal_mode", "OFF").ok();
        conn.pragma_update(None, "synchronous", "OFF").ok();

        let sqlite_tx = conn.transaction().map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": format!("Failed to start SQLite transaction: {}", e) })),
            )
        })?;

        // Create standard GnuCash SQLite schema
        sqlite_tx.execute_batch(
        r#"
        CREATE TABLE gnclock (
            Hostname text(255),
            PID integer
        );

        CREATE TABLE versions (
            table_name text(50) PRIMARY KEY,
            table_version integer NOT NULL
        );

        CREATE TABLE books (
            guid text(32) PRIMARY KEY,
            root_account_guid text(32) NOT NULL,
            root_template_guid text(32) NOT NULL
        );

        CREATE TABLE commodities (
            guid text(32) PRIMARY KEY,
            namespace text(2048) NOT NULL,
            mnemonic text(2048) NOT NULL,
            fullname text(2048),
            cusip text(2048),
            fraction integer NOT NULL,
            quote_flag integer NOT NULL,
            quote_source text(2048)
        );

        CREATE TABLE accounts (
            guid text(32) PRIMARY KEY,
            name text(2048) NOT NULL,
            account_type text(2048) NOT NULL,
            commodity_guid text(32),
            commodity_scu integer NOT NULL,
            non_std_scu integer NOT NULL,
            parent_guid text(32),
            code text(2048),
            description text(2048),
            hidden integer,
            placeholder integer
        );

        CREATE TABLE transactions (
            guid text(32) PRIMARY KEY,
            currency_guid text(32) NOT NULL,
            num text(2048) NOT NULL,
            post_date text(14),
            enter_date text(14),
            description text(2048)
        );

        CREATE TABLE splits (
            guid text(32) PRIMARY KEY,
            tx_guid text(32) NOT NULL,
            account_guid text(32) NOT NULL,
            memo text(2048) NOT NULL,
            action text(2048) NOT NULL,
            reconcile_state text(1) NOT NULL,
            reconcile_date text(14),
            value_num integer NOT NULL,
            value_denom integer NOT NULL,
            quantity_num integer NOT NULL,
            quantity_denom integer NOT NULL,
            lot_guid text(32)
        );

        CREATE TABLE slots (
            id integer PRIMARY KEY AUTOINCREMENT,
            obj_guid text(32) NOT NULL,
            name text(4096) NOT NULL,
            slot_type integer NOT NULL,
            int64_val integer,
            string_val text(4096),
            double_val real,
            timespec_val text(14),
            guid_val text(32),
            numeric_val_num integer,
            numeric_val_denom integer,
            gdate_val text(8)
        );

        CREATE INDEX tx_post_date_index ON transactions (post_date);
        CREATE INDEX splits_tx_guid_index ON splits (tx_guid);
        CREATE INDEX splits_account_guid_index ON splits (account_guid);
        CREATE INDEX slots_guid_index ON slots (obj_guid);

        INSERT INTO versions (table_name, table_version) VALUES
            ('Gnucash', 5000017),
            ('Gnucash-Resave', 19920),
            ('accounts', 1),
            ('books', 1),
            ('commodities', 1),
            ('transactions', 4),
            ('splits', 5),
            ('slots', 4),
            ('prices', 3),
            ('recurrences', 2),
            ('lots', 2),
            ('billterms', 2),
            ('customers', 2),
            ('employees', 2),
            ('entries', 4),
            ('invoices', 4),
            ('jobs', 1),
            ('orders', 1),
            ('taxtables', 2),
            ('taxtable_entries', 3),
            ('vendors', 1),
            ('budgets', 1),
            ('budget_amounts', 1),
            ('schedxactions', 1);
        "#
    ).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Failed to initialize GnuCash tables: {}", e) })),
        )
    })?;

    // Generate GUIDs for book and root account
    let book_guid = Uuid::new_v4().to_string().replace('-', "");
    let root_account_guid = Uuid::new_v4().to_string().replace('-', "");
    let root_template_guid = Uuid::new_v4().to_string().replace('-', "");

    sqlite_tx.execute(
        "INSERT INTO books (guid, root_account_guid, root_template_guid) VALUES (?1, ?2, ?3)",
        params![book_guid, root_account_guid, root_template_guid],
    ).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Failed to insert book: {}", e) })),
        )
    })?;

    // Insert SQLite3 ISO dates feature slot on the book
    sqlite_tx.execute(
        "INSERT INTO slots (obj_guid, name, slot_type, string_val)
         VALUES (?1, 'features/ISO-8601 formatted date strings in SQLite3 databases.', 4, 'Use ISO formatted date-time strings in SQLite3 databases (requires at least GnuCash 2.6.20)')",
        params![book_guid],
    ).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Failed to insert book feature slot: {}", e) })),
        )
    })?;

    // Collect all currencies and insert commodities
    let mut commodity_map: HashMap<String, String> = HashMap::new();
    let mut currencies: Vec<String> = accounts.iter().map(|a| a.currency.clone()).collect();
    if currencies.is_empty() {
        currencies.push("INR".to_string());
    }
    currencies.sort();
    currencies.dedup();

    let mut currency_meta: HashMap<String, (String, i64)> = HashMap::new();
    for acc in &accounts {
        let is_iso_currency = acc.currency.len() == 3 && acc.currency.chars().all(|c| c.is_ascii_alphabetic());
        let default_ns = if is_iso_currency { "CURRENCY".to_string() } else { "FUND".to_string() };
        let ns = acc.commodity_namespace.clone().unwrap_or(default_ns);
        let fr = acc.commodity_fraction.unwrap_or(if ns == "CURRENCY" { 100 } else { 1000 });
        currency_meta.insert(acc.currency.clone(), (ns, fr));
    }

    for curr in &currencies {
        let comm_guid = Uuid::new_v5(&Uuid::NAMESPACE_DNS, curr.as_bytes()).to_string().replace('-', "");
        commodity_map.insert(curr.clone(), comm_guid.clone());

        let (namespace, fraction) = currency_meta.get(curr).cloned().unwrap_or_else(|| {
            let is_iso = curr.len() == 3 && curr.chars().all(|c| c.is_ascii_alphabetic());
            (if is_iso { "CURRENCY".to_string() } else { "FUND".to_string() }, 100)
        });
        let quote_source = if namespace == "CURRENCY" { "currency" } else { "financequote" };

        sqlite_tx.execute(
            "INSERT INTO commodities (guid, namespace, mnemonic, fullname, cusip, fraction, quote_flag, quote_source)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                comm_guid,
                namespace,
                curr,
                curr,
                "",
                fraction,
                1,
                quote_source
            ],
        ).map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": format!("Failed to insert commodity: {}", e) })),
            )
        })?;
    }

    let default_commodity_guid = currencies.first()
        .and_then(|c| commodity_map.get(c))
        .cloned()
        .unwrap_or_else(|| Uuid::new_v4().to_string().replace('-', ""));

    // Insert Root account
    sqlite_tx.execute(
        "INSERT INTO accounts (guid, name, account_type, commodity_guid, commodity_scu, non_std_scu, parent_guid, code, description, hidden, placeholder)
         VALUES (?1, ?2, ?3, NULL, ?4, ?5, NULL, NULL, NULL, ?6, ?7)",
        params![
            root_account_guid,
            "Root Account",
            "ROOT",
            100,
            0,
            0,
            0
        ],
    ).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Failed to insert root account: {}", e) })),
        )
    })?;

    // Insert Accounts
    let mut account_currency_map: HashMap<Uuid, String> = HashMap::new();
    for acc in &accounts {
        account_currency_map.insert(acc.id, acc.currency.clone());
        let acc_guid = acc.id.to_string().replace('-', "");
        let parent_guid = acc.parent_id
            .map(|p| p.to_string().replace('-', ""))
            .unwrap_or_else(|| root_account_guid.clone());

        let comm_guid = commodity_map.get(&acc.currency).cloned().unwrap_or_else(|| default_commodity_guid.clone());
        let acc_type = acc.account_type.to_uppercase();

        sqlite_tx.execute(
            "INSERT INTO accounts (guid, name, account_type, commodity_guid, commodity_scu, non_std_scu, parent_guid, code, description, hidden, placeholder)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                acc_guid,
                acc.name,
                acc_type,
                comm_guid,
                100,
                0,
                parent_guid,
                acc.code,
                acc.description,
                if acc.hidden { 1 } else { 0 },
                if acc.placeholder { 1 } else { 0 },
            ],
        ).map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": format!("Failed to insert account: {}", e) })),
            )
        })?;
    }

    // Map split account currencies to transactions
    let mut tx_currency_map: HashMap<Uuid, String> = HashMap::new();
    for s in &splits {
        if !tx_currency_map.contains_key(&s.transaction_id) {
            if let Some(curr) = account_currency_map.get(&s.account_id) {
                tx_currency_map.insert(s.transaction_id, curr.clone());
            }
        }
    }

    // Insert Transactions
    for tx in &transactions {
        let tx_guid = tx.id.to_string().replace('-', "");
        let tx_curr = tx_currency_map.get(&tx.id).cloned().unwrap_or_else(|| "INR".to_string());
        let comm_guid = commodity_map.get(&tx_curr).cloned().unwrap_or_else(|| default_commodity_guid.clone());
        let post_date_str = tx.post_date.format("%Y%m%d105900").to_string();
        let enter_date_str = tx.enter_date.format("%Y%m%d%H%M%S").to_string();

        sqlite_tx.execute(
            "INSERT INTO transactions (guid, currency_guid, num, post_date, enter_date, description)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                tx_guid,
                comm_guid,
                tx.num.clone().unwrap_or_default(),
                post_date_str,
                enter_date_str,
                tx.description,
            ],
        ).map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": format!("Failed to insert transaction: {}", e) })),
            )
        })?;

        if let Some(ref notes) = tx.notes {
            if !notes.trim().is_empty() {
                sqlite_tx.execute(
                    "INSERT INTO slots (obj_guid, name, slot_type, string_val) VALUES (?1, 'notes', 4, ?2)",
                    params![tx_guid, notes.trim()],
                ).map_err(|e| {
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!({ "error": format!("Failed to insert transaction notes slot: {}", e) })),
                    )
                })?;
            }
        }
    }

    // Map account_id -> commodity fraction
    let account_fraction_map: HashMap<Uuid, i64> = accounts
        .iter()
        .map(|a| (a.id, a.commodity_fraction.unwrap_or(100)))
        .collect();

    // Insert Splits
    for sp in &splits {
        let sp_guid = sp.id.to_string().replace('-', "");
        let tx_guid = sp.transaction_id.to_string().replace('-', "");
        let acc_guid = sp.account_id.to_string().replace('-', "");

        let val_num = (sp.amount * Decimal::from(100)).round().to_i64().unwrap_or(0);
        let val_denom = 100i64;
        let qty_denom = account_fraction_map.get(&sp.account_id).copied().unwrap_or(100);
        let qty = sp.quantity.unwrap_or(sp.amount);
        let qty_num = (qty * Decimal::from(qty_denom)).round().to_i64().unwrap_or(0);
        let rec_date_str = sp.reconciled_date.map(|d| d.format("%Y%m%d%H%M%S").to_string());

        sqlite_tx.execute(
            "INSERT INTO splits (guid, tx_guid, account_guid, memo, action, reconcile_state, reconcile_date, value_num, value_denom, quantity_num, quantity_denom, lot_guid)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, NULL)",
            params![
                sp_guid,
                tx_guid,
                acc_guid,
                sp.memo.clone().unwrap_or_default(),
                "",
                sp.reconciled_state,
                rec_date_str,
                val_num,
                val_denom,
                qty_num,
                qty_denom,
            ],
        ).map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": format!("Failed to insert split: {}", e) })),
            )
        })?;
    }

        // Commit SQLite export transaction
        sqlite_tx.commit().map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": format!("Failed to commit SQLite export: {}", e) })),
            )
        })?;

        drop(conn);

        std::fs::read(&temp_path).map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": format!("Failed to read generated SQLite file: {}", e) })),
            )
        })
    })
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Export blocking task failed: {}", e) })),
        )
    })??;

    // Format filename: username_yyyyMMddhhmmss.gnucash
    let timestamp = Utc::now().format("%Y%m%d%H%M%S").to_string();
    let safe_username: String = auth.username
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '_' || c == '-' { c } else { '_' })
        .collect();
    let filename = format!("{}_{}.gnucash", safe_username, timestamp);

    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/x-sqlite3"),
    );
    headers.insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_str(&format!("attachment; filename=\"{}\"", filename))
            .unwrap_or_else(|_| HeaderValue::from_static("attachment; filename=\"export.gnucash\"")),
    );
    headers.insert(
        header::ACCESS_CONTROL_EXPOSE_HEADERS,
        HeaderValue::from_static("Content-Disposition"),
    );

    Ok((headers, file_bytes).into_response())
}
