use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::Deserialize;
use uuid::Uuid;

#[derive(Debug, Deserialize)]
pub struct ImportQuery {
    pub wipe: Option<bool>,
    pub default_currency: Option<String>,
}

#[derive(Debug, Clone, Default)]
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

#[derive(Debug, Clone, Default)]
pub struct RawTransaction {
    pub guid: String,
    pub post_date: NaiveDate,
    pub num: Option<String>,
    pub description: String,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Default)]
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

#[derive(Debug, Clone, Default)]
pub struct CommInfo {
    pub mnemonic: String,
    pub namespace: String,
    pub fraction: i64,
}

pub type ParsedGnuCashData = (Vec<RawAccount>, Vec<RawTransaction>, Vec<RawSplit>);
pub type ParsedGnuCashFile = (Vec<RawAccount>, Vec<RawTransaction>, Vec<RawSplit>, &'static str);

pub fn parse_gnucash_guid_for_user(guid_str: &str, user_id: &Uuid) -> Uuid {
    let clean = guid_str.trim().replace('-', "");
    // User-scoped namespace prevents any cross-tenant ID collision or takeover
    Uuid::new_v5(user_id, clean.as_bytes())
}
