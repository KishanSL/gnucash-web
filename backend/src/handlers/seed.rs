use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use chrono::NaiveDate;
use rust_decimal_macros::dec;
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

use crate::auth::AuthUser;
use crate::handlers::internal_error;

pub async fn seed_database(
    auth: AuthUser,
    State(pool): State<PgPool>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let mut tx = pool.begin().await.map_err(|e| internal_error("Database error starting seed", e))?;

    // Clear existing user transactions & accounts before seeding
    sqlx::query("DELETE FROM transactions WHERE user_id = $1")
        .bind(auth.id)
        .execute(&mut *tx)
        .await
        .map_err(|e| internal_error("Failed to clear transactions", e))?;

    sqlx::query("DELETE FROM accounts WHERE user_id = $1")
        .bind(auth.id)
        .execute(&mut *tx)
        .await
        .map_err(|e| internal_error("Failed to clear accounts", e))?;

    if let Err(e) = run_seed_tx(&mut tx, auth.id).await {
        let _ = tx.rollback().await;
        return Err(internal_error("Failed to seed database", e));
    }

    tx.commit().await.map_err(|e| internal_error("Failed to commit seed transaction", e))?;

    Ok(Json(json!({
        "message": "Chart of accounts and sample transactions successfully seeded!"
    })))
}

pub async fn reset_dataset(
    auth: AuthUser,
    State(pool): State<PgPool>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let mut tx = pool.begin().await.map_err(|e| internal_error("Database transaction error starting reset", e))?;

    // 1. Delete all transactions for this user (cascades to splits)
    sqlx::query("DELETE FROM transactions WHERE user_id = $1")
        .bind(auth.id)
        .execute(&mut *tx)
        .await
        .map_err(|e| internal_error("Failed to delete transactions", e))?;

    // 2. Delete all accounts for this user
    sqlx::query("DELETE FROM accounts WHERE user_id = $1")
        .bind(auth.id)
        .execute(&mut *tx)
        .await
        .map_err(|e| internal_error("Failed to delete accounts", e))?;

    tx.commit().await.map_err(|e| internal_error("Failed to commit database reset", e))?;

    Ok(Json(json!({
        "success": true,
        "message": "Entire database reset successfully. All accounts and transactions have been deleted."
    })))
}

pub async fn run_seed(pool: &PgPool, user_id: Uuid) -> Result<(), sqlx::Error> {
    let mut tx = pool.begin().await?;
    run_seed_tx(&mut tx, user_id).await?;
    tx.commit().await?;
    Ok(())
}

pub async fn run_seed_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    user_id: Uuid,
) -> Result<(), sqlx::Error> {
    #[cfg(debug_assertions)]
    if let Ok(fail_user) = std::env::var("FAIL_SEED_TEST_USER_ID") {
        if fail_user == user_id.to_string() || fail_user == "all" || fail_user == "1" {
            return Err(sqlx::Error::RowNotFound);
        }
    }

    // Helper to insert account
    async fn insert_acc(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        name: &str,
        code: Option<&str>,
        account_type: &str,
        parent_id: Option<Uuid>,
        placeholder: bool,
        user_id: Uuid,
    ) -> Result<Uuid, sqlx::Error> {
        let row: (Uuid,) = sqlx::query_as(
            r#"INSERT INTO accounts (name, code, account_type, parent_id, currency, placeholder, user_id)
               VALUES ($1, $2, $3, $4, 'INR', $5, $6)
               RETURNING id"#
        )
        .bind(name)
        .bind(code)
        .bind(account_type)
        .bind(parent_id)
        .bind(placeholder)
        .bind(user_id)
        .fetch_one(&mut **tx)
        .await?;
        Ok(row.0)
    }

    // 1. Root Accounts
    let assets_root = insert_acc(&mut *tx, "Assets", Some("1000"), "asset", None, true, user_id).await?;
    let liab_root = insert_acc(&mut *tx, "Liabilities", Some("2000"), "liability", None, true, user_id).await?;
    let equity_root = insert_acc(&mut *tx, "Equity", Some("3000"), "equity", None, true, user_id).await?;
    let income_root = insert_acc(&mut *tx, "Income", Some("4000"), "income", None, true, user_id).await?;
    let expense_root = insert_acc(&mut *tx, "Expenses", Some("5000"), "expense", None, true, user_id).await?;

    // 2. Assets Subaccounts
    let curr_assets = insert_acc(&mut *tx, "Current Assets", Some("1100"), "asset", Some(assets_root), true, user_id).await?;
    let checking = insert_acc(&mut *tx, "Checking Account", Some("1110"), "bank", Some(curr_assets), false, user_id).await?;
    let savings = insert_acc(&mut *tx, "Savings Account", Some("1120"), "bank", Some(curr_assets), false, user_id).await?;
    let cash = insert_acc(&mut *tx, "Cash in Wallet", Some("1130"), "cash", Some(curr_assets), false, user_id).await?;
    let _receivable = insert_acc(&mut *tx, "Accounts Receivable", Some("1140"), "receivable", Some(curr_assets), false, user_id).await?;

    let fixed_assets = insert_acc(&mut *tx, "Fixed Assets", Some("1500"), "asset", Some(assets_root), true, user_id).await?;
    let _computer = insert_acc(&mut *tx, "Computer & Tech", Some("1510"), "asset", Some(fixed_assets), false, user_id).await?;
    let _auto_asset = insert_acc(&mut *tx, "Automobile", Some("1520"), "asset", Some(fixed_assets), false, user_id).await?;

    // 3. Liabilities Subaccounts
    let credit_cards = insert_acc(&mut *tx, "Credit Cards", Some("2100"), "liability", Some(liab_root), true, user_id).await?;
    let visa = insert_acc(&mut *tx, "Visa Signature Card", Some("2110"), "credit", Some(credit_cards), false, user_id).await?;
    let mastercard = insert_acc(&mut *tx, "Mastercard", Some("2120"), "credit", Some(credit_cards), false, user_id).await?;
    let _payable = insert_acc(&mut *tx, "Accounts Payable", Some("2200"), "payable", Some(liab_root), false, user_id).await?;

    // 4. Equity Subaccounts
    let opening_equity = insert_acc(&mut *tx, "Opening Balances", Some("3100"), "equity", Some(equity_root), false, user_id).await?;
    let _retained = insert_acc(&mut *tx, "Retained Earnings", Some("3200"), "equity", Some(equity_root), false, user_id).await?;

    // 5. Income Subaccounts
    let salary = insert_acc(&mut *tx, "Salary & Wages", Some("4100"), "income", Some(income_root), false, user_id).await?;
    let consulting = insert_acc(&mut *tx, "Freelance Consulting", Some("4200"), "income", Some(income_root), false, user_id).await?;
    let interest = insert_acc(&mut *tx, "Interest Income", Some("4300"), "income", Some(income_root), false, user_id).await?;

    // 6. Expenses Subaccounts
    let housing = insert_acc(&mut *tx, "Housing", Some("5100"), "expense", Some(expense_root), true, user_id).await?;
    let rent = insert_acc(&mut *tx, "Rent", Some("5110"), "expense", Some(housing), false, user_id).await?;
    let utilities = insert_acc(&mut *tx, "Utilities (Electric/Water)", Some("5120"), "expense", Some(housing), false, user_id).await?;
    let internet = insert_acc(&mut *tx, "High-Speed Internet", Some("5130"), "expense", Some(housing), false, user_id).await?;

    let food = insert_acc(&mut *tx, "Food & Dining", Some("5200"), "expense", Some(expense_root), true, user_id).await?;
    let groceries = insert_acc(&mut *tx, "Groceries", Some("5210"), "expense", Some(food), false, user_id).await?;
    let dining = insert_acc(&mut *tx, "Restaurants & Dining Out", Some("5220"), "expense", Some(food), false, user_id).await?;

    let transport = insert_acc(&mut *tx, "Transportation", Some("5300"), "expense", Some(expense_root), true, user_id).await?;
    let gas = insert_acc(&mut *tx, "Gas & Fuel", Some("5310"), "expense", Some(transport), false, user_id).await?;
    let _transit = insert_acc(&mut *tx, "Public Transit", Some("5320"), "expense", Some(transport), false, user_id).await?;

    let taxes = insert_acc(&mut *tx, "Taxes", Some("5400"), "expense", Some(expense_root), true, user_id).await?;
    let fed_tax = insert_acc(&mut *tx, "Federal Income Tax", Some("5410"), "expense", Some(taxes), false, user_id).await?;
    let ss_tax = insert_acc(&mut *tx, "Social Security & Medicare", Some("5420"), "expense", Some(taxes), false, user_id).await?;

    let entertainment = insert_acc(&mut *tx, "Entertainment & Media", Some("5500"), "expense", Some(expense_root), false, user_id).await?;
    let healthcare = insert_acc(&mut *tx, "Healthcare & Pharmacy", Some("5600"), "expense", Some(expense_root), false, user_id).await?;

    // Sample Transactions
    // Helper to insert transaction with splits
    struct SplitDef {
        account_id: Uuid,
        amount: rust_decimal::Decimal,
        memo: Option<&'static str>,
        rec_state: &'static str,
    }

    async fn insert_tx(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        user_id: Uuid,
        post_date: NaiveDate,
        num: Option<&str>,
        desc: &str,
        notes: Option<&str>,
        splits: Vec<SplitDef>,
    ) -> Result<(), sqlx::Error> {
        let tx_row: (Uuid,) = sqlx::query_as(
            r#"INSERT INTO transactions (post_date, num, description, notes, user_id)
               VALUES ($1, $2, $3, $4, $5)
               RETURNING id"#
        )
        .bind(post_date)
        .bind(num)
        .bind(desc)
        .bind(notes)
        .bind(user_id)
        .fetch_one(&mut **tx)
        .await?;

        for s in splits {
            let rec_date = if s.rec_state == "y" {
                Some(post_date.and_hms_opt(12, 0, 0).unwrap().and_utc())
            } else {
                None
            };

            sqlx::query(
                r#"INSERT INTO splits (transaction_id, account_id, amount, memo, reconciled_state, reconciled_date)
                   VALUES ($1, $2, $3, $4, $5, $6)"#
            )
            .bind(tx_row.0)
            .bind(s.account_id)
            .bind(s.amount)
            .bind(s.memo)
            .bind(s.rec_state)
            .bind(rec_date)
            .execute(&mut **tx)
            .await?;
        }
        Ok(())
    }

    // Tx 1: Opening Balances (Reconciled 'y')
    insert_tx(
        &mut *tx,
        user_id,
        NaiveDate::from_ymd_opt(2026, 10, 1).unwrap(),
        Some("OPEN"),
        "Opening Balances",
        Some("Initial balance transfer"),
        vec![
            SplitDef { account_id: checking, amount: dec!(5500.00), memo: Some("Checking opening"), rec_state: "y" },
            SplitDef { account_id: savings, amount: dec!(12000.00), memo: Some("Savings opening"), rec_state: "y" },
            SplitDef { account_id: cash, amount: dec!(350.00), memo: Some("Cash in pocket"), rec_state: "y" },
            SplitDef { account_id: opening_equity, amount: dec!(-17850.00), memo: Some("Opening balance equity offset"), rec_state: "n" },
        ],
    ).await?;

    // Tx 2: Bi-weekly Salary 1
    insert_tx(
        &mut *tx,
        user_id,
        NaiveDate::from_ymd_opt(2026, 10, 5).unwrap(),
        Some("DIR-DEP"),
        "Acme Corp Paycheck",
        Some("Bi-weekly payroll with tax deductions"),
        vec![
            SplitDef { account_id: checking, amount: dec!(2550.00), memo: Some("Net pay deposit"), rec_state: "y" },
            SplitDef { account_id: fed_tax, amount: dec!(650.00), memo: Some("Federal withholding"), rec_state: "n" },
            SplitDef { account_id: ss_tax, amount: dec!(300.00), memo: Some("FICA / Medicare"), rec_state: "n" },
            SplitDef { account_id: salary, amount: dec!(-3500.00), memo: Some("Gross salary earnings"), rec_state: "n" },
        ],
    ).await?;

    // Tx 3: Rent
    insert_tx(
        &mut *tx,
        user_id,
        NaiveDate::from_ymd_opt(2026, 10, 6).unwrap(),
        Some("101"),
        "Skyline Property Management",
        Some("Monthly apartment lease"),
        vec![
            SplitDef { account_id: rent, amount: dec!(1400.00), memo: Some("October apartment rent"), rec_state: "n" },
            SplitDef { account_id: checking, amount: dec!(-1400.00), memo: Some("Check #101"), rec_state: "y" },
        ],
    ).await?;

    // Tx 4: Supermarket
    insert_tx(
        &mut *tx,
        user_id,
        NaiveDate::from_ymd_opt(2026, 10, 8).unwrap(),
        None,
        "Whole Foods Market",
        Some("Weekly groceries and pantry goods"),
        vec![
            SplitDef { account_id: groceries, amount: dec!(164.20), memo: Some("Weekly organic groceries"), rec_state: "n" },
            SplitDef { account_id: checking, amount: dec!(-164.20), memo: Some("Debit card POS"), rec_state: "y" },
        ],
    ).await?;

    // Tx 5: Internet on Visa
    insert_tx(
        &mut *tx,
        user_id,
        NaiveDate::from_ymd_opt(2026, 10, 10).unwrap(),
        Some("AUTO"),
        "FiberOptic Gigabit Internet",
        Some("Monthly internet subscription"),
        vec![
            SplitDef { account_id: internet, amount: dec!(79.99), memo: Some("Fiber 1Gbps plan"), rec_state: "n" },
            SplitDef { account_id: visa, amount: dec!(-79.99), memo: Some("Card charge"), rec_state: "c" },
        ],
    ).await?;

    // Tx 6: Dining on Visa
    insert_tx(
        &mut *tx,
        user_id,
        NaiveDate::from_ymd_opt(2026, 10, 12).unwrap(),
        None,
        "The Italian Trattoria",
        Some("Dinner with colleagues"),
        vec![
            SplitDef { account_id: dining, amount: dec!(68.50), memo: Some("Dinner pasta and wine"), rec_state: "n" },
            SplitDef { account_id: visa, amount: dec!(-68.50), memo: Some("Card charge"), rec_state: "c" },
        ],
    ).await?;

    // Tx 7: Gas in Cash
    insert_tx(
        &mut *tx,
        user_id,
        NaiveDate::from_ymd_opt(2026, 10, 14).unwrap(),
        None,
        "Shell Gas Station",
        Some("Fuel for vehicle"),
        vec![
            SplitDef { account_id: gas, amount: dec!(42.00), memo: Some("Regular unleaded 12 gal"), rec_state: "n" },
            SplitDef { account_id: cash, amount: dec!(-42.00), memo: Some("Cash payment"), rec_state: "n" },
        ],
    ).await?;

    // Tx 8: Freelance Consulting
    insert_tx(
        &mut *tx,
        user_id,
        NaiveDate::from_ymd_opt(2026, 10, 16).unwrap(),
        Some("INV-042"),
        "Nexus Software LLC",
        Some("Code review and consulting services"),
        vec![
            SplitDef { account_id: checking, amount: dec!(850.00), memo: Some("ACH wire received"), rec_state: "c" },
            SplitDef { account_id: consulting, amount: dec!(-850.00), memo: Some("Invoice #042 consulting"), rec_state: "n" },
        ],
    ).await?;

    // Tx 9: Utilities Bill
    insert_tx(
        &mut *tx,
        user_id,
        NaiveDate::from_ymd_opt(2026, 10, 18).unwrap(),
        Some("ELEC"),
        "City Power & Light",
        Some("Electricity and water bill"),
        vec![
            SplitDef { account_id: utilities, amount: dec!(92.40), memo: Some("October utility bill"), rec_state: "n" },
            SplitDef { account_id: checking, amount: dec!(-92.40), memo: Some("Bill pay"), rec_state: "c" },
        ],
    ).await?;

    // Tx 10: Bi-weekly Salary 2
    insert_tx(
        &mut *tx,
        user_id,
        NaiveDate::from_ymd_opt(2026, 10, 19).unwrap(),
        Some("DIR-DEP"),
        "Acme Corp Paycheck",
        Some("Second bi-weekly payroll"),
        vec![
            SplitDef { account_id: checking, amount: dec!(2550.00), memo: Some("Net pay deposit"), rec_state: "c" },
            SplitDef { account_id: fed_tax, amount: dec!(650.00), memo: Some("Federal tax"), rec_state: "n" },
            SplitDef { account_id: ss_tax, amount: dec!(300.00), memo: Some("FICA tax"), rec_state: "n" },
            SplitDef { account_id: salary, amount: dec!(-3500.00), memo: Some("Gross salary earnings"), rec_state: "n" },
        ],
    ).await?;

    // Tx 11: Visa Credit Card Payment
    insert_tx(
        &mut *tx,
        user_id,
        NaiveDate::from_ymd_opt(2026, 10, 20).unwrap(),
        Some("CC-PMT"),
        "Visa Card Statement Payment",
        Some("Autopay from checking to credit card"),
        vec![
            SplitDef { account_id: visa, amount: dec!(148.49), memo: Some("Payment received"), rec_state: "c" },
            SplitDef { account_id: checking, amount: dec!(-148.49), memo: Some("Autopay debit"), rec_state: "c" },
        ],
    ).await?;

    // Tx 12: Savings Transfer
    insert_tx(
        &mut *tx,
        user_id,
        NaiveDate::from_ymd_opt(2026, 10, 22).unwrap(),
        Some("XFER"),
        "Emergency Fund Transfer",
        Some("Monthly savings allocation"),
        vec![
            SplitDef { account_id: savings, amount: dec!(500.00), memo: Some("Savings deposit"), rec_state: "n" },
            SplitDef { account_id: checking, amount: dec!(-500.00), memo: Some("Checking transfer out"), rec_state: "c" },
        ],
    ).await?;

    // Tx 13: Streaming subscriptions on Mastercard
    insert_tx(
        &mut *tx,
        user_id,
        NaiveDate::from_ymd_opt(2026, 10, 24).unwrap(),
        None,
        "Netflix & Spotify",
        Some("Monthly digital media streaming"),
        vec![
            SplitDef { account_id: entertainment, amount: dec!(29.98), memo: Some("Streaming services"), rec_state: "n" },
            SplitDef { account_id: mastercard, amount: dec!(-29.98), memo: Some("Card charge"), rec_state: "n" },
        ],
    ).await?;

    // Tx 14: Pharmacy in Cash
    insert_tx(
        &mut *tx,
        user_id,
        NaiveDate::from_ymd_opt(2026, 10, 26).unwrap(),
        None,
        "Walgreens Pharmacy",
        Some("Vitamins and prescription medicine"),
        vec![
            SplitDef { account_id: healthcare, amount: dec!(24.50), memo: Some("Health supplies"), rec_state: "n" },
            SplitDef { account_id: cash, amount: dec!(-24.50), memo: Some("Cash purchase"), rec_state: "n" },
        ],
    ).await?;

    // Tx 15: Interest Income on Savings
    insert_tx(
        &mut *tx,
        user_id,
        NaiveDate::from_ymd_opt(2026, 10, 31).unwrap(),
        Some("INT"),
        "High-Yield Savings Interest",
        Some("Monthly interest compound"),
        vec![
            SplitDef { account_id: savings, amount: dec!(42.15), memo: Some("Interest credited"), rec_state: "n" },
            SplitDef { account_id: interest, amount: dec!(-42.15), memo: Some("Interest earned"), rec_state: "n" },
        ],
    ).await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn get_seed_test_db_pool() -> (String, sqlx::PgPool) {
        let db_url = crate::db::get_test_database_url();
        crate::db::ensure_test_database_exists(&db_url).await;
        let pool = crate::db::init_db(&db_url).await.expect("Failed to initialize test database schema");
        (db_url, pool)
    }

    #[tokio::test]
    async fn test_run_seed_tx_rollback_on_failure() {
        let (_db_url, pool) = get_seed_test_db_pool().await;
        let mut tx = pool.begin().await.expect("begin tx");

        let test_user_id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO users (id, username, email, password_hash) VALUES ($1, $2, $3, 'hash')"
        )
        .bind(test_user_id)
        .bind(format!("seed_test_{}", test_user_id.simple()))
        .bind(format!("seed_test_{}@example.com", test_user_id.simple()))
        .execute(&mut *tx)
        .await
        .expect("create test user in tx");

        run_seed_tx(&mut tx, test_user_id).await.expect("seed tx succeeds");

        let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM accounts WHERE user_id = $1")
            .bind(test_user_id)
            .fetch_one(&mut *tx)
            .await
            .expect("fetch account count");
        assert_eq!(count.0, 37);

        // Rollback transaction to verify nothing persists
        tx.rollback().await.expect("rollback tx");

        let count_after: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM accounts WHERE user_id = $1")
            .bind(test_user_id)
            .fetch_one(&pool)
            .await
            .expect("fetch account count after rollback");
        assert_eq!(count_after.0, 0);
    }

    #[tokio::test]
    async fn test_reconciled_state_constraint_exists() {
        let (_db_url, pool) = get_seed_test_db_pool().await;
        let exists: (bool,) = sqlx::query_as(
            "SELECT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'chk_splits_reconciled_state')"
        )
        .fetch_one(&pool)
        .await
        .expect("check pg_constraint");
        assert!(exists.0, "chk_splits_reconciled_state constraint must exist");
    }

    #[tokio::test]
    async fn test_seed_database_wipe_and_insert_atomic_rollback() {
        let (_db_url, pool) = get_seed_test_db_pool().await;
        let test_user_id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO users (id, username, email, password_hash) VALUES ($1, $2, $3, 'hash')"
        )
        .bind(test_user_id)
        .bind(format!("seed_atomic_{}", test_user_id.simple()))
        .bind(format!("seed_atomic_{}@example.com", test_user_id.simple()))
        .execute(&pool)
        .await
        .expect("create test user");

        // Create initial account for user
        let acc_id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO accounts (id, name, account_type, currency, placeholder, user_id) VALUES ($1, 'Pre-seed Checking', 'bank', 'INR', false, $2)"
        )
        .bind(acc_id)
        .bind(test_user_id)
        .execute(&pool)
        .await
        .expect("create pre-seed account");

        // Simulate seed_database failure: start tx, wipe accounts, error before commit
        let mut tx = pool.begin().await.expect("begin tx");
        sqlx::query("DELETE FROM transactions WHERE user_id = $1")
            .bind(test_user_id)
            .execute(&mut *tx)
            .await
            .expect("wipe txns");
        sqlx::query("DELETE FROM accounts WHERE user_id = $1")
            .bind(test_user_id)
            .execute(&mut *tx)
            .await
            .expect("wipe accounts");

        // Rollback (simulating failed sample insert)
        tx.rollback().await.expect("rollback simulated failure");

        // Verify pre-seed account is STILL present!
        let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM accounts WHERE user_id = $1")
            .bind(test_user_id)
            .fetch_one(&pool)
            .await
            .expect("fetch account count");
        assert_eq!(count.0, 1, "Pre-existing account must be preserved on rollback");

        // Clean up test user
        let _ = sqlx::query("DELETE FROM accounts WHERE user_id = $1").bind(test_user_id).execute(&pool).await;
        let _ = sqlx::query("DELETE FROM users WHERE id = $1").bind(test_user_id).execute(&pool).await;
    }
}
