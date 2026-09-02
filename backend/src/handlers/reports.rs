use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use chrono::{Datelike, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::Deserialize;
use serde_json::json;
use sqlx::PgPool;
use std::collections::HashMap;
use uuid::Uuid;

use crate::models::{
    calculate_display_balance, is_asset_type, is_equity_type, is_expense_type, is_income_type,
    is_liability_type, AccountRow, BalanceSheetItem, BalanceSheetReport, BalanceSheetSection,
    IncomeStatementItem, IncomeStatementReport, IncomeStatementSection, TrialBalanceItem,
    TrialBalanceReport,
};

#[derive(Debug, Deserialize)]
pub struct DateQuery {
    pub date: Option<NaiveDate>,
}

#[derive(Debug, Deserialize)]
pub struct DateRangeQuery {
    pub from_date: Option<NaiveDate>,
    pub to_date: Option<NaiveDate>,
}

pub async fn get_balance_sheet(
    State(pool): State<PgPool>,
    Query(query): Query<DateQuery>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let as_of_date = query.date.unwrap_or_else(|| Utc::now().date_naive());

    // 1. Fetch all accounts
    let accounts: Vec<AccountRow> = sqlx::query_as(
        "SELECT id, name, code, account_type, parent_id, currency, description, placeholder, hidden, created_at, updated_at
         FROM accounts WHERE hidden = FALSE
         ORDER BY code ASC NULLS LAST, name ASC",
    )
    .fetch_all(&pool)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Database error: {}", e) })),
        )
    })?;

    // 2. Fetch sum of amounts per account up to as_of_date
    let balances: Vec<(Uuid, Decimal)> = sqlx::query_as(
        r#"SELECT s.account_id, COALESCE(SUM(s.amount), 0)
           FROM splits s
           JOIN transactions t ON s.transaction_id = t.id
           WHERE t.post_date <= $1
           GROUP BY s.account_id"#
    )
    .bind(as_of_date)
    .fetch_all(&pool)
    .await
    .unwrap_or_default();

    let balance_map: HashMap<Uuid, Decimal> = balances.into_iter().collect();

    // 3. Build parent map
    let mut parent_to_children: HashMap<Option<Uuid>, Vec<Uuid>> = HashMap::new();
    let mut account_map: HashMap<Uuid, &AccountRow> = HashMap::new();
    for acc in &accounts {
        account_map.insert(acc.id, acc);
        parent_to_children.entry(acc.parent_id).or_default().push(acc.id);
    }

    // Helper for recursive balance
    fn get_total_balance(
        id: Uuid,
        balance_map: &HashMap<Uuid, Decimal>,
        parent_to_children: &HashMap<Option<Uuid>, Vec<Uuid>>,
    ) -> Decimal {
        let mut sum = balance_map.get(&id).copied().unwrap_or(Decimal::ZERO);
        if let Some(children) = parent_to_children.get(&Some(id)) {
            for child in children {
                sum += get_total_balance(*child, balance_map, parent_to_children);
            }
        }
        sum
    }

    // Classify into sections
    let mut asset_items = Vec::new();
    let mut liability_items = Vec::new();
    let mut equity_items = Vec::new();
    let mut total_income_raw = Decimal::ZERO;
    let mut total_expense_raw = Decimal::ZERO;

    for acc in &accounts {
        let raw_direct = balance_map.get(&acc.id).copied().unwrap_or(Decimal::ZERO);
        if is_income_type(&acc.account_type) {
            total_income_raw += raw_direct;
        } else if is_expense_type(&acc.account_type) {
            total_expense_raw += raw_direct;
        }
    }

    // Net income = Income (credit, negative) + Expense (debit, positive) in raw terms
    // Standard display net income = Total Revenue - Total Expense
    // In raw terms: Revenue is credit (negative), Expense is debit (positive).
    // Display Revenue = -total_income_raw. Display Expense = total_expense_raw.
    // Display Net Income = (-total_income_raw) - total_expense_raw.
    let net_income = (-total_income_raw) - total_expense_raw;

    // Helper to walk tree for a section
    fn collect_section_items(
        parent_id: Option<Uuid>,
        depth: usize,
        type_predicate: fn(&str) -> bool,
        accounts: &[AccountRow],
        balance_map: &HashMap<Uuid, Decimal>,
        parent_to_children: &HashMap<Option<Uuid>, Vec<Uuid>>,
        items: &mut Vec<BalanceSheetItem>,
    ) {
        if let Some(children) = parent_to_children.get(&parent_id) {
            for child_id in children {
                if let Some(acc) = accounts.iter().find(|a| a.id == *child_id) {
                    if type_predicate(&acc.account_type) {
                        let total = get_total_balance(acc.id, balance_map, parent_to_children);
                        let display = calculate_display_balance(&acc.account_type, total);

                        // Only show if non-zero or has children or is a main category
                        let has_children = parent_to_children.contains_key(&Some(acc.id));
                        if display != Decimal::ZERO || has_children {
                            items.push(BalanceSheetItem {
                                account_id: acc.id,
                                name: acc.name.clone(),
                                code: acc.code.clone(),
                                account_type: acc.account_type.clone(),
                                depth,
                                balance: display,
                            });
                        }
                        collect_section_items(Some(acc.id), depth + 1, type_predicate, accounts, balance_map, parent_to_children, items);
                    }
                }
            }
        }
    }

    collect_section_items(None, 0, is_asset_type, &accounts, &balance_map, &parent_to_children, &mut asset_items);
    collect_section_items(None, 0, is_liability_type, &accounts, &balance_map, &parent_to_children, &mut liability_items);
    collect_section_items(None, 0, is_equity_type, &accounts, &balance_map, &parent_to_children, &mut equity_items);

    // Compute top-level section totals (from root accounts of each type)
    let total_assets: Decimal = accounts
        .iter()
        .filter(|a| a.parent_id.is_none() && is_asset_type(&a.account_type))
        .map(|a| calculate_display_balance(&a.account_type, get_total_balance(a.id, &balance_map, &parent_to_children)))
        .sum();

    let total_liabilities: Decimal = accounts
        .iter()
        .filter(|a| a.parent_id.is_none() && is_liability_type(&a.account_type))
        .map(|a| calculate_display_balance(&a.account_type, get_total_balance(a.id, &balance_map, &parent_to_children)))
        .sum();

    let total_equity_base: Decimal = accounts
        .iter()
        .filter(|a| a.parent_id.is_none() && is_equity_type(&a.account_type))
        .map(|a| calculate_display_balance(&a.account_type, get_total_balance(a.id, &balance_map, &parent_to_children)))
        .sum();

    let total_equity = total_equity_base + net_income;
    let total_liabilities_and_equity = total_liabilities + total_equity;
    let is_balanced = (total_assets - total_liabilities_and_equity).abs() < Decimal::new(1, 2);

    let report = BalanceSheetReport {
        as_of_date,
        assets: BalanceSheetSection {
            title: "Assets".to_string(),
            items: asset_items,
            total: total_assets,
        },
        liabilities: BalanceSheetSection {
            title: "Liabilities".to_string(),
            items: liability_items,
            total: total_liabilities,
        },
        equity: BalanceSheetSection {
            title: "Equity".to_string(),
            items: equity_items,
            total: total_equity,
        },
        net_income,
        total_liabilities_and_equity,
        is_balanced,
    };

    Ok(Json(report))
}

pub async fn get_income_statement(
    State(pool): State<PgPool>,
    Query(query): Query<DateRangeQuery>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let now = Utc::now().date_naive();
    let from_date = query
        .from_date
        .unwrap_or_else(|| NaiveDate::from_ymd_opt(now.year(), 1, 1).unwrap());
    let to_date = query.to_date.unwrap_or(now);

    let accounts: Vec<AccountRow> = sqlx::query_as(
        "SELECT id, name, code, account_type, parent_id, currency, description, placeholder, hidden, created_at, updated_at
         FROM accounts WHERE hidden = FALSE
         ORDER BY code ASC NULLS LAST, name ASC",
    )
    .fetch_all(&pool)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Database error: {}", e) })),
        )
    })?;

    // Sum splits within date range
    let balances: Vec<(Uuid, Decimal)> = sqlx::query_as(
        r#"SELECT s.account_id, COALESCE(SUM(s.amount), 0)
           FROM splits s
           JOIN transactions t ON s.transaction_id = t.id
           WHERE t.post_date >= $1 AND t.post_date <= $2
           GROUP BY s.account_id"#
    )
    .bind(from_date)
    .bind(to_date)
    .fetch_all(&pool)
    .await
    .unwrap_or_default();

    let balance_map: HashMap<Uuid, Decimal> = balances.into_iter().collect();

    let mut parent_to_children: HashMap<Option<Uuid>, Vec<Uuid>> = HashMap::new();
    for acc in &accounts {
        parent_to_children.entry(acc.parent_id).or_default().push(acc.id);
    }

    fn get_total_balance(
        id: Uuid,
        balance_map: &HashMap<Uuid, Decimal>,
        parent_to_children: &HashMap<Option<Uuid>, Vec<Uuid>>,
    ) -> Decimal {
        let mut sum = balance_map.get(&id).copied().unwrap_or(Decimal::ZERO);
        if let Some(children) = parent_to_children.get(&Some(id)) {
            for child in children {
                sum += get_total_balance(*child, balance_map, parent_to_children);
            }
        }
        sum
    }

    let mut revenue_items = Vec::new();
    let mut expense_items = Vec::new();

    fn collect_pnl_items(
        parent_id: Option<Uuid>,
        depth: usize,
        type_predicate: fn(&str) -> bool,
        accounts: &[AccountRow],
        balance_map: &HashMap<Uuid, Decimal>,
        parent_to_children: &HashMap<Option<Uuid>, Vec<Uuid>>,
        items: &mut Vec<IncomeStatementItem>,
    ) {
        if let Some(children) = parent_to_children.get(&parent_id) {
            for child_id in children {
                if let Some(acc) = accounts.iter().find(|a| a.id == *child_id) {
                    if type_predicate(&acc.account_type) {
                        let total = get_total_balance(acc.id, balance_map, parent_to_children);
                        let display = calculate_display_balance(&acc.account_type, total);
                        let has_children = parent_to_children.contains_key(&Some(acc.id));

                        if display != Decimal::ZERO || has_children {
                            items.push(IncomeStatementItem {
                                account_id: acc.id,
                                name: acc.name.clone(),
                                code: acc.code.clone(),
                                account_type: acc.account_type.clone(),
                                depth,
                                amount: display,
                            });
                        }
                        collect_pnl_items(Some(acc.id), depth + 1, type_predicate, accounts, balance_map, parent_to_children, items);
                    }
                }
            }
        }
    }

    collect_pnl_items(None, 0, is_income_type, &accounts, &balance_map, &parent_to_children, &mut revenue_items);
    collect_pnl_items(None, 0, is_expense_type, &accounts, &balance_map, &parent_to_children, &mut expense_items);

    let total_revenue: Decimal = accounts
        .iter()
        .filter(|a| a.parent_id.is_none() && is_income_type(&a.account_type))
        .map(|a| calculate_display_balance(&a.account_type, get_total_balance(a.id, &balance_map, &parent_to_children)))
        .sum();

    let total_expenses: Decimal = accounts
        .iter()
        .filter(|a| a.parent_id.is_none() && is_expense_type(&a.account_type))
        .map(|a| calculate_display_balance(&a.account_type, get_total_balance(a.id, &balance_map, &parent_to_children)))
        .sum();

    let net_income = total_revenue - total_expenses;

    let report = IncomeStatementReport {
        from_date,
        to_date,
        revenues: IncomeStatementSection {
            title: "Revenues & Income".to_string(),
            items: revenue_items,
            total: total_revenue,
        },
        expenses: IncomeStatementSection {
            title: "Expenses".to_string(),
            items: expense_items,
            total: total_expenses,
        },
        net_income,
    };

    Ok(Json(report))
}

pub async fn get_trial_balance(
    State(pool): State<PgPool>,
    Query(query): Query<DateQuery>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let as_of_date = query.date.unwrap_or_else(|| Utc::now().date_naive());

    let rows = sqlx::query_as::<_, (Uuid, Option<String>, String, String, Decimal)>(
        r#"SELECT a.id, a.code, a.name, a.account_type, COALESCE(SUM(s.amount), 0) as balance
           FROM accounts a
           LEFT JOIN splits s ON a.id = s.account_id
           LEFT JOIN transactions t ON s.transaction_id = t.id AND t.post_date <= $1
           WHERE a.placeholder = FALSE AND a.hidden = FALSE
           GROUP BY a.id, a.code, a.name, a.account_type
           HAVING COALESCE(SUM(s.amount), 0) != 0
           ORDER BY a.code ASC NULLS LAST, a.name ASC"#
    )
    .bind(as_of_date)
    .fetch_all(&pool)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Database error: {}", e) })),
        )
    })?;

    let mut items = Vec::new();
    let mut total_debit = Decimal::ZERO;
    let mut total_credit = Decimal::ZERO;

    for (id, code, name, account_type, balance) in rows {
        let (debit, credit) = if balance > Decimal::ZERO {
            (balance, Decimal::ZERO)
        } else {
            (Decimal::ZERO, -balance)
        };

        total_debit += debit;
        total_credit += credit;

        items.push(TrialBalanceItem {
            account_id: id,
            code,
            name,
            account_type,
            debit,
            credit,
        });
    }

    let is_balanced = (total_debit - total_credit).abs() < Decimal::new(1, 2);

    let report = TrialBalanceReport {
        as_of_date,
        items,
        total_debit,
        total_credit,
        is_balanced,
    };

    Ok(Json(report))
}
