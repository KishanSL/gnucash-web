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
        r#"SELECT id, name, code, account_type, parent_id, currency, description, placeholder, hidden
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
        r#"SELECT s.id, s.transaction_id, s.account_id, s.amount, s.memo, s.reconciled_state, s.reconciled_date
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

    // 4. Create SQLite database in temp file
    let temp_file = NamedTempFile::new().map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Temp file error: {}", e) })),
        )
    })?;
    let temp_path = temp_file.path().to_path_buf();

    let conn = Connection::open(&temp_path).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("SQLite connection failed: {}", e) })),
        )
    })?;

    // Create standard GnuCash SQLite schema
    conn.execute_batch(
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
            numeric_val_denom integer
        );

        INSERT INTO versions VALUES ('accounts', 1);
        INSERT INTO versions VALUES ('books', 1);
        INSERT INTO versions VALUES ('commodities', 1);
        INSERT INTO versions VALUES ('transactions', 2);
        INSERT INTO versions VALUES ('splits', 1);
        INSERT INTO versions VALUES ('slots', 2);
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

    conn.execute(
        "INSERT INTO books (guid, root_account_guid, root_template_guid) VALUES (?1, ?2, ?3)",
        params![book_guid, root_account_guid, root_template_guid],
    ).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Failed to insert book: {}", e) })),
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

    for curr in &currencies {
        let comm_guid = Uuid::new_v5(&Uuid::NAMESPACE_DNS, curr.as_bytes()).to_string().replace('-', "");
        commodity_map.insert(curr.clone(), comm_guid.clone());

        conn.execute(
            "INSERT INTO commodities (guid, namespace, mnemonic, fullname, cusip, fraction, quote_flag, quote_source)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                comm_guid,
                "CURRENCY",
                curr,
                curr,
                "",
                100,
                1,
                "currency"
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
    conn.execute(
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

        conn.execute(
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
        let post_date_str = tx.post_date.format("%Y%m%d000000").to_string();
        let enter_date_str = tx.enter_date.format("%Y%m%d%H%M%S").to_string();

        conn.execute(
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
    }

    // Insert Splits
    for sp in &splits {
        let sp_guid = sp.id.to_string().replace('-', "");
        let tx_guid = sp.transaction_id.to_string().replace('-', "");
        let acc_guid = sp.account_id.to_string().replace('-', "");

        let val_num = (sp.amount * Decimal::from(100)).round().to_i64().unwrap_or(0);
        let val_denom = 100i64;
        let rec_date_str = sp.reconciled_date.map(|d| d.format("%Y%m%d%H%M%S").to_string());

        conn.execute(
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
                val_num,
                val_denom,
            ],
        ).map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": format!("Failed to insert split: {}", e) })),
            )
        })?;
    }

    // Close SQLite connection to flush all writes to disk
    drop(conn);

    let file_bytes = std::fs::read(&temp_path).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Failed to read generated SQLite file: {}", e) })),
        )
    })?;

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
