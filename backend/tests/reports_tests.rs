//! Balance sheet, trial balance and investment balances in reports (RPT-*, INV-*)

mod common;
use common::*;

#[tokio::test]
async fn test_balance_sheet_equation() {
    let pool = get_test_pool().await;
    let app = create_app(pool);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/reports/balance-sheet")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let report: Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(
        report["is_balanced"], true,
        "Balance sheet must satisfy Assets = Liabilities + Equity"
    );
}

#[tokio::test]
async fn test_trial_balance_debits_equal_credits() {
    let pool = get_test_pool().await;
    let app = create_app(pool);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/reports/trial-balance")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let report: Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(
        report["is_balanced"], true,
        "Trial balance total debits must equal total credits"
    );
    assert_eq!(report["total_debit"], report["total_credit"]);
}

#[tokio::test]
async fn test_issue_21_investment_balance_shows_amount_not_units() {
    // Issue #21: Stock and mutual accounts show investment money amount, not units.
    // Parent total_balance rolls up money cleanly without mixing units and currency.
    let pool = get_test_pool().await;
    let user_id = uuid::Uuid::new_v4();
    let username = format!("inv_{}", &user_id.to_string()[..8]);
    let email = format!("{}@example.com", username);
    sqlx::query("INSERT INTO users (id, username, email, password_hash) VALUES ($1, $2, $3, 'hash')")
        .bind(user_id)
        .bind(&username)
        .bind(&email)
        .execute(&pool)
        .await
        .unwrap();
    let user_token = gnucash_backend::auth::create_jwt(user_id, &username, &email, 0).unwrap();

    let temp_db = tempfile::NamedTempFile::new().unwrap();
    let conn = rusqlite::Connection::open(temp_db.path()).unwrap();
    conn.execute_batch(r#"
        CREATE TABLE accounts (guid TEXT PRIMARY KEY, name TEXT, account_type TEXT, commodity_guid TEXT, parent_guid TEXT, code TEXT, description TEXT, hidden INT, placeholder INT);
        CREATE TABLE transactions (guid TEXT PRIMARY KEY, currency_guid TEXT, num TEXT, post_date TEXT, enter_date TEXT, description TEXT);
        CREATE TABLE splits (guid TEXT PRIMARY KEY, tx_guid TEXT, account_guid TEXT, memo TEXT, action TEXT, reconcile_state TEXT, reconcile_date TEXT, value_num INT, value_denom INT, quantity_num INT, quantity_denom INT, lot_guid TEXT);
        CREATE TABLE commodities (guid TEXT PRIMARY KEY, namespace TEXT, mnemonic TEXT, fullname TEXT, cusip TEXT, fraction INT, quote_flag INT, quote_source TEXT);

        INSERT INTO commodities VALUES ('c_inr', 'CURRENCY', 'INR', 'INR', '', 100, 1, 'currency');
        INSERT INTO commodities VALUES ('c_fund', 'FUND', 'MYFUND', 'My Mutual Fund', 'MYFUND', 1000, 1, 'financequote');

        INSERT INTO accounts VALUES ('a_assets', 'Assets', 'ASSET', 'c_inr', NULL, NULL, NULL, 0, 1);
        INSERT INTO accounts VALUES ('a_bank', 'Checking', 'BANK', 'c_inr', 'a_assets', NULL, NULL, 0, 0);
        INSERT INTO accounts VALUES ('a_fund', 'My Fund', 'MUTUAL', 'c_fund', 'a_assets', NULL, NULL, 0, 0);

        INSERT INTO transactions VALUES ('t_buy', 'c_inr', '', '20261101105900', '20261101105900', 'Fund SIP Purchase');
        INSERT INTO splits VALUES ('s_bank', 't_buy', 'a_bank', '', '', 'n', NULL, -150000, 100, -150000, 100, NULL);
        INSERT INTO splits VALUES ('s_fund', 't_buy', 'a_fund', '', '', 'n', NULL, 150000, 100, 10000, 1000, NULL);
    "#).unwrap();

    let bytes = std::fs::read(temp_db.path()).unwrap();
    let boundary = "---------------------------11223344556677889900";
    let mut body = Vec::new();
    body.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
    body.extend_from_slice(b"Content-Disposition: form-data; name=\"file\"; filename=\"investment.gnucash\"\r\nContent-Type: application/x-sqlite3\r\n\r\n");
    body.extend_from_slice(&bytes);
    body.extend_from_slice(format!("\r\n--{}--\r\n", boundary).as_bytes());

    let app = create_app(pool.clone());
    let resp = app.oneshot(
        Request::builder()
            .method("POST")
            .uri("/api/import/gnucash-sqlite?wipe=true")
            .header("authorization", format!("Bearer {}", user_token))
            .header("content-type", format!("multipart/form-data; boundary={}", boundary))
            .body(Body::from(body))
            .unwrap()
    ).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // 1. GET /api/accounts: My Fund balance = 1500.00, Assets total_balance = 0.00
    let app = create_app(pool.clone());
    let resp = app.oneshot(
        Request::builder()
            .method("GET")
            .uri("/api/accounts")
            .header("authorization", format!("Bearer {}", user_token))
            .body(Body::empty())
            .unwrap()
    ).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let accs: Vec<serde_json::Value> = serde_json::from_slice(&body).unwrap();
    let fund_acc = accs.iter().find(|a| a["name"] == "My Fund").unwrap();
    let bank_acc = accs.iter().find(|a| a["name"] == "Checking").unwrap();
    let assets_acc = accs.iter().find(|a| a["name"] == "Assets").unwrap();

    let fund_bal: rust_decimal::Decimal = fund_acc["balance"].as_str().unwrap().parse().unwrap();
    let bank_bal: rust_decimal::Decimal = bank_acc["balance"].as_str().unwrap().parse().unwrap();
    let assets_tot: rust_decimal::Decimal = assets_acc["total_balance"].as_str().unwrap().parse().unwrap();

    assert_eq!(fund_bal, rust_decimal::Decimal::from(1500), "My Fund balance must be investment money amount 1500, not units (10)");
    assert_eq!(bank_bal, rust_decimal::Decimal::from(-1500), "Checking balance must be -1500");
    assert_eq!(assets_tot, rust_decimal::Decimal::ZERO, "Assets total balance must roll up to 0, not mix units and money (-1490)");

    // 2. GET /api/accounts/tree: verify Assets tree node total_balance = 0
    let app = create_app(pool.clone());
    let resp = app.oneshot(
        Request::builder()
            .method("GET")
            .uri("/api/accounts/tree")
            .header("authorization", format!("Bearer {}", user_token))
            .body(Body::empty())
            .unwrap()
    ).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let tree: Vec<serde_json::Value> = serde_json::from_slice(&body).unwrap();
    let assets_node = tree.iter().find(|n| n["account"]["name"] == "Assets").unwrap();
    let assets_node_tot: rust_decimal::Decimal = assets_node["account"]["total_balance"].as_str().unwrap().parse().unwrap();
    assert_eq!(assets_node_tot, rust_decimal::Decimal::ZERO, "Assets tree node total_balance must roll up to 0");

    // 3. GET /api/reports/balance-sheet: verify balance sheet agrees (is_balanced = true)
    let app = create_app(pool.clone());
    let resp = app.oneshot(
        Request::builder()
            .method("GET")
            .uri("/api/reports/balance-sheet")
            .header("authorization", format!("Bearer {}", user_token))
            .body(Body::empty())
            .unwrap()
    ).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let bs: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(bs["is_balanced"], true, "Balance sheet must be balanced");
    let total_assets: rust_decimal::Decimal = serde_json::from_value(bs["assets"]["total"].clone()).unwrap();
    assert_eq!(total_assets, rust_decimal::Decimal::ZERO, "Total assets on balance sheet must equal 0.00");
}

#[tokio::test]
async fn test_issue_24_balance_sheet_mixed_hierarchy() {
    let pool = get_test_pool().await;
    let user_id = uuid::Uuid::new_v4();
    let username = format!("user_bs_mixed_{}", user_id.simple());
    let email = format!("user_bs_mixed_{}@example.com", user_id.simple());

    sqlx::query(
        "INSERT INTO users (id, username, email, password_hash, token_version) VALUES ($1, $2, $3, 'hash', 0)"
    )
    .bind(user_id)
    .bind(&username)
    .bind(&email)
    .execute(&pool)
    .await
    .unwrap();

    let token = gnucash_backend::auth::create_jwt(user_id, &username, &email, 0).unwrap();

    // Create mixed tree: Assets placeholder -> Checking (bank), Fuel (expense nested under asset)
    let assets_id = uuid::Uuid::new_v4();
    let checking_id = uuid::Uuid::new_v4();
    let fuel_id = uuid::Uuid::new_v4();
    let equity_id = uuid::Uuid::new_v4();
    let opening_equity_id = uuid::Uuid::new_v4();
    let salary_id = uuid::Uuid::new_v4();

    sqlx::query("INSERT INTO accounts (id, name, account_type, parent_id, currency, placeholder, user_id) VALUES ($1, 'Assets', 'asset', NULL, 'INR', true, $2)")
        .bind(assets_id).bind(user_id).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO accounts (id, name, account_type, parent_id, currency, placeholder, user_id) VALUES ($1, 'Checking', 'bank', $2, 'INR', false, $3)")
        .bind(checking_id).bind(assets_id).bind(user_id).execute(&pool).await.unwrap();
    // Fuel is an EXPENSE account nested under Assets
    sqlx::query("INSERT INTO accounts (id, name, account_type, parent_id, currency, placeholder, user_id) VALUES ($1, 'Fuel', 'expense', $2, 'INR', false, $3)")
        .bind(fuel_id).bind(assets_id).bind(user_id).execute(&pool).await.unwrap();

    sqlx::query("INSERT INTO accounts (id, name, account_type, parent_id, currency, placeholder, user_id) VALUES ($1, 'Equity', 'equity', NULL, 'INR', true, $2)")
        .bind(equity_id).bind(user_id).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO accounts (id, name, account_type, parent_id, currency, placeholder, user_id) VALUES ($1, 'Opening Balances', 'equity', $2, 'INR', false, $3)")
        .bind(opening_equity_id).bind(equity_id).bind(user_id).execute(&pool).await.unwrap();

    sqlx::query("INSERT INTO accounts (id, name, account_type, parent_id, currency, placeholder, user_id) VALUES ($1, 'Salary', 'income', NULL, 'INR', false, $2)")
        .bind(salary_id).bind(user_id).execute(&pool).await.unwrap();

    // Tx 1: Opening balance ₹10,000 to checking
    let tx1_id = uuid::Uuid::new_v4();
    sqlx::query("INSERT INTO transactions (id, user_id, post_date, description) VALUES ($1, $2, '2026-11-01', 'Opening')")
        .bind(tx1_id).bind(user_id).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO splits (id, transaction_id, account_id, amount, quantity, reconciled_state) VALUES ($1, $2, $3, 10000.00, 10000.00, 'n')")
        .bind(uuid::Uuid::new_v4()).bind(tx1_id).bind(checking_id).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO splits (id, transaction_id, account_id, amount, quantity, reconciled_state) VALUES ($1, $2, $3, -10000.00, -10000.00, 'n')")
        .bind(uuid::Uuid::new_v4()).bind(tx1_id).bind(opening_equity_id).execute(&pool).await.unwrap();

    // Tx 2: Checking to Fuel (₹1,500)
    let tx2_id = uuid::Uuid::new_v4();
    sqlx::query("INSERT INTO transactions (id, user_id, post_date, description) VALUES ($1, $2, '2026-11-02', 'Gas station')")
        .bind(tx2_id).bind(user_id).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO splits (id, transaction_id, account_id, amount, quantity, reconciled_state) VALUES ($1, $2, $3, -1500.00, -1500.00, 'n')")
        .bind(uuid::Uuid::new_v4()).bind(tx2_id).bind(checking_id).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO splits (id, transaction_id, account_id, amount, quantity, reconciled_state) VALUES ($1, $2, $3, 1500.00, 1500.00, 'n')")
        .bind(uuid::Uuid::new_v4()).bind(tx2_id).bind(fuel_id).execute(&pool).await.unwrap();

    // Tx 3: Salary to Checking (₹5,000)
    let tx3_id = uuid::Uuid::new_v4();
    sqlx::query("INSERT INTO transactions (id, user_id, post_date, description) VALUES ($1, $2, '2026-11-03', 'Monthly Pay')")
        .bind(tx3_id).bind(user_id).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO splits (id, transaction_id, account_id, amount, quantity, reconciled_state) VALUES ($1, $2, $3, -5000.00, -5000.00, 'n')")
        .bind(uuid::Uuid::new_v4()).bind(tx3_id).bind(salary_id).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO splits (id, transaction_id, account_id, amount, quantity, reconciled_state) VALUES ($1, $2, $3, 5000.00, 5000.00, 'n')")
        .bind(uuid::Uuid::new_v4()).bind(tx3_id).bind(checking_id).execute(&pool).await.unwrap();

    // Check Balance Sheet
    let app = create_app(pool.clone());
    let resp = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/reports/balance-sheet")
                .header("authorization", format!("Bearer {}", token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let body_bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&body_bytes).unwrap();

    assert_eq!(json["is_balanced"], true, "Balance sheet must balance with mixed trees (ACC-8)");
    let assets_total = json["assets"]["total"].as_str().unwrap().parse::<f64>().unwrap();
    let liab_equity_total = json["total_liabilities_and_equity"].as_str().unwrap().parse::<f64>().unwrap();
    assert!((assets_total - liab_equity_total).abs() < 0.01);
    assert_eq!(assets_total, 13500.00); // 10000 - 1500 + 5000 = 13500

    // Check Income Statement
    let app2 = create_app(pool.clone());
    let resp2 = app2
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/reports/income-statement")
                .header("authorization", format!("Bearer {}", token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp2.status(), StatusCode::OK);
    let body_bytes2 = resp2.into_body().collect().await.unwrap().to_bytes();
    let json2: Value = serde_json::from_slice(&body_bytes2).unwrap();

    let rev_total = json2["revenues"]["total"].as_str().unwrap().parse::<f64>().unwrap();
    let exp_total = json2["expenses"]["total"].as_str().unwrap().parse::<f64>().unwrap();
    let net_inc = json2["net_income"].as_str().unwrap().parse::<f64>().unwrap();

    assert_eq!(rev_total, 5000.00);
    assert_eq!(exp_total, 1500.00);
    assert_eq!(net_inc, 3500.00);
}
