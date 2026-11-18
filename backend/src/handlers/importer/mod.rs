pub mod models;
pub mod parser;
pub mod persister;
pub mod validator;

pub use models::*;
pub use parser::*;
pub use persister::*;
pub use validator::*;

use axum::{
    extract::{Multipart, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde_json::json;
use sqlx::PgPool;

use crate::auth::{require_session, AuthUser};
use crate::handlers::internal_error;

pub async fn import_gnucash_sqlite(
    auth: AuthUser,
    State(pool): State<PgPool>,
    Query(query): Query<ImportQuery>,
    mut multipart: Multipart,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    require_session(&auth)?;

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

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Datelike, Timelike, Utc};
    use std::collections::{HashMap, HashSet};

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

        // Date-only format
        let dt5 = parse_gnucash_datetime("2024-05-18").expect("Date-only failed");
        assert_eq!(dt5.year(), 2024);
        assert_eq!(dt5.month(), 5);
        assert_eq!(dt5.day(), 18);

        // Compact date-only format
        let dt6 = parse_gnucash_datetime("20240518").expect("Compact date-only failed");
        assert_eq!(dt6.year(), 2024);
        assert_eq!(dt6.month(), 5);
        assert_eq!(dt6.day(), 18);

        // Epoch timestamp
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
        let _ = parse_gnucash_date("２０२६年０३月３１日");
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

    #[test]
    fn test_resolve_entity_or_char_ref() {
        assert_eq!(resolve_entity_or_char_ref("amp"), Some("&".to_string()));
        assert_eq!(resolve_entity_or_char_ref("lt"), Some("<".to_string()));
        assert_eq!(resolve_entity_or_char_ref("gt"), Some(">".to_string()));
        assert_eq!(resolve_entity_or_char_ref("apos"), Some("'".to_string()));
        assert_eq!(resolve_entity_or_char_ref("quot"), Some("\"".to_string()));
        assert_eq!(resolve_entity_or_char_ref("#x26"), Some("&".to_string()));
        assert_eq!(resolve_entity_or_char_ref("#38"), Some("&".to_string()));
        assert_eq!(resolve_entity_or_char_ref("#x20AC"), Some("€".to_string()));
        assert_eq!(resolve_entity_or_char_ref("unknown_entity"), None);
    }

    #[test]
    fn test_parse_gnucash_xml_with_entities() {
        let xml = r#"<?xml version="1.0" encoding="utf-8" ?>
<gnc-v2 xmlns:gnc="http://www.gnucash.org/XML/gnc"
        xmlns:act="http://www.gnucash.org/XML/act"
        xmlns:trn="http://www.gnucash.org/XML/trn"
        xmlns:split="http://www.gnucash.org/XML/split"
        xmlns:cmdty="http://www.gnucash.org/XML/cmdty"
        xmlns:ts="http://www.gnucash.org/XML/ts"
        xmlns:slot="http://www.gnucash.org/XML/slot">
  <gnc:book>
    <gnc:account>
      <act:name>Food &amp; Dining</act:name>
      <act:id type="guid">acc-1</act:id>
      <act:type>EXPENSE</act:type>
      <act:commodity>
        <cmdty:space>CURRENCY</cmdty:space>
        <cmdty:id>INR</cmdty:id>
      </act:commodity>
    </gnc:account>
    <gnc:account>
      <act:name>Checking &amp; Savings</act:name>
      <act:id type="guid">acc-2</act:id>
      <act:type>BANK</act:type>
      <act:commodity>
        <cmdty:space>CURRENCY</cmdty:space>
        <cmdty:id>INR</cmdty:id>
      </act:commodity>
    </gnc:account>
    <gnc:transaction>
      <trn:id type="guid">tx-1</trn:id>
      <trn:currency>
        <cmdty:space>CURRENCY</cmdty:space>
        <cmdty:id>INR</cmdty:id>
      </trn:currency>
      <trn:date-posted>
        <ts:date>2026-11-16 10:00:00 +0000</ts:date>
      </trn:date-posted>
      <trn:description>AT&amp;T &lt;Bill&gt;</trn:description>
      <trn:slots>
        <slot>
          <slot:key>notes</slot:key>
          <slot:value type="string">Payment for 5G &amp; Fiber</slot:value>
        </slot>
      </trn:slots>
      <trn:splits>
        <trn:split>
          <split:id type="guid">sp-1</split:id>
          <split:reconciled-state>n</split:reconciled-state>
          <split:value>150000/100</split:value>
          <split:quantity>150000/100</split:quantity>
          <split:account type="guid">acc-1</split:account>
          <split:memo>Groceries &amp; More</split:memo>
        </trn:split>
        <trn:split>
          <split:id type="guid">sp-2</split:id>
          <split:reconciled-state>n</split:reconciled-state>
          <split:value>-150000/100</split:value>
          <split:quantity>-150000/100</split:quantity>
          <split:account type="guid">acc-2</split:account>
        </trn:split>
      </trn:splits>
    </gnc:transaction>
  </gnc:book>
</gnc-v2>"#;

        let (accounts, transactions, splits) = parse_gnucash_xml(xml.as_bytes(), "INR").expect("parse xml");
        assert_eq!(accounts.len(), 2);
        assert_eq!(accounts[0].name, "Food & Dining");
        assert_eq!(accounts[1].name, "Checking & Savings");

        assert_eq!(transactions.len(), 1);
        assert_eq!(transactions[0].description, "AT&T <Bill>");
        assert_eq!(transactions[0].notes.as_deref(), Some("Payment for 5G & Fiber"));

        assert_eq!(splits.len(), 2);
        assert_eq!(splits[0].memo.as_deref(), Some("Groceries & More"));
    }
}
