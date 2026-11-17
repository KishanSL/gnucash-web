//! Commodity, book and transaction currency resolution for export (INV-5, EXP-5, Issue #26).

use std::collections::HashMap;
use uuid::Uuid;

use super::{AccountExportRow, SplitExportRow};

/// Commodity mnemonic -> (namespace, fraction).
pub type CurrencyMeta = HashMap<String, (String, i64)>;

/// Resolves commodity metadata (namespace, fraction) and unique currency list for exported accounts (INV-5, EXP-5, Issue #26).
/// Explicit account metadata takes precedence. Missing values default to CURRENCY (fraction 100) for 3-letter alphabetic codes,
/// or FUND (fraction 1000) otherwise. Ensures at least one CURRENCY commodity exists (defaulting to INR).
pub fn resolve_currency_metadata(
    accounts: &[AccountExportRow],
) -> (HashMap<String, (String, i64)>, Vec<String>) {
    let mut currency_meta: HashMap<String, (String, i64)> = HashMap::new();
    for acc in accounts {
        if let Some(ref ns) = acc.commodity_namespace {
            let fr = acc.commodity_fraction.unwrap_or(if ns == "CURRENCY" { 100 } else { 1000 });
            currency_meta.insert(acc.currency.clone(), (ns.clone(), fr));
        }
    }
    for acc in accounts {
        if !currency_meta.contains_key(&acc.currency) {
            let is_iso_currency = acc.currency.len() == 3 && acc.currency.chars().all(|c| c.is_ascii_alphabetic());
            let default_ns = if is_iso_currency { "CURRENCY".to_string() } else { "FUND".to_string() };
            let fr = acc.commodity_fraction.unwrap_or(if default_ns == "CURRENCY" { 100 } else { 1000 });
            currency_meta.insert(acc.currency.clone(), (default_ns, fr));
        }
    }

    let mut currencies: Vec<String> = accounts.iter().map(|a| a.currency.clone()).collect();
    let has_currency_ns = currencies.iter().any(|c| {
        currency_meta.get(c).map(|(ns, _)| ns == "CURRENCY").unwrap_or(false)
    });
    if !has_currency_ns {
        if !currencies.contains(&"INR".to_string()) {
            currencies.push("INR".to_string());
        }
        currency_meta.insert("INR".to_string(), ("CURRENCY".to_string(), 100));
    }
    currencies.sort();
    currencies.dedup();

    (currency_meta, currencies)
}

/// Determines the book currency from CURRENCY-namespace accounts, defaulting to INR (INV-5, SCOPE-1, Issue #26).
pub fn resolve_book_currency(
    accounts: &[AccountExportRow],
    currency_meta: &HashMap<String, (String, i64)>,
) -> String {
    let mut currency_acc_counts: HashMap<&str, usize> = HashMap::new();
    for acc in accounts {
        if currency_meta.get(&acc.currency).map(|(ns, _)| ns == "CURRENCY").unwrap_or(false) {
            *currency_acc_counts.entry(acc.currency.as_str()).or_insert(0) += 1;
        }
    }
    currency_acc_counts
        .into_iter()
        .max_by_key(|&(_, count)| count)
        .map(|(curr, _)| curr.to_string())
        .unwrap_or_else(|| "INR".to_string())
}

/// Resolves transaction currency from splits, considering only CURRENCY-namespace accounts (INV-5, SCOPE-1, Issue #26).
/// Non-currency commodities such as mutual funds or stocks are excluded so that transactions are never assigned a fund currency.
pub fn resolve_transaction_currencies(
    splits: &[SplitExportRow],
    account_currency_map: &HashMap<Uuid, String>,
    currency_meta: &HashMap<String, (String, i64)>,
) -> HashMap<Uuid, String> {
    let mut tx_currency_splits: HashMap<Uuid, Vec<String>> = HashMap::new();
    for s in splits {
        if let Some(curr) = account_currency_map.get(&s.account_id) {
            let is_currency_ns = currency_meta
                .get(curr)
                .map(|(ns, _)| ns == "CURRENCY")
                .unwrap_or(false);
            if is_currency_ns {
                tx_currency_splits.entry(s.transaction_id).or_default().push(curr.clone());
            }
        }
    }

    let mut tx_currency_map: HashMap<Uuid, String> = HashMap::new();
    for (tx_id, curr_list) in tx_currency_splits {
        let mut counts: HashMap<&str, usize> = HashMap::new();
        for c in &curr_list {
            *counts.entry(c.as_str()).or_insert(0) += 1;
        }
        if let Some((most_frequent, _)) = counts.into_iter().max_by_key(|&(_, count)| count) {
            tx_currency_map.insert(tx_id, most_frequent.to_string());
        }
    }
    tx_currency_map
}

/// Exports all accounts, transactions, and splits for the authenticated user as a standard GnuCash SQLite (.gnucash) file.
#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::Decimal;
    use std::collections::HashMap;
    use uuid::Uuid;

    fn make_test_account(
        currency: &str,
        commodity_namespace: Option<&str>,
        commodity_fraction: Option<i64>,
    ) -> AccountExportRow {
        AccountExportRow {
            id: Uuid::new_v4(),
            name: format!("Account {}", currency),
            code: None,
            account_type: "asset".to_string(),
            parent_id: None,
            currency: currency.to_string(),
            description: None,
            placeholder: false,
            hidden: false,
            commodity_namespace: commodity_namespace.map(|s| s.to_string()),
            commodity_fraction,
        }
    }

    fn make_test_split(
        transaction_id: Uuid,
        account_id: Uuid,
        amount: Decimal,
        quantity: Option<Decimal>,
    ) -> SplitExportRow {
        SplitExportRow {
            id: Uuid::new_v4(),
            transaction_id,
            account_id,
            amount,
            quantity,
            memo: None,
            reconciled_state: "n".to_string(),
            reconciled_date: None,
        }
    }

    #[test]
    fn test_resolve_currency_metadata_iso_and_fund_defaults() {
        let accounts = vec![
            make_test_account("USD", None, None),
            make_test_account("INF209K01VA3", None, None),
            make_test_account("HDFCBANK.NS", None, None),
        ];

        let (meta, currencies) = resolve_currency_metadata(&accounts);
        assert_eq!(meta.get("USD"), Some(&("CURRENCY".to_string(), 100)));
        assert_eq!(meta.get("INF209K01VA3"), Some(&("FUND".to_string(), 1000)));
        assert_eq!(meta.get("HDFCBANK.NS"), Some(&("FUND".to_string(), 1000)));
        assert!(currencies.contains(&"USD".to_string()));
        assert!(currencies.contains(&"INF209K01VA3".to_string()));
        assert!(currencies.contains(&"HDFCBANK.NS".to_string()));
    }

    #[test]
    fn test_resolve_currency_metadata_explicit_metadata_takes_precedence() {
        let accounts = vec![
            make_test_account("CUSTOM", Some("CURRENCY"), Some(100)),
            make_test_account("USD", Some("CURRENCY"), Some(10000)),
            make_test_account("MYFUND", Some("FUND"), Some(500)),
        ];

        let (meta, _) = resolve_currency_metadata(&accounts);
        assert_eq!(meta.get("CUSTOM"), Some(&("CURRENCY".to_string(), 100)));
        assert_eq!(meta.get("USD"), Some(&("CURRENCY".to_string(), 10000)));
        assert_eq!(meta.get("MYFUND"), Some(&("FUND".to_string(), 500)));
    }

    #[test]
    fn test_resolve_currency_metadata_ensures_at_least_one_currency_commodity() {
        // When accounts only contain FUND commodities, INR must be added
        let accounts = vec![
            make_test_account("INF209K01VA3", Some("FUND"), Some(1000)),
        ];

        let (meta, currencies) = resolve_currency_metadata(&accounts);
        assert!(currencies.contains(&"INR".to_string()));
        assert_eq!(meta.get("INR"), Some(&("CURRENCY".to_string(), 100)));

        // Empty accounts list also ensures INR
        let empty_accounts: Vec<AccountExportRow> = vec![];
        let (empty_meta, empty_currencies) = resolve_currency_metadata(&empty_accounts);
        assert!(empty_currencies.contains(&"INR".to_string()));
        assert_eq!(empty_meta.get("INR"), Some(&("CURRENCY".to_string(), 100)));
    }

    #[test]
    fn test_resolve_book_currency_ignores_funds() {
        let accounts = vec![
            make_test_account("INR", None, None),
            make_test_account("INR", None, None),
            make_test_account("USD", None, None),
            make_test_account("INF209K01VA3", None, None),
            make_test_account("INF209K01VA3", None, None),
            make_test_account("INF209K01VA3", None, None),
        ];

        let (meta, _) = resolve_currency_metadata(&accounts);
        let book_curr = resolve_book_currency(&accounts, &meta);
        // Even though INF209K01VA3 appears 3 times, it's a FUND, so INR (2 times) wins
        assert_eq!(book_curr, "INR");

        // When only funds exist, fallback is INR
        let fund_only_accounts = vec![
            make_test_account("INF209K01VA3", None, None),
        ];
        let (fund_meta, _) = resolve_currency_metadata(&fund_only_accounts);
        let fallback_curr = resolve_book_currency(&fund_only_accounts, &fund_meta);
        assert_eq!(fallback_curr, "INR");
    }

    #[test]
    fn test_resolve_transaction_currencies_investment_trade_never_picks_fund() {
        let bank_account = make_test_account("INR", Some("CURRENCY"), Some(100));
        let fund_account = make_test_account("INF209K01VA3", Some("FUND"), Some(1000));

        let accounts = vec![bank_account.clone(), fund_account.clone()];
        let (meta, _) = resolve_currency_metadata(&accounts);

        let mut acc_map = HashMap::new();
        acc_map.insert(bank_account.id, bank_account.currency.clone());
        acc_map.insert(fund_account.id, fund_account.currency.clone());

        let tx_id = Uuid::new_v4();
        // Investment trade: Bank split and Fund split
        let splits = vec![
            make_test_split(tx_id, fund_account.id, Decimal::new(1500, 0), Some(Decimal::new(10, 0))),
            make_test_split(tx_id, bank_account.id, Decimal::new(-1500, 0), None),
        ];

        let tx_curr_map = resolve_transaction_currencies(&splits, &acc_map, &meta);
        // Crucial invariant (Issue #26): Transaction currency must be INR, NEVER INF209K01VA3
        assert_eq!(tx_curr_map.get(&tx_id), Some(&"INR".to_string()));
        assert_ne!(tx_curr_map.get(&tx_id), Some(&"INF209K01VA3".to_string()));
    }

    #[test]
    fn test_resolve_transaction_currencies_picks_most_frequent_currency_split() {
        let checking = make_test_account("USD", Some("CURRENCY"), Some(100));
        let savings = make_test_account("USD", Some("CURRENCY"), Some(100));
        let fee_acc = make_test_account("EUR", Some("CURRENCY"), Some(100));
        let fund_acc = make_test_account("INF209K01VA3", Some("FUND"), Some(1000));

        let accounts = vec![checking.clone(), savings.clone(), fee_acc.clone(), fund_acc.clone()];
        let (meta, _) = resolve_currency_metadata(&accounts);

        let mut acc_map = HashMap::new();
        acc_map.insert(checking.id, checking.currency.clone());
        acc_map.insert(savings.id, savings.currency.clone());
        acc_map.insert(fee_acc.id, fee_acc.currency.clone());
        acc_map.insert(fund_acc.id, fund_acc.currency.clone());

        let tx_id = Uuid::new_v4();
        let splits = vec![
            make_test_split(tx_id, checking.id, Decimal::new(100, 0), None),
            make_test_split(tx_id, savings.id, Decimal::new(-120, 0), None),
            make_test_split(tx_id, fee_acc.id, Decimal::new(20, 0), None),
            make_test_split(tx_id, fund_acc.id, Decimal::new(0, 0), None),
        ];

        let tx_curr_map = resolve_transaction_currencies(&splits, &acc_map, &meta);
        // USD appears twice, EUR appears once, FUND is ignored -> USD is chosen
        assert_eq!(tx_curr_map.get(&tx_id), Some(&"USD".to_string()));
    }

    #[test]
    fn test_resolve_transaction_currencies_pure_fund_splits_omitted_from_map() {
        let fund_account1 = make_test_account("FUNDA", Some("FUND"), Some(1000));
        let fund_account2 = make_test_account("FUNDB", Some("FUND"), Some(1000));

        let accounts = vec![fund_account1.clone(), fund_account2.clone()];
        let (meta, _) = resolve_currency_metadata(&accounts);

        let mut acc_map = HashMap::new();
        acc_map.insert(fund_account1.id, fund_account1.currency.clone());
        acc_map.insert(fund_account2.id, fund_account2.currency.clone());

        let tx_id = Uuid::new_v4();
        let splits = vec![
            make_test_split(tx_id, fund_account1.id, Decimal::new(100, 0), None),
            make_test_split(tx_id, fund_account2.id, Decimal::new(-100, 0), None),
        ];

        let tx_curr_map = resolve_transaction_currencies(&splits, &acc_map, &meta);
        // No currency splits -> tx_id should NOT be present in tx_curr_map (falls back to book currency)
        assert_eq!(tx_curr_map.get(&tx_id), None);
    }
}
