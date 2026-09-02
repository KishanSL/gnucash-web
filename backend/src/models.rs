use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct AccountRow {
    pub id: Uuid,
    pub name: String,
    pub code: Option<String>,
    pub account_type: String,
    pub parent_id: Option<Uuid>,
    pub currency: String,
    pub description: Option<String>,
    pub placeholder: bool,
    pub hidden: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    pub id: Uuid,
    pub name: String,
    pub code: Option<String>,
    pub account_type: String,
    pub parent_id: Option<Uuid>,
    pub currency: String,
    pub description: Option<String>,
    pub placeholder: bool,
    pub hidden: bool,
    pub balance: Decimal,        // direct balance of this account
    pub total_balance: Decimal,  // includes all children recursively
    pub display_balance: Decimal,// formatted accounting balance according to account normal balance
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountTreeNode {
    pub account: Account,
    pub children: Vec<AccountTreeNode>,
}

#[derive(Debug, Deserialize)]
pub struct CreateAccountRequest {
    pub name: String,
    pub code: Option<String>,
    pub account_type: String,
    pub parent_id: Option<Uuid>,
    pub currency: Option<String>,
    pub description: Option<String>,
    pub placeholder: Option<bool>,
    pub hidden: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateAccountRequest {
    pub name: Option<String>,
    pub code: Option<String>,
    pub account_type: Option<String>,
    pub parent_id: Option<Uuid>,
    pub currency: Option<String>,
    pub description: Option<String>,
    pub placeholder: Option<bool>,
    pub hidden: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct TransactionRow {
    pub id: Uuid,
    pub post_date: NaiveDate,
    pub enter_date: DateTime<Utc>,
    pub num: Option<String>,
    pub description: String,
    pub notes: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct SplitRow {
    pub id: Uuid,
    pub transaction_id: Uuid,
    pub account_id: Uuid,
    pub amount: Decimal,
    pub memo: Option<String>,
    pub reconciled_state: String,
    pub reconciled_date: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SplitWithAccount {
    pub id: Uuid,
    pub transaction_id: Uuid,
    pub account_id: Uuid,
    pub account_name: String,
    pub account_type: String,
    pub amount: Decimal,
    pub memo: Option<String>,
    pub reconciled_state: String,
    pub reconciled_date: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionWithSplits {
    pub id: Uuid,
    pub post_date: NaiveDate,
    pub enter_date: DateTime<Utc>,
    pub num: Option<String>,
    pub description: String,
    pub notes: Option<String>,
    pub splits: Vec<SplitWithAccount>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateSplitRequest {
    pub account_id: Uuid,
    pub amount: Decimal, // Positive = Debit, Negative = Credit
    pub memo: Option<String>,
    pub reconciled_state: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateTransactionRequest {
    pub post_date: NaiveDate,
    pub num: Option<String>,
    pub description: String,
    pub notes: Option<String>,
    pub splits: Vec<CreateSplitRequest>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateTransactionRequest {
    pub post_date: Option<NaiveDate>,
    pub num: Option<String>,
    pub description: Option<String>,
    pub notes: Option<String>,
    pub splits: Option<Vec<CreateSplitRequest>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterEntry {
    pub transaction_id: Uuid,
    pub split_id: Uuid,
    pub post_date: NaiveDate,
    pub num: Option<String>,
    pub description: String,
    pub transfer_account_name: String,
    pub memo: Option<String>,
    pub reconciled_state: String,
    pub amount: Decimal,
    pub debit: Option<Decimal>,
    pub credit: Option<Decimal>,
    pub running_balance: Decimal,
    pub all_splits: Vec<SplitWithAccount>,
}

// Reconciliation models
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReconcileData {
    pub account: Account,
    pub statement_date: NaiveDate,
    pub starting_balance: Decimal,
    pub ending_balance: Decimal,
    pub deposits: Vec<ReconcileSplitItem>,
    pub withdrawals: Vec<ReconcileSplitItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReconcileSplitItem {
    pub split_id: Uuid,
    pub transaction_id: Uuid,
    pub post_date: NaiveDate,
    pub num: Option<String>,
    pub description: String,
    pub amount: Decimal,
    pub display_amount: Decimal,
    pub reconciled_state: String,
}

#[derive(Debug, Deserialize)]
pub struct FinishReconcileRequest {
    pub statement_date: NaiveDate,
    pub reconciled_split_ids: Vec<Uuid>,
}

// Reports
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BalanceSheetItem {
    pub account_id: Uuid,
    pub name: String,
    pub code: Option<String>,
    pub account_type: String,
    pub depth: usize,
    pub balance: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BalanceSheetSection {
    pub title: String,
    pub items: Vec<BalanceSheetItem>,
    pub total: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BalanceSheetReport {
    pub as_of_date: NaiveDate,
    pub assets: BalanceSheetSection,
    pub liabilities: BalanceSheetSection,
    pub equity: BalanceSheetSection,
    pub net_income: Decimal,
    pub total_liabilities_and_equity: Decimal,
    pub is_balanced: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncomeStatementItem {
    pub account_id: Uuid,
    pub name: String,
    pub code: Option<String>,
    pub account_type: String,
    pub depth: usize,
    pub amount: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncomeStatementSection {
    pub title: String,
    pub items: Vec<IncomeStatementItem>,
    pub total: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncomeStatementReport {
    pub from_date: NaiveDate,
    pub to_date: NaiveDate,
    pub revenues: IncomeStatementSection,
    pub expenses: IncomeStatementSection,
    pub net_income: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrialBalanceItem {
    pub account_id: Uuid,
    pub code: Option<String>,
    pub name: String,
    pub account_type: String,
    pub debit: Decimal,
    pub credit: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrialBalanceReport {
    pub as_of_date: NaiveDate,
    pub items: Vec<TrialBalanceItem>,
    pub total_debit: Decimal,
    pub total_credit: Decimal,
    pub is_balanced: bool,
}

// Helper functions for accounting normal balances
pub fn is_asset_type(t: &str) -> bool {
    matches!(t.to_lowercase().as_str(), "asset" | "bank" | "cash" | "receivable")
}

pub fn is_liability_type(t: &str) -> bool {
    matches!(t.to_lowercase().as_str(), "liability" | "credit" | "payable")
}

pub fn is_equity_type(t: &str) -> bool {
    matches!(t.to_lowercase().as_str(), "equity")
}

pub fn is_income_type(t: &str) -> bool {
    matches!(t.to_lowercase().as_str(), "income")
}

pub fn is_expense_type(t: &str) -> bool {
    matches!(t.to_lowercase().as_str(), "expense")
}

/// Normal balance:
/// Assets & Expenses: normal balance is Debit (positive raw sum).
/// Liabilities, Equity, Income: normal balance is Credit (negative raw sum in standard double-entry,
/// but displayed positive in financial reports).
pub fn calculate_display_balance(account_type: &str, raw_balance: Decimal) -> Decimal {
    if is_asset_type(account_type) || is_expense_type(account_type) {
        raw_balance
    } else {
        -raw_balance
    }
}
