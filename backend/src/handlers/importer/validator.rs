use axum::{http::StatusCode, Json};
use chrono::Utc;
use rust_decimal::Decimal;
use serde_json::json;
use std::collections::{HashMap, HashSet};

use super::models::{RawAccount, RawSplit, RawTransaction};

pub type ImportResult<T> = Result<T, (StatusCode, Json<serde_json::Value>)>;

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

pub fn get_tx_all_accounts_string(
    tx_guid: &str,
    raw_splits: &[RawSplit],
    acc_by_guid: &HashMap<String, &RawAccount>,
    initial_account_names: &HashMap<String, String>,
) -> String {
    let mut acc_names = Vec::new();
    let mut seen = HashSet::new();
    for sp in raw_splits {
        if sp.tx_guid == tx_guid {
            let name = if let Some(acc) = acc_by_guid.get(&sp.account_guid) {
                build_full_account_name(&acc.guid, acc_by_guid)
            } else if let Some(raw_name) = initial_account_names.get(&sp.account_guid) {
                if !raw_name.trim().is_empty() {
                    raw_name.clone()
                } else {
                    sp.account_guid.clone()
                }
            } else {
                sp.account_guid.clone()
            };
            if seen.insert(name.clone()) {
                acc_names.push(name);
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

/// Pre-validates account names, codes, and currency fields (IMP-11, ACC-11).
pub fn validate_accounts(
    raw_accounts: &[RawAccount],
    acc_by_guid: &HashMap<String, &RawAccount>,
) -> ImportResult<()> {
    for acc in raw_accounts {
        let full_name = build_full_account_name(&acc.guid, acc_by_guid);
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
    Ok(())
}

/// Normalizes split reconciled state and pre-validates split account and transaction references (ACC-2, IMP-4, IMP-10, IMP-11, REC-2).
pub fn normalize_and_validate_splits(
    raw_splits: &mut [RawSplit],
    raw_transactions: &[RawTransaction],
    acc_by_guid: &HashMap<String, &RawAccount>,
    initial_account_names: &HashMap<String, String>,
) -> ImportResult<()> {
    let tx_by_guid: HashMap<String, &RawTransaction> =
        raw_transactions.iter().map(|t| (t.guid.clone(), t)).collect();

    for s in raw_splits.iter_mut() {
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

    for s in raw_splits.iter() {
        if !acc_by_guid.contains_key(&s.account_guid) {
            let (date_str, desc_str, accounts_str) = if let Some(tx) = tx_by_guid.get(&s.tx_guid) {
                (
                    tx.post_date.to_string(),
                    tx_desc_preview(&tx.description),
                    get_tx_all_accounts_string(&tx.guid, raw_splits, acc_by_guid, initial_account_names),
                )
            } else {
                ("unknown".to_string(), "unknown".to_string(), "unknown".to_string())
            };

            return Err((
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": format!(
                        "Import error in transaction (date: {}, description: '{}', accounts: [{}]): split references unknown or dropped account GUID '{}' (IMP-11, ACC-2, IMP-4)",
                        date_str, desc_str, accounts_str, s.account_guid
                    )
                })),
            ));
        }

        if !tx_by_guid.contains_key(&s.tx_guid) {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": format!(
                        "Import error: split '{}' references unknown transaction GUID '{}' (IMP-11, IMP-4)",
                        s.guid, s.tx_guid
                    )
                })),
            ));
        }
    }
    Ok(())
}

/// Filters out transactions with fewer than 2 splits and validates transaction descriptions and check numbers (IMP-10, IMP-11, TXN-1, ACC-1).
pub fn filter_and_validate_transactions(
    raw_transactions: &mut Vec<RawTransaction>,
    raw_splits: &mut Vec<RawSplit>,
    acc_by_guid: &HashMap<String, &RawAccount>,
) -> ImportResult<usize> {
    let mut split_count_per_tx: HashMap<String, usize> = HashMap::new();
    for s in raw_splits.iter() {
        *split_count_per_tx.entry(s.tx_guid.clone()).or_insert(0) += 1;
    }

    let mut invalid_tx_guids: HashSet<String> = HashSet::new();
    for tx in raw_transactions.iter() {
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

    for tx in raw_transactions.iter() {
        let desc_len = tx.description.chars().count();
        let preview = tx_desc_preview(&tx.description);
        let accounts_str = get_tx_accounts_string(&tx.guid, raw_splits, acc_by_guid);

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

    Ok(skipped_transactions)
}

/// Verifies double-entry invariant across all imported transaction splits (ACC-2, IMP-11, Issue #19, Issue #35).
pub fn verify_double_entry(
    raw_splits: &[RawSplit],
    raw_transactions: &[RawTransaction],
    acc_by_guid: &HashMap<String, &RawAccount>,
) -> ImportResult<()> {
    let tx_by_guid: HashMap<String, &RawTransaction> =
        raw_transactions.iter().map(|t| (t.guid.clone(), t)).collect();

    let mut tx_sums: HashMap<String, Decimal> = HashMap::new();
    for s in raw_splits {
        *tx_sums.entry(s.tx_guid.clone()).or_insert(Decimal::ZERO) += s.amount;
    }

    for (tx_guid, sum) in &tx_sums {
        if *sum != Decimal::ZERO {
            let (date_str, desc_str, accounts_str) = if let Some(tx) = tx_by_guid.get(tx_guid) {
                (
                    tx.post_date.to_string(),
                    tx_desc_preview(&tx.description),
                    get_tx_accounts_string(tx_guid, raw_splits, acc_by_guid),
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

    Ok(())
}
