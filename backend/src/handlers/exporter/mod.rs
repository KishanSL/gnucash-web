mod currency;
mod schema;
mod writer;

pub use currency::{resolve_book_currency, resolve_currency_metadata, resolve_transaction_currencies};

use axum::{
    extract::State,
    http::{header, HeaderMap, HeaderValue},
    response::IntoResponse,
};
use chrono::Utc;
use rust_decimal::Decimal;
use sqlx::PgPool;
use uuid::Uuid;

use crate::auth::AuthUser;
use crate::handlers::{internal_error, ApiError};

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct AccountExportRow {
    pub id: Uuid,
    pub name: String,
    pub code: Option<String>,
    pub account_type: String,
    pub parent_id: Option<Uuid>,
    pub currency: String,
    pub description: Option<String>,
    pub placeholder: bool,
    pub hidden: bool,
    pub commodity_namespace: Option<String>,
    pub commodity_fraction: Option<i64>,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct TransactionExportRow {
    pub id: Uuid,
    pub post_date: chrono::NaiveDate,
    pub enter_date: chrono::DateTime<chrono::Utc>,
    pub num: Option<String>,
    pub description: String,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct SplitExportRow {
    pub id: Uuid,
    pub transaction_id: Uuid,
    pub account_id: Uuid,
    pub amount: Decimal,
    pub quantity: Option<Decimal>,
    pub memo: Option<String>,
    pub reconciled_state: String,
    pub reconciled_date: Option<chrono::DateTime<chrono::Utc>>,
}

pub async fn export_gnucash_sqlite(
    auth: AuthUser,
    State(pool): State<PgPool>,
) -> Result<impl IntoResponse, ApiError> {
    let accounts = sqlx::query_as::<_, AccountExportRow>(
        r#"SELECT id, name, code, account_type, parent_id, currency, description, placeholder, hidden, commodity_namespace, commodity_fraction
           FROM accounts
           WHERE user_id = $1
           ORDER BY code ASC NULLS LAST, name ASC"#
    )
    .bind(auth.id)
    .fetch_all(&pool)
    .await
    .map_err(|e| internal_error("Failed to fetch accounts for export", e))?;

    let transactions = sqlx::query_as::<_, TransactionExportRow>(
        r#"SELECT id, post_date, enter_date, num, description, notes
           FROM transactions
           WHERE user_id = $1
           ORDER BY post_date ASC, enter_date ASC"#
    )
    .bind(auth.id)
    .fetch_all(&pool)
    .await
    .map_err(|e| internal_error("Failed to fetch transactions for export", e))?;

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
    .map_err(|e| internal_error("Failed to fetch splits for export", e))?;

    // SQLite writing is blocking I/O
    let file_bytes = tokio::task::spawn_blocking(move || {
        writer::write_gnucash_sqlite(&accounts, &transactions, &splits)
    })
    .await
    .map_err(|e| internal_error("Export blocking task failed", e))??;

    let filename = export_filename(&auth.username);
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

/// `username_yyyyMMddhhmmss.gnucash`, with unsafe filename characters replaced by `_`.
fn export_filename(username: &str) -> String {
    let timestamp = Utc::now().format("%Y%m%d%H%M%S").to_string();
    let safe_username: String = username
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '_' || c == '-' { c } else { '_' })
        .collect();
    format!("{}_{}.gnucash", safe_username, timestamp)
}
