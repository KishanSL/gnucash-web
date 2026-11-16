use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use sqlx::PgPool;
use tower::ServiceExt;

use gnucash_backend::create_app;

async fn get_test_pool() -> PgPool {
    std::env::set_var("GNUCASH_TEST_MODE", "1");
    std::env::set_var("JWT_SECRET", "test-secret-key-must-be-at-least-32-chars-long!");
    let database_url = std::env::var("DATABASE_URL").unwrap_or_else(|_| {
        "postgres://gnucash:gnucash_secret@127.0.0.1:5432/gnucash".to_string()
    });
    gnucash_backend::db::init_db(&database_url)
        .await
        .expect("Failed to connect to and init test database")
}

#[tokio::test]
async fn test_health_check() {
    let pool = get_test_pool().await;
    let app = create_app(pool);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = response.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["status"], "ok");
}

#[tokio::test]
async fn test_get_accounts_and_tree() {
    let pool = get_test_pool().await;
    let app = create_app(pool.clone());

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/accounts")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let accounts: Value = serde_json::from_slice(&body).unwrap();
    assert!(accounts.as_array().unwrap().len() >= 5);

    let app2 = create_app(pool);
    let tree_response = app2
        .oneshot(
            Request::builder()
                .uri("/api/accounts/tree")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(tree_response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_double_entry_balance_validation() {
    let pool = get_test_pool().await;
    let app = create_app(pool.clone());

    let accounts = sqlx::query_as::<_, (uuid::Uuid, String)>(
        "SELECT id, name FROM accounts WHERE user_id = '00000000-0000-0000-0000-000000000001' AND placeholder = FALSE LIMIT 2",
    )
    .fetch_all(&pool)
    .await
    .unwrap();

    let acc1 = accounts[0].0;
    let acc2 = accounts[1].0;

    let payload = json!({
        "post_date": "2026-10-31",
        "description": "Unbalanced Test Transaction",
        "splits": [
            { "account_id": acc1, "amount": "100.00" },
            { "account_id": acc2, "amount": "-80.00" }
        ]
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/transactions")
                .header("content-type", "application/json")
                .body(Body::from(payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let body = response.into_body().collect().await.unwrap().to_bytes();
    let err_json: Value = serde_json::from_slice(&body).unwrap();
    assert!(err_json["error"].as_str().unwrap().contains("not balanced"));
}

#[tokio::test]
async fn test_minimum_two_splits_validation() {
    let pool = get_test_pool().await;
    let app = create_app(pool.clone());

    let accounts = sqlx::query_as::<_, (uuid::Uuid, String)>(
        "SELECT id, name FROM accounts WHERE user_id = '00000000-0000-0000-0000-000000000001' AND placeholder = FALSE LIMIT 1",
    )
    .fetch_all(&pool)
    .await
    .unwrap();

    let acc1 = accounts[0].0;

    let payload = json!({
        "post_date": "2026-10-31",
        "description": "One Split Transaction",
        "splits": [
            { "account_id": acc1, "amount": "0.00" }
        ]
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/transactions")
                .header("content-type", "application/json")
                .body(Body::from(payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_cannot_post_to_placeholder_account() {
    let pool = get_test_pool().await;
    let app = create_app(pool.clone());

    let placeholder = sqlx::query_as::<_, (uuid::Uuid,)>(
        "SELECT id FROM accounts WHERE user_id = '00000000-0000-0000-0000-000000000001' AND placeholder = TRUE LIMIT 1",
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    let regular = sqlx::query_as::<_, (uuid::Uuid,)>(
        "SELECT id FROM accounts WHERE user_id = '00000000-0000-0000-0000-000000000001' AND placeholder = FALSE LIMIT 1",
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    let payload = json!({
        "post_date": "2026-10-31",
        "description": "Attempt to post to placeholder",
        "splits": [
            { "account_id": placeholder.0, "amount": "50.00" },
            { "account_id": regular.0, "amount": "-50.00" }
        ]
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/transactions")
                .header("content-type", "application/json")
                .body(Body::from(payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_valid_double_entry_transaction_and_balances() {
    let pool = get_test_pool().await;
    let app = create_app(pool.clone());

    let checking = sqlx::query_as::<_, (uuid::Uuid,)>(
        "SELECT id FROM accounts WHERE user_id = '00000000-0000-0000-0000-000000000001' AND placeholder = FALSE AND (account_type = 'bank' OR name ILIKE '%Checking%' OR name ILIKE '%Bank%' OR name ILIKE '%Cash%') LIMIT 1",
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    let groceries = sqlx::query_as::<_, (uuid::Uuid,)>(
        "SELECT id FROM accounts WHERE user_id = '00000000-0000-0000-0000-000000000001' AND name ILIKE '%Groceries%' LIMIT 1",
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    let payload = json!({
        "post_date": "2026-10-31",
        "description": "Test Organic Supermarket Run",
        "splits": [
            { "account_id": groceries.0, "amount": "75.50", "memo": "Fresh vegetables" },
            { "account_id": checking.0, "amount": "-75.50", "memo": "Debit card payment" }
        ]
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/transactions")
                .header("content-type", "application/json")
                .body(Body::from(payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let tx_json: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(tx_json["splits"].as_array().unwrap().len(), 2);
}

#[tokio::test]
async fn test_account_register_running_balance() {
    let pool = get_test_pool().await;
    let app = create_app(pool.clone());

    let checking = sqlx::query_as::<_, (uuid::Uuid,)>(
        "SELECT id FROM accounts WHERE user_id = '00000000-0000-0000-0000-000000000001' AND placeholder = FALSE AND (account_type = 'bank' OR name ILIKE '%Checking%' OR name ILIKE '%Bank%' OR name ILIKE '%Cash%') LIMIT 1",
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    let response = app
        .oneshot(
            Request::builder()
                .uri(&format!("/api/accounts/{}/register", checking.0))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let register: Value = serde_json::from_slice(&body).unwrap();
    let items = register.as_array().unwrap();
    assert!(!items.is_empty(), "Register should contain entries");

    for item in items {
        assert!(item.get("running_balance").is_some());
        assert!(item.get("description").is_some());
    }
}

#[tokio::test]
async fn test_reconciliation_endpoint() {
    let pool = get_test_pool().await;
    let app = create_app(pool.clone());

    let checking = sqlx::query_as::<_, (uuid::Uuid,)>(
        "SELECT id FROM accounts WHERE user_id = '00000000-0000-0000-0000-000000000001' AND placeholder = FALSE AND (account_type = 'bank' OR name ILIKE '%Checking%' OR name ILIKE '%Bank%' OR name ILIKE '%Cash%') LIMIT 1",
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    let response = app
        .oneshot(
            Request::builder()
                .uri(&format!("/api/reconcile/{}", checking.0))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let data: Value = serde_json::from_slice(&body).unwrap();
    assert!(data.get("deposits").is_some());
    assert!(data.get("withdrawals").is_some());
    assert!(data.get("starting_balance").is_some());
}

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
async fn test_import_gnucash_sqlite() {
    let pool = get_test_pool().await;
    let app = create_app(pool.clone());

    // 1. Create a temporary in-file GnuCash SQLite DB
    let temp_db = tempfile::NamedTempFile::new().unwrap();
    let conn = rusqlite::Connection::open(temp_db.path()).unwrap();

    conn.execute_batch(
        r#"
        CREATE TABLE commodities (
            guid VARCHAR(32) PRIMARY KEY,
            namespace VARCHAR(2048),
            mnemonic VARCHAR(2048),
            fullname VARCHAR(2048),
            fraction INTEGER
        );
        INSERT INTO commodities VALUES ('c1111111111111111111111111111111', 'CURRENCY', 'INR', 'Indian Rupee', 100);

        CREATE TABLE accounts (
            guid VARCHAR(32) PRIMARY KEY,
            name VARCHAR(2048) NOT NULL,
            account_type VARCHAR(2048) NOT NULL,
            commodity_guid VARCHAR(32),
            commodity_scu INTEGER,
            non_std_scu INTEGER,
            parent_guid VARCHAR(32),
            code VARCHAR(2048),
            description VARCHAR(2048),
            hidden INTEGER,
            placeholder INTEGER
        );
        INSERT INTO accounts VALUES ('00000000000000000000000000000001', 'Root Account', 'ROOT', NULL, 100, 0, NULL, '', '', 0, 1);
        INSERT INTO accounts VALUES ('00000000000000000000000000000002', 'Assets', 'ASSET', 'c1111111111111111111111111111111', 100, 0, '00000000000000000000000000000001', '1000', 'Top Assets', 0, 1);
        INSERT INTO accounts VALUES ('00000000000000000000000000000003', 'HDFC Bank Account', 'BANK', 'c1111111111111111111111111111111', 100, 0, '00000000000000000000000000000002', '1110', 'Primary Checking', 0, 0);
        INSERT INTO accounts VALUES ('00000000000000000000000000000004', 'Expenses', 'EXPENSE', 'c1111111111111111111111111111111', 100, 0, '00000000000000000000000000000001', '5000', 'Operating Expenses', 0, 1);
        INSERT INTO accounts VALUES ('00000000000000000000000000000005', 'Food & Groceries', 'EXPENSE', 'c1111111111111111111111111111111', 100, 0, '00000000000000000000000000000004', '5100', 'Daily food groceries', 0, 0);

        CREATE TABLE transactions (
            guid VARCHAR(32) PRIMARY KEY,
            currency_guid VARCHAR(32),
            num VARCHAR(2048),
            post_date VARCHAR(14) NOT NULL,
            enter_date VARCHAR(14),
            description VARCHAR(2048)
        );
        INSERT INTO transactions VALUES ('t0000000000000000000000000000001', 'c1111111111111111111111111111111', 'TX-01', '20261025120000', '20261025120000', 'Supermarket Mart');

        CREATE TABLE splits (
            guid VARCHAR(32) PRIMARY KEY,
            tx_guid VARCHAR(32) NOT NULL,
            account_guid VARCHAR(32) NOT NULL,
            memo VARCHAR(2048),
            action VARCHAR(2048),
            reconcile_state VARCHAR(1),
            reconcile_date VARCHAR(14),
            value_num BIGINT NOT NULL,
            value_denom BIGINT NOT NULL,
            quantity_num BIGINT,
            quantity_denom BIGINT,
            lot_guid VARCHAR(32)
        );
        -- Debit Food (₹1,500.00), Credit Bank (-₹1,500.00)
        INSERT INTO splits VALUES ('s0000000000000000000000000000001', 't0000000000000000000000000000001', '00000000000000000000000000000005', 'Vegetables and dairy', '', 'n', NULL, 150000, 100, 150000, 100, NULL);
        INSERT INTO splits VALUES ('s0000000000000000000000000000002', 't0000000000000000000000000000001', '00000000000000000000000000000003', 'UPI payment', '', 'y', '20261025120000', -150000, 100, -150000, 100, NULL);
        "#,
    ).unwrap();

    let file_bytes = std::fs::read(temp_db.path()).unwrap();

    // 2. Build multipart body
    let boundary = "---------------------------974767299852498929531610575";
    let mut body = Vec::new();
    body.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
    body.extend_from_slice(b"Content-Disposition: form-data; name=\"file\"; filename=\"my_finances.gnucash\"\r\n");
    body.extend_from_slice(b"Content-Type: application/x-sqlite3\r\n\r\n");
    body.extend_from_slice(&file_bytes);
    body.extend_from_slice(format!("\r\n--{}--\r\n", boundary).as_bytes());

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/import/gnucash-sqlite?wipe=false")
                .header(
                    "content-type",
                    format!("multipart/form-data; boundary={}", boundary),
                )
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let res_body = response.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&res_body).unwrap();
    assert_eq!(json["success"], true);
    assert_eq!(json["accounts_imported"], 4);
    assert_eq!(json["transactions_imported"], 1);
    assert_eq!(json["splits_imported"], 2);
}

#[tokio::test]
async fn test_import_gnucash_compressed_xml() {
    use flate2::write::GzEncoder;
    use flate2::Compression;
    use std::io::Write;

    let pool = get_test_pool().await;
    let app = create_app(pool.clone());

    let xml_data = r#"<?xml version="1.0" encoding="utf-8" ?>
<gnc-v2 xmlns:gnc="http://www.gnucash.org/XML/gnc" xmlns:act="http://www.gnucash.org/XML/act" xmlns:trn="http://www.gnucash.org/XML/trn" xmlns:split="http://www.gnucash.org/XML/split" xmlns:cmdty="http://www.gnucash.org/XML/cmdty" xmlns:ts="http://www.gnucash.org/XML/ts" xmlns:slot="http://www.gnucash.org/XML/slot">
<gnc:book version="2.0.0">
  <gnc:account version="2.0.0">
    <act:name>Root Account</act:name>
    <act:id type="guid">10000000000000000000000000000000</act:id>
    <act:type>ROOT</act:type>
  </gnc:account>
  <gnc:account version="2.0.0">
    <act:name>Assets</act:name>
    <act:id type="guid">20000000000000000000000000000000</act:id>
    <act:type>ASSET</act:type>
    <act:parent type="guid">10000000000000000000000000000000</act:parent>
  </gnc:account>
  <gnc:account version="2.0.0">
    <act:name>Checking</act:name>
    <act:id type="guid">30000000000000000000000000000000</act:id>
    <act:type>BANK</act:type>
    <act:parent type="guid">20000000000000000000000000000000</act:parent>
  </gnc:account>
  <gnc:account version="2.0.0">
    <act:name>Salary</act:name>
    <act:id type="guid">40000000000000000000000000000000</act:id>
    <act:type>INCOME</act:type>
    <act:parent type="guid">10000000000000000000000000000000</act:parent>
  </gnc:account>
  <gnc:transaction version="2.0.0">
    <trn:id type="guid">50000000000000000000000000000000</trn:id>
    <trn:date-posted><ts:date>2026-05-01 12:00:00 +0000</ts:date></trn:date-posted>
    <trn:description>May Paycheck</trn:description>
    <trn:splits>
      <trn:split>
        <split:id type="guid">60000000000000000000000000000000</split:id>
        <split:account type="guid">30000000000000000000000000000000</split:account>
        <split:value>7500000/100</split:value>
        <split:reconciled-state>n</split:reconciled-state>
      </trn:split>
      <trn:split>
        <split:id type="guid">70000000000000000000000000000000</split:id>
        <split:account type="guid">40000000000000000000000000000000</split:account>
        <split:value>-7500000/100</split:value>
        <split:reconciled-state>n</split:reconciled-state>
      </trn:split>
    </trn:splits>
  </gnc:transaction>
</gnc:book>
</gnc-v2>"#;

    // Compress with Gzip
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(xml_data.as_bytes()).unwrap();
    let gzip_bytes = encoder.finish().unwrap();

    let boundary = "---------------------------12345678901234567890";
    let mut body = Vec::new();
    body.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
    body.extend_from_slice(b"Content-Disposition: form-data; name=\"file\"; filename=\"sample.gnucash\"\r\n");
    body.extend_from_slice(b"Content-Type: application/gzip\r\n\r\n");
    body.extend_from_slice(&gzip_bytes);
    body.extend_from_slice(format!("\r\n--{}--\r\n", boundary).as_bytes());

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/import/gnucash-sqlite?wipe=false")
                .header(
                    "content-type",
                    format!("multipart/form-data; boundary={}", boundary),
                )
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let res_body = response.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&res_body).unwrap();
    assert_eq!(json["success"], true);
    assert_eq!(json["accounts_imported"], 3);
    assert_eq!(json["transactions_imported"], 1);
    assert_eq!(json["splits_imported"], 2);
}

#[tokio::test]
async fn test_import_from_path_is_disabled() {
    let pool = get_test_pool().await;
    let app = create_app(pool);

    let payload = json!({
        "path": "/etc/passwd",
        "wipe": false,
        "default_currency": "INR"
    });

    let res = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/import/from-path")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_auth_login_and_reset_password() {
    let pool = get_test_pool().await;
    let app = create_app(pool.clone());

    // 1. Successful login with default user
    let mut login_payload = json!({
        "username": "kishan",
        "password": "Kishan@123"
    });

    let mut res = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/auth/login")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&login_payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    if res.status() == StatusCode::UNAUTHORIZED {
        let app_fallback = create_app(pool.clone());
        login_payload = json!({
            "username": "kishan",
            "password": "Kishan@123"
        });
        res = app_fallback
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/auth/login")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&login_payload).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
    }

    assert_eq!(res.status(), StatusCode::OK);
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&body).unwrap();
    assert!(json.get("token").is_some());
    assert_eq!(json["user"]["username"], "kishan");

    // 2. Request password reset without admin key (must be rejected 403 Forbidden)
    let reset_payload = json!({
        "username": "kishan"
    });

    let app_forbidden = create_app(pool.clone());
    let res_forbidden = app_forbidden
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/auth/reset-password")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&reset_payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res_forbidden.status(), StatusCode::FORBIDDEN);
    let forbidden_body = res_forbidden.into_body().collect().await.unwrap().to_bytes();
    let forbidden_json: Value = serde_json::from_slice(&forbidden_body).unwrap();
    assert!(forbidden_json["error"].as_str().unwrap().contains("disabled"));

    // 2b. Request password reset with X-Admin-Key (authorized CLI administrator)
    let jwt_secret = gnucash_backend::auth::get_jwt_secret();
    let app2 = create_app(pool.clone());
    let reset_res = app2
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/auth/reset-password")
                .header("content-type", "application/json")
                .header("x-admin-key", jwt_secret)
                .body(Body::from(serde_json::to_vec(&reset_payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(reset_res.status(), StatusCode::OK);

    // Retrieve generated temporary password written to server file
    let temp_password = gnucash_backend::handlers::auth::read_latest_temporary_password()
        .expect("Temporary password file should be written on server");
    assert!(temp_password.starts_with("TempPass-"));

    // 3. Login with temporary password retrieved from server file
    let app3 = create_app(pool.clone());
    let temp_login_payload = json!({
        "username": "kishan",
        "password": temp_password
    });

    let res3 = app3
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/auth/login")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&temp_login_payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res3.status(), StatusCode::OK);
    let temp_login_body = res3.into_body().collect().await.unwrap().to_bytes();
    let temp_login_json: Value = serde_json::from_slice(&temp_login_body).unwrap();
    let temp_token = temp_login_json["token"].as_str().unwrap();

    // 4. In logged-in user space, change password back to Kishan@123
    let app4 = create_app(pool);
    let change_pwd_payload = json!({
        "current_password": temp_password,
        "new_password": "Kishan@123"
    });

    let res4 = app4
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/auth/change-password")
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {}", temp_token))
                .body(Body::from(serde_json::to_vec(&change_pwd_payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res4.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_auth_change_password() {
    let pool = get_test_pool().await;

    // Login to get token
    let app1 = create_app(pool.clone());
    let mut login_payload = json!({
        "username": "kishan",
        "password": "Kishan@123"
    });
    let mut login_res = app1
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/auth/login")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&login_payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    if login_res.status() == StatusCode::UNAUTHORIZED {
        let app_fallback = create_app(pool.clone());
        login_payload = json!({
            "username": "kishan",
            "password": "Kishan@123"
        });
        login_res = app_fallback
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/auth/login")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&login_payload).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
    }
    assert_eq!(login_res.status(), StatusCode::OK);
    let current_pwd = login_payload["password"].as_str().unwrap().to_string();
    let body = login_res.into_body().collect().await.unwrap().to_bytes();
    let login_data: Value = serde_json::from_slice(&body).unwrap();
    let token = login_data["token"].as_str().unwrap();

    // Change password with wrong current password -> should fail
    let app2 = create_app(pool.clone());
    let fail_res = app2
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/auth/change-password")
                .header("Authorization", format!("Bearer {}", token))
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "current_password": "wrongpassword",
                        "new_password": "updated_password_456"
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(fail_res.status(), StatusCode::BAD_REQUEST);

    // Change password with correct current password -> should succeed
    let app3 = create_app(pool.clone());
    let ok_res = app3
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/auth/change-password")
                .header("Authorization", format!("Bearer {}", token))
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "current_password": current_pwd,
                        "new_password": "updated_password_456"
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(ok_res.status(), StatusCode::OK);

    // Verify login with updated password
    let app4 = create_app(pool.clone());
    let new_login = app4
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/auth/login")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "username": "kishan",
                        "password": "updated_password_456"
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(new_login.status(), StatusCode::OK);
    let new_login_body = new_login.into_body().collect().await.unwrap().to_bytes();
    let new_login_data: Value = serde_json::from_slice(&new_login_body).unwrap();
    let updated_token = new_login_data["token"].as_str().unwrap();

    // Change back to Kishan@123 via authenticated change-password endpoint
    let app5 = create_app(pool);
    let reset_back_res = app5
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/auth/change-password")
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {}", updated_token))
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "current_password": "updated_password_456",
                        "new_password": "Kishan@123"
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(reset_back_res.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_signup_disabled_by_default() {
    let pool = get_test_pool().await;
    let app = create_app(pool);

    let signup_payload = json!({
        "username": "random_attacker",
        "email": "attacker@example.com",
        "password": "Attacker@123"
    });

    let res = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/auth/signup")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&signup_payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::FORBIDDEN);
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&body).unwrap();
    assert!(json["error"].as_str().unwrap().contains("disabled"));
}

#[tokio::test]
async fn test_signup_with_admin_key() {
    let pool = get_test_pool().await;
    let _ = sqlx::query("DELETE FROM users WHERE username = 'admin_provisioned_user' OR email = 'provisioned@example.com'")
        .execute(&pool)
        .await;
    let app = create_app(pool);

    let signup_payload = json!({
        "username": "admin_provisioned_user",
        "email": "provisioned@example.com",
        "password": "Attacker@123"
    });

    let jwt_secret = gnucash_backend::auth::get_jwt_secret();

    let res = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/auth/signup")
                .header("content-type", "application/json")
                .header("x-admin-key", jwt_secret)
                .body(Body::from(serde_json::to_vec(&signup_payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::CREATED);
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&body).unwrap();
    assert!(json.get("token").is_some());
    assert_eq!(json["user"]["username"], "admin_provisioned_user");
}

#[tokio::test]
async fn test_login_non_enumeration() {
    let pool = get_test_pool().await;
    let app1 = create_app(pool.clone());

    // 1. Non-existent username
    let res1 = app1
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/auth/login")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&json!({
                    "username": "totally_non_existent_user_9999",
                    "password": "WrongPassword@123"
                })).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res1.status(), StatusCode::UNAUTHORIZED);
    let body1 = res1.into_body().collect().await.unwrap().to_bytes();
    let json1: Value = serde_json::from_slice(&body1).unwrap();
    assert_eq!(json1["error"], "Invalid username or password");

    // 2. Existing username with wrong password
    let app2 = create_app(pool);
    let res2 = app2
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/auth/login")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&json!({
                    "username": "kishan",
                    "password": "WrongPassword@999"
                })).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res2.status(), StatusCode::UNAUTHORIZED);
    let body2 = res2.into_body().collect().await.unwrap().to_bytes();
    let json2: Value = serde_json::from_slice(&body2).unwrap();
    // Verify identical error message is returned
    assert_eq!(json2["error"], "Invalid username or password");
}

#[tokio::test]
async fn test_change_password_without_current_password_fails() {
    let pool = get_test_pool().await;
    let user_id = uuid::Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap();
    let current_ver = sqlx::query_scalar::<_, i32>(
        "SELECT token_version FROM users WHERE id = $1"
    )
    .bind(user_id)
    .fetch_one(&pool)
    .await
    .unwrap_or(0);

    let token = gnucash_backend::auth::create_jwt(
        user_id,
        "kishan",
        "kishan@localhost",
        current_ver,
    )
    .unwrap();

    // Change password omitting current_password field
    let app2 = create_app(pool.clone());
    let res = app2
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/auth/change-password")
                .header("Authorization", format!("Bearer {}", token))
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "new_password": "some_random_password_789"
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert!(
        res.status() == StatusCode::BAD_REQUEST || res.status() == StatusCode::UNPROCESSABLE_ENTITY,
        "Omitting current_password must be rejected with 400 or 422, got {}",
        res.status()
    );
}

#[tokio::test]
async fn test_account_hierarchy_cycle_prevention() {
    let pool = get_test_pool().await;
    let app = create_app(pool.clone());

    // 1. Create Account A
    let res_a = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/accounts")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&json!({
                    "name": "Cycle Account A",
                    "account_type": "asset"
                })).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res_a.status(), StatusCode::CREATED);
    let body_a = res_a.into_body().collect().await.unwrap().to_bytes();
    let acc_a: Value = serde_json::from_slice(&body_a).unwrap();
    let id_a = acc_a["id"].as_str().unwrap();

    // 2. Create Account B with parent A
    let app2 = create_app(pool.clone());
    let res_b = app2
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/accounts")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&json!({
                    "name": "Cycle Account B",
                    "account_type": "asset",
                    "parent_id": id_a
                })).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res_b.status(), StatusCode::CREATED);
    let body_b = res_b.into_body().collect().await.unwrap().to_bytes();
    let acc_b: Value = serde_json::from_slice(&body_b).unwrap();
    let id_b = acc_b["id"].as_str().unwrap();

    // 3. Attempt to update Account A's parent to B (creating A -> B -> A cycle)
    let app3 = create_app(pool.clone());
    let res_cycle = app3
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/api/accounts/{}", id_a))
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&json!({
                    "parent_id": id_b
                })).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(
        res_cycle.status(),
        StatusCode::BAD_REQUEST,
        "Cycle creation must be rejected with 400 Bad Request"
    );
}

#[tokio::test]
async fn test_zero_balance_tolerance() {
    let pool = get_test_pool().await;
    let app = create_app(pool.clone());

    let accounts = sqlx::query_as::<_, (uuid::Uuid, String)>(
        "SELECT id, name FROM accounts WHERE user_id = '00000000-0000-0000-0000-000000000001' AND placeholder = FALSE LIMIT 2",
    )
    .fetch_all(&pool)
    .await
    .unwrap();

    let acc1 = accounts[0].0;
    let acc2 = accounts[1].0;

    // Transaction with 0.01 imbalance
    let payload = json!({
        "post_date": "2026-11-01",
        "description": "0.01 Cent Imbalance Test",
        "splits": [
            { "account_id": acc1, "amount": "100.00" },
            { "account_id": acc2, "amount": "-99.99" }
        ]
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/transactions")
                .header("content-type", "application/json")
                .body(Body::from(payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(
        response.status(),
        StatusCode::BAD_REQUEST,
        "0.01 imbalance must be strictly rejected"
    );
}

#[tokio::test]
async fn test_cross_tenant_isolation_on_import() {
    let pool = get_test_pool().await;

    // Create Tenant B in users table
    let user_b_id = uuid::Uuid::parse_str("00000000-0000-0000-0000-000000000002").unwrap();
    sqlx::query(
        r#"INSERT INTO users (id, username, email, password_hash)
           VALUES ($1, 'tenant_b', 'tenant_b@localhost', '$2b$12$e8YgN17Pj9QGqZ2eCq.X3OMWj944j/mYFkU5nZ7cI19j1r5u0gJvG')
           ON CONFLICT (id) DO NOTHING"#
    )
    .bind(user_b_id)
    .execute(&pool)
    .await
    .unwrap();

    let user_a_id = uuid::Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap();
    let user_a_acc_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM accounts WHERE user_id = $1")
        .bind(user_a_id)
        .fetch_one(&pool)
        .await
        .unwrap();

    let user_b_token = gnucash_backend::auth::create_jwt(user_b_id, "tenant_b", "tenant_b@localhost", 0).unwrap();

    // Build SQLite database for User B import
    let temp_db = tempfile::NamedTempFile::new().unwrap();
    let conn = rusqlite::Connection::open(temp_db.path()).unwrap();
    conn.execute_batch(
        r#"
        CREATE TABLE accounts (
            guid VARCHAR(32) PRIMARY KEY,
            name VARCHAR(2048) NOT NULL,
            account_type VARCHAR(2048) NOT NULL,
            commodity_guid VARCHAR(32),
            commodity_scu INTEGER NOT NULL,
            non_std_scu INTEGER NOT NULL,
            parent_guid VARCHAR(32),
            code VARCHAR(2048),
            description VARCHAR(2048),
            hidden INTEGER,
            placeholder INTEGER
        );
        INSERT INTO accounts VALUES ('00000000000000000000000000000001', 'Tenant B Root', 'ROOT', NULL, 100, 0, NULL, '', '', 0, 1);
        INSERT INTO accounts VALUES ('00000000000000000000000000000002', 'Tenant B Checking', 'BANK', NULL, 100, 0, '00000000000000000000000000000001', '1100', 'Private Account', 0, 0);
        INSERT INTO accounts VALUES ('00000000000000000000000000000003', 'Tenant B Savings', 'BANK', NULL, 100, 0, '00000000000000000000000000000001', '1110', 'Savings Account', 0, 0);

        CREATE TABLE transactions (guid VARCHAR(32) PRIMARY KEY, currency_guid VARCHAR(32), num VARCHAR(2048), post_date VARCHAR(14) NOT NULL, enter_date VARCHAR(14), description VARCHAR(2048));
        CREATE TABLE splits (guid VARCHAR(32) PRIMARY KEY, tx_guid VARCHAR(32) NOT NULL, account_guid VARCHAR(32) NOT NULL, memo VARCHAR(2048), action VARCHAR(2048), reconcile_state VARCHAR(1), reconcile_date VARCHAR(14), value_num BIGINT NOT NULL, value_denom BIGINT NOT NULL, quantity_num BIGINT, quantity_denom BIGINT, lot_guid VARCHAR(32));
        "#,
    ).unwrap();

    let file_bytes = std::fs::read(temp_db.path()).unwrap();
    let boundary = "---------------------------cross_tenant_test_boundary";
    let mut body = Vec::new();
    body.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
    body.extend_from_slice(b"Content-Disposition: form-data; name=\"file\"; filename=\"tenant_b.gnucash\"\r\n");
    body.extend_from_slice(b"Content-Type: application/x-sqlite3\r\n\r\n");
    body.extend_from_slice(&file_bytes);
    body.extend_from_slice(format!("\r\n--{}--\r\n", boundary).as_bytes());

    let app = create_app(pool.clone());
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/import/gnucash-sqlite?wipe=false")
                .header("Authorization", format!("Bearer {}", user_b_token))
                .header("content-type", format!("multipart/form-data; boundary={}", boundary))
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    // Verify User A's accounts are 100% unchanged
    let user_a_acc_count_after: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM accounts WHERE user_id = $1")
        .bind(user_a_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        user_a_acc_count, user_a_acc_count_after,
        "User A accounts must not be affected by User B's import"
    );

    // Verify User B's accounts are correctly owned by User B
    let user_b_acc_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM accounts WHERE user_id = $1")
        .bind(user_b_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(user_b_acc_count >= 2, "User B must have imported accounts");
}

#[tokio::test]
async fn test_update_transaction_preserves_reconciled_state() {
    let pool = get_test_pool().await;
    let app = create_app(pool.clone());

    // 1. Get 2 non-placeholder accounts belonging to default test user
    let acc1 = sqlx::query_scalar::<_, uuid::Uuid>(
        "SELECT id FROM accounts WHERE user_id = '00000000-0000-0000-0000-000000000001' AND placeholder = false ORDER BY id LIMIT 1",
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    let acc2 = sqlx::query_scalar::<_, uuid::Uuid>(
        "SELECT id FROM accounts WHERE user_id = '00000000-0000-0000-0000-000000000001' AND placeholder = false AND id != $1 ORDER BY id LIMIT 1",
    )
    .bind(acc1)
    .fetch_one(&pool)
    .await
    .unwrap();

    // 2. Create a new transaction
    let create_payload = json!({
        "post_date": "2026-11-01",
        "description": "Original Reconciled Test Tx",
        "splits": [
            { "account_id": acc1, "amount": "75.00", "memo": "Split 1" },
            { "account_id": acc2, "amount": "-75.00", "memo": "Split 2" }
        ]
    });

    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/transactions")
                .header("content-type", "application/json")
                .body(Body::from(create_payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::CREATED);
    let body_bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let tx_data: Value = serde_json::from_slice(&body_bytes).unwrap();
    let tx_id = tx_data["id"].as_str().unwrap();
    let split_ids: Vec<String> = tx_data["splits"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["id"].as_str().unwrap().to_string())
        .collect();

    // 3. Reconcile both splits via their respective account endpoints
    let app = create_app(pool.clone());
    let rec_payload = json!({
        "statement_date": "2026-11-05",
        "reconciled_split_ids": [&split_ids[0]]
    });

    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(&format!("/api/reconcile/{}", acc1))
                .header("content-type", "application/json")
                .body(Body::from(rec_payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let app = create_app(pool.clone());
    let rec_payload2 = json!({
        "statement_date": "2026-11-05",
        "reconciled_split_ids": [&split_ids[1]]
    });

    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(&format!("/api/reconcile/{}", acc2))
                .header("content-type", "application/json")
                .body(Body::from(rec_payload2.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // 4. Verify transaction splits are now 'y' and have reconciled_date
    let app = create_app(pool.clone());
    let resp = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(&format!("/api/transactions/{}", tx_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body_bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let reconciled_tx: Value = serde_json::from_slice(&body_bytes).unwrap();
    for split in reconciled_tx["splits"].as_array().unwrap() {
        assert_eq!(split["reconciled_state"], "y");
        assert!(split["reconciled_date"].as_str().is_some());
    }
    let orig_date_split1 = reconciled_tx["splits"][0]["reconciled_date"].as_str().unwrap().to_string();

    // 5. Update transaction description (sending same splits without reconciled_state, like UI does)
    let app = create_app(pool.clone());
    let update_payload = json!({
        "description": "Updated Description Typo Fixed",
        "splits": [
            { "account_id": acc1, "amount": "75.00", "memo": "Split 1 updated" },
            { "account_id": acc2, "amount": "-75.00", "memo": "Split 2 updated" }
        ]
    });

    let resp = app
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(&format!("/api/transactions/{}", tx_id))
                .header("content-type", "application/json")
                .body(Body::from(update_payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // 6. Verify splits STILL have reconciled_state = 'y' and preserved reconciled_date and split IDs!
    let body_bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let updated_tx: Value = serde_json::from_slice(&body_bytes).unwrap();
    assert_eq!(updated_tx["description"], "Updated Description Typo Fixed");
    for (i, split) in updated_tx["splits"].as_array().unwrap().iter().enumerate() {
        assert_eq!(
            split["reconciled_state"], "y",
            "Split {} reconciled_state must remain 'y' after editing transaction",
            i
        );
        assert!(
            split["reconciled_date"].as_str().is_some(),
            "Split {} reconciled_date must be preserved",
            i
        );
        assert_eq!(
            split["id"].as_str().unwrap(),
            split_ids[i],
            "Split ID must be preserved in place"
        );
    }
    assert_eq!(
        updated_tx["splits"][0]["reconciled_date"].as_str().unwrap(),
        orig_date_split1,
        "Original reconciled timestamp must be preserved"
    );

    // 7. (Issue #16): When split amounts or accounts change, reconciled_state MUST reset to 'n'
    let app = create_app(pool.clone());
    let change_amount_payload = json!({
        "description": "Changed Amounts Test",
        "splits": [
            { "id": split_ids[0], "account_id": acc1, "amount": "90.00", "reconciled_state": "y" },
            { "id": split_ids[1], "account_id": acc2, "amount": "-90.00", "reconciled_state": "y" }
        ]
    });

    let resp = app
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(&format!("/api/transactions/{}", tx_id))
                .header("content-type", "application/json")
                .body(Body::from(change_amount_payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let body_bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let changed_amount_tx: Value = serde_json::from_slice(&body_bytes).unwrap();
    for split in changed_amount_tx["splits"].as_array().unwrap() {
        assert_eq!(
            split["reconciled_state"], "n",
            "Split reconciled_state must reset to 'n' when amount changes"
        );
        assert!(
            split["reconciled_date"].is_null(),
            "Split reconciled_date must be cleared when amount changes"
        );
    }
}

#[tokio::test]
async fn test_update_transaction_with_multiple_splits_sharing_same_account() {
    // Issue #6: Transaction with two splits sharing the same account can be updated
    let pool = get_test_pool().await;
    let app = create_app(pool.clone());

    let acc1 = sqlx::query_scalar::<_, uuid::Uuid>(
        "SELECT id FROM accounts WHERE user_id = '00000000-0000-0000-0000-000000000001' AND placeholder = false ORDER BY id LIMIT 1",
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    let acc2 = sqlx::query_scalar::<_, uuid::Uuid>(
        "SELECT id FROM accounts WHERE user_id = '00000000-0000-0000-0000-000000000001' AND placeholder = false AND id != $1 ORDER BY id LIMIT 1",
    )
    .bind(acc1)
    .fetch_one(&pool)
    .await
    .unwrap();

    // Create transaction with two splits on acc1 and one on acc2
    let create_payload = json!({
        "post_date": "2026-11-02",
        "description": "Multi-split Same Account Tx",
        "splits": [
            { "account_id": acc1, "amount": "40.00", "memo": "Item A" },
            { "account_id": acc1, "amount": "60.00", "memo": "Item B" },
            { "account_id": acc2, "amount": "-100.00", "memo": "Total Payment" }
        ]
    });

    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/transactions")
                .header("content-type", "application/json")
                .body(Body::from(create_payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let body_bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let tx_data: Value = serde_json::from_slice(&body_bytes).unwrap();
    let tx_id = tx_data["id"].as_str().unwrap();

    // Now update transaction description
    let app = create_app(pool.clone());
    let update_payload = json!({
        "description": "Multi-split Same Account Tx Updated",
        "splits": [
            { "account_id": acc1, "amount": "40.00", "memo": "Item A" },
            { "account_id": acc1, "amount": "60.00", "memo": "Item B" },
            { "account_id": acc2, "amount": "-100.00", "memo": "Total Payment" }
        ]
    });

    let resp = app
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(&format!("/api/transactions/{}", tx_id))
                .header("content-type", "application/json")
                .body(Body::from(update_payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK, "Editing transaction with shared accounts must succeed");
}

#[tokio::test]
async fn test_create_transaction_normalizes_reconciled_state_uppercase_y() {
    // Issue #14: 'Y' is normalized to 'y' and sets reconciled_date
    let pool = get_test_pool().await;
    let app = create_app(pool.clone());

    let acc1 = sqlx::query_scalar::<_, uuid::Uuid>(
        "SELECT id FROM accounts WHERE user_id = '00000000-0000-0000-0000-000000000001' AND placeholder = false ORDER BY id LIMIT 1",
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    let acc2 = sqlx::query_scalar::<_, uuid::Uuid>(
        "SELECT id FROM accounts WHERE user_id = '00000000-0000-0000-0000-000000000001' AND placeholder = false AND id != $1 ORDER BY id LIMIT 1",
    )
    .bind(acc1)
    .fetch_one(&pool)
    .await
    .unwrap();

    let create_payload = json!({
        "post_date": "2026-11-03",
        "description": "Uppercase Y Test",
        "splits": [
            { "account_id": acc1, "amount": "50.00", "reconciled_state": "Y" },
            { "account_id": acc2, "amount": "-50.00", "reconciled_state": "N" }
        ]
    });

    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/transactions")
                .header("content-type", "application/json")
                .body(Body::from(create_payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let body_bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let tx_data: Value = serde_json::from_slice(&body_bytes).unwrap();
    let split1 = &tx_data["splits"][0];
    let split2 = &tx_data["splits"][1];
    assert_eq!(split1["reconciled_state"], "y");
    assert!(split1["reconciled_date"].as_str().is_some());
    assert_eq!(split2["reconciled_state"], "n");
}

#[tokio::test]
async fn test_jwt_revocation_on_password_change() {
    // Issue #8: Token revocation on password change
    let pool = get_test_pool().await;

    let user_id = uuid::Uuid::new_v4();
    let username = format!("revoketest_{}", &user_id.to_string()[..8]);
    let email = format!("{}@example.com", username);
    let password_hash = bcrypt::hash("InitialPass123", bcrypt::DEFAULT_COST).unwrap();

    sqlx::query(
        "INSERT INTO users (id, username, email, password_hash, token_version) VALUES ($1, $2, $3, $4, 0)"
    )
    .bind(user_id)
    .bind(&username)
    .bind(&email)
    .bind(&password_hash)
    .execute(&pool)
    .await
    .unwrap();

    let token_v0 = gnucash_backend::auth::create_jwt(user_id, &username, &email, 0).unwrap();

    // Verify token_v0 is accepted
    let app = create_app(pool.clone());
    let resp = app.oneshot(
        Request::builder()
            .method("GET")
            .uri("/api/auth/me")
            .header("authorization", format!("Bearer {}", token_v0))
            .body(Body::empty())
            .unwrap()
    ).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // Change password
    let app = create_app(pool.clone());
    let change_payload = json!({
        "current_password": "InitialPass123",
        "new_password": "NewSecretPassword456"
    });
    let resp = app.oneshot(
        Request::builder()
            .method("POST")
            .uri("/api/auth/change-password")
            .header("authorization", format!("Bearer {}", token_v0))
            .header("content-type", "application/json")
            .body(Body::from(change_payload.to_string()))
            .unwrap()
    ).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // Verify token_v0 is now REVOKED (401 UNAUTHORIZED)
    let app = create_app(pool.clone());
    let resp = app.oneshot(
        Request::builder()
            .method("GET")
            .uri("/api/auth/me")
            .header("authorization", format!("Bearer {}", token_v0))
            .body(Body::empty())
            .unwrap()
    ).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

    // Log in with new password -> token_v1
    let app = create_app(pool.clone());
    let login_payload = json!({
        "username": username,
        "password": "NewSecretPassword456"
    });
    let resp = app.oneshot(
        Request::builder()
            .method("POST")
            .uri("/api/auth/login")
            .header("content-type", "application/json")
            .body(Body::from(login_payload.to_string()))
            .unwrap()
    ).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body_bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let login_data: Value = serde_json::from_slice(&body_bytes).unwrap();
    let token_v1 = login_data["token"].as_str().unwrap();

    // Verify token_v1 works
    let app = create_app(pool.clone());
    let resp = app.oneshot(
        Request::builder()
            .method("GET")
            .uri("/api/auth/me")
            .header("authorization", format!("Bearer {}", token_v1))
            .body(Body::empty())
            .unwrap()
    ).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_account_and_transaction_update_field_clearing_and_validation() {
    // Issue #10: Clearing fields with null and validation on update
    let pool = get_test_pool().await;

    // 1. Create a parent account and a child account
    let app = create_app(pool.clone());
    let parent_payload = json!({
        "name": "Test Parent Acct",
        "account_type": "asset",
        "currency": "INR"
    });
    let resp = app.oneshot(
        Request::builder()
            .method("POST")
            .uri("/api/accounts")
            .header("content-type", "application/json")
            .body(Body::from(parent_payload.to_string()))
            .unwrap()
    ).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let p_data: Value = serde_json::from_slice(&resp.into_body().collect().await.unwrap().to_bytes()).unwrap();
    let parent_id = p_data["id"].as_str().unwrap();

    let app = create_app(pool.clone());
    let child_payload = json!({
        "name": "Test Child Acct",
        "account_type": "asset",
        "parent_id": parent_id,
        "code": "1234",
        "description": "Initial child description"
    });
    let resp = app.oneshot(
        Request::builder()
            .method("POST")
            .uri("/api/accounts")
            .header("content-type", "application/json")
            .body(Body::from(child_payload.to_string()))
            .unwrap()
    ).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let c_data: Value = serde_json::from_slice(&resp.into_body().collect().await.unwrap().to_bytes()).unwrap();
    let child_id = c_data["id"].as_str().unwrap();

    // 2. Test PUT with invalid account_type -> 400
    let app = create_app(pool.clone());
    let invalid_type_payload = json!({
        "account_type": "bogus_type"
    });
    let resp = app.oneshot(
        Request::builder()
            .method("PUT")
            .uri(&format!("/api/accounts/{}", child_id))
            .header("content-type", "application/json")
            .body(Body::from(invalid_type_payload.to_string()))
            .unwrap()
    ).await.unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

    // 3. Test PUT with parent_id: null -> moves to top level (parent_id is null)
    let app = create_app(pool.clone());
    let clear_parent_payload = json!({
        "parent_id": null,
        "code": null,
        "description": null
    });
    let resp = app.oneshot(
        Request::builder()
            .method("PUT")
            .uri(&format!("/api/accounts/{}", child_id))
            .header("content-type", "application/json")
            .body(Body::from(clear_parent_payload.to_string()))
            .unwrap()
    ).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let updated_child: Value = serde_json::from_slice(&resp.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert!(updated_child["parent_id"].is_null(), "parent_id should be cleared to null");
    assert!(updated_child["code"].is_null(), "code should be cleared to null");
    assert!(updated_child["description"].is_null(), "description should be cleared to null");

    // 4. Test Transaction clearing notes & num, and validation
    let app = create_app(pool.clone());
    let tx_create_payload = json!({
        "post_date": "2026-11-04",
        "description": "Clear Fields Tx Test",
        "num": "CHK-999",
        "notes": "Original note content",
        "splits": [
            { "account_id": parent_id, "amount": "100.00" },
            { "account_id": child_id, "amount": "-100.00" }
        ]
    });
    let resp = app.oneshot(
        Request::builder()
            .method("POST")
            .uri("/api/transactions")
            .header("content-type", "application/json")
            .body(Body::from(tx_create_payload.to_string()))
            .unwrap()
    ).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let tx_data: Value = serde_json::from_slice(&resp.into_body().collect().await.unwrap().to_bytes()).unwrap();
    let tx_id = tx_data["id"].as_str().unwrap();

    // Empty description -> 400
    let app = create_app(pool.clone());
    let empty_desc_payload = json!({
        "description": ""
    });
    let resp = app.oneshot(
        Request::builder()
            .method("PUT")
            .uri(&format!("/api/transactions/{}", tx_id))
            .header("content-type", "application/json")
            .body(Body::from(empty_desc_payload.to_string()))
            .unwrap()
    ).await.unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

    // Clear notes & num with null
    let app = create_app(pool.clone());
    let clear_tx_payload = json!({
        "notes": null,
        "num": null
    });
    let resp = app.oneshot(
        Request::builder()
            .method("PUT")
            .uri(&format!("/api/transactions/{}", tx_id))
            .header("content-type", "application/json")
            .body(Body::from(clear_tx_payload.to_string()))
            .unwrap()
    ).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let updated_tx: Value = serde_json::from_slice(&resp.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert!(updated_tx["notes"].is_null(), "notes should be cleared to null");
    assert!(updated_tx["num"].is_null(), "num should be cleared to null");
}

#[tokio::test]
async fn test_transaction_notes_roundtrip_import_export() {
    // Issue #11: Transaction notes preserved through export and import
    let pool = get_test_pool().await;

    let user_id = uuid::Uuid::new_v4();
    let username = format!("notestest_{}", &user_id.to_string()[..8]);
    let email = format!("{}@example.com", username);
    sqlx::query(
        "INSERT INTO users (id, username, email, password_hash) VALUES ($1, $2, $3, 'hash')"
    )
    .bind(user_id)
    .bind(&username)
    .bind(&email)
    .execute(&pool)
    .await
    .unwrap();

    let user_token = gnucash_backend::auth::create_jwt(user_id, &username, &email, 0).unwrap();

    let acc1_id = uuid::Uuid::new_v4();
    let acc2_id = uuid::Uuid::new_v4();
    sqlx::query("INSERT INTO accounts (id, name, account_type, currency, placeholder, user_id) VALUES ($1, 'Checking', 'bank', 'INR', false, $2), ($3, 'Groceries', 'expense', 'INR', false, $2)")
        .bind(acc1_id)
        .bind(user_id)
        .bind(acc2_id)
        .execute(&pool)
        .await
        .unwrap();

    let app = create_app(pool.clone());
    let create_payload = json!({
        "post_date": "2026-11-05",
        "description": "Weekly Grocery Run",
        "notes": "Paid with GPay; receipt saved in drawer",
        "splits": [
            { "account_id": acc2_id, "amount": "1500.00" },
            { "account_id": acc1_id, "amount": "-1500.00" }
        ]
    });
    let resp = app.oneshot(
        Request::builder()
            .method("POST")
            .uri("/api/transactions")
            .header("authorization", format!("Bearer {}", user_token))
            .header("content-type", "application/json")
            .body(Body::from(create_payload.to_string()))
            .unwrap()
    ).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);

    // Export SQLite database
    let app = create_app(pool.clone());
    let resp = app.oneshot(
        Request::builder()
            .method("GET")
            .uri("/api/export/gnucash-sqlite")
            .header("authorization", format!("Bearer {}", user_token))
            .body(Body::empty())
            .unwrap()
    ).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let sqlite_bytes = resp.into_body().collect().await.unwrap().to_bytes();

    // Verify SQLite slots table has the notes
    let temp_db = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(temp_db.path(), &sqlite_bytes).unwrap();
    let conn = rusqlite::Connection::open(temp_db.path()).unwrap();
    let slot_val: String = conn.query_row(
        "SELECT string_val FROM slots WHERE name = 'notes'",
        [],
        |row| row.get(0)
    ).unwrap();
    assert_eq!(slot_val, "Paid with GPay; receipt saved in drawer");

    // Import this SQLite file into a new fresh user
    let user2_id = uuid::Uuid::new_v4();
    let username2 = format!("notestest2_{}", &user2_id.to_string()[..8]);
    let email2 = format!("{}@example.com", username2);
    sqlx::query(
        "INSERT INTO users (id, username, email, password_hash) VALUES ($1, $2, $3, 'hash')"
    )
    .bind(user2_id)
    .bind(&username2)
    .bind(&email2)
    .execute(&pool)
    .await
    .unwrap();
    let user2_token = gnucash_backend::auth::create_jwt(user2_id, &username2, &email2, 0).unwrap();

    let boundary = "---------------------------974767299852498929531610575";
    let mut body = Vec::new();
    body.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
    body.extend_from_slice(b"Content-Disposition: form-data; name=\"file\"; filename=\"test_notes.gnucash\"\r\n");
    body.extend_from_slice(b"Content-Type: application/x-sqlite3\r\n\r\n");
    body.extend_from_slice(&sqlite_bytes);
    body.extend_from_slice(format!("\r\n--{}--\r\n", boundary).as_bytes());

    let app = create_app(pool.clone());
    let resp = app.oneshot(
        Request::builder()
            .method("POST")
            .uri("/api/import/gnucash-sqlite?wipe=true")
            .header("authorization", format!("Bearer {}", user2_token))
            .header("content-type", format!("multipart/form-data; boundary={}", boundary))
            .body(Body::from(body))
            .unwrap()
    ).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // Verify imported transaction has notes preserved in Postgres
    let imported_notes: Option<String> = sqlx::query_scalar(
        "SELECT notes FROM transactions WHERE user_id = $1"
    )
    .bind(user2_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(imported_notes.as_deref(), Some("Paid with GPay; receipt saved in drawer"));
}

#[tokio::test]
async fn test_reimport_without_wipe_replaces_stale_splits() {
    // Issue #5: Re-import without wipe removes stale splits and keeps balance intact
    let pool = get_test_pool().await;
    let user_id = uuid::Uuid::new_v4();
    let username = format!("reimport_{}", &user_id.to_string()[..8]);
    let email = format!("{}@example.com", username);
    sqlx::query("INSERT INTO users (id, username, email, password_hash) VALUES ($1, $2, $3, 'hash')")
        .bind(user_id)
        .bind(&username)
        .bind(&email)
        .execute(&pool)
        .await
        .unwrap();
    let user_token = gnucash_backend::auth::create_jwt(user_id, &username, &email, 0).unwrap();

    let temp_db1 = tempfile::NamedTempFile::new().unwrap();
    let conn = rusqlite::Connection::open(temp_db1.path()).unwrap();
    conn.execute_batch(r#"
        CREATE TABLE accounts (guid TEXT PRIMARY KEY, name TEXT, account_type TEXT, commodity_guid TEXT, parent_guid TEXT, code TEXT, description TEXT, hidden INT, placeholder INT);
        CREATE TABLE transactions (guid TEXT PRIMARY KEY, currency_guid TEXT, num TEXT, post_date TEXT, enter_date TEXT, description TEXT);
        CREATE TABLE splits (guid TEXT PRIMARY KEY, tx_guid TEXT, account_guid TEXT, memo TEXT, action TEXT, reconcile_state TEXT, reconcile_date TEXT, value_num INT, value_denom INT, quantity_num INT, quantity_denom INT, lot_guid TEXT);
        CREATE TABLE commodities (guid TEXT PRIMARY KEY, namespace TEXT, mnemonic TEXT, fullname TEXT, cusip TEXT, fraction INT, quote_flag INT, quote_source TEXT);

        INSERT INTO commodities VALUES ('c1', 'CURRENCY', 'INR', 'INR', '', 100, 1, 'currency');
        INSERT INTO accounts VALUES ('a1', 'Assets', 'ASSET', 'c1', NULL, NULL, NULL, 0, 0);
        INSERT INTO accounts VALUES ('a2', 'Expenses', 'EXPENSE', 'c1', NULL, NULL, NULL, 0, 0);
        INSERT INTO transactions VALUES ('t1', 'c1', '1', '20261101105900', '20261101105900', 'Grocery');
        INSERT INTO splits VALUES ('s1', 't1', 'a1', '', '', 'n', NULL, -10000, 100, -10000, 100, NULL);
        INSERT INTO splits VALUES ('s2', 't1', 'a2', '', '', 'n', NULL, 10000, 100, 10000, 100, NULL);
    "#).unwrap();

    let bytes1 = std::fs::read(temp_db1.path()).unwrap();
    let boundary = "---------------------------11223344556677889900";
    let mut body1 = Vec::new();
    body1.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
    body1.extend_from_slice(b"Content-Disposition: form-data; name=\"file\"; filename=\"book1.gnucash\"\r\nContent-Type: application/x-sqlite3\r\n\r\n");
    body1.extend_from_slice(&bytes1);
    body1.extend_from_slice(format!("\r\n--{}--\r\n", boundary).as_bytes());

    let app = create_app(pool.clone());
    let resp = app.oneshot(
        Request::builder()
            .method("POST")
            .uri("/api/import/gnucash-sqlite?wipe=false")
            .header("authorization", format!("Bearer {}", user_token))
            .header("content-type", format!("multipart/form-data; boundary={}", boundary))
            .body(Body::from(body1))
            .unwrap()
    ).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // Modify SQLite DB: change amount from 100 to 120 and use new split guid s3
    let temp_db2 = tempfile::NamedTempFile::new().unwrap();
    let conn2 = rusqlite::Connection::open(temp_db2.path()).unwrap();
    conn2.execute_batch(r#"
        CREATE TABLE accounts (guid TEXT PRIMARY KEY, name TEXT, account_type TEXT, commodity_guid TEXT, parent_guid TEXT, code TEXT, description TEXT, hidden INT, placeholder INT);
        CREATE TABLE transactions (guid TEXT PRIMARY KEY, currency_guid TEXT, num TEXT, post_date TEXT, enter_date TEXT, description TEXT);
        CREATE TABLE splits (guid TEXT PRIMARY KEY, tx_guid TEXT, account_guid TEXT, memo TEXT, action TEXT, reconcile_state TEXT, reconcile_date TEXT, value_num INT, value_denom INT, quantity_num INT, quantity_denom INT, lot_guid TEXT);
        CREATE TABLE commodities (guid TEXT PRIMARY KEY, namespace TEXT, mnemonic TEXT, fullname TEXT, cusip TEXT, fraction INT, quote_flag INT, quote_source TEXT);

        INSERT INTO commodities VALUES ('c1', 'CURRENCY', 'INR', 'INR', '', 100, 1, 'currency');
        INSERT INTO accounts VALUES ('a1', 'Assets', 'ASSET', 'c1', NULL, NULL, NULL, 0, 0);
        INSERT INTO accounts VALUES ('a2', 'Expenses', 'EXPENSE', 'c1', NULL, NULL, NULL, 0, 0);
        INSERT INTO transactions VALUES ('t1', 'c1', '1', '20261101105900', '20261101105900', 'Grocery Updated');
        INSERT INTO splits VALUES ('s1', 't1', 'a1', '', '', 'n', NULL, -12000, 100, -12000, 100, NULL);
        INSERT INTO splits VALUES ('s3', 't1', 'a2', '', '', 'n', NULL, 12000, 100, 12000, 100, NULL);
    "#).unwrap();

    let bytes2 = std::fs::read(temp_db2.path()).unwrap();
    let mut body2 = Vec::new();
    body2.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
    body2.extend_from_slice(b"Content-Disposition: form-data; name=\"file\"; filename=\"book2.gnucash\"\r\nContent-Type: application/x-sqlite3\r\n\r\n");
    body2.extend_from_slice(&bytes2);
    body2.extend_from_slice(format!("\r\n--{}--\r\n", boundary).as_bytes());

    let app = create_app(pool.clone());
    let resp = app.oneshot(
        Request::builder()
            .method("POST")
            .uri("/api/import/gnucash-sqlite?wipe=false")
            .header("authorization", format!("Bearer {}", user_token))
            .header("content-type", format!("multipart/form-data; boundary={}", boundary))
            .body(Body::from(body2))
            .unwrap()
    ).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // Verify there are exactly 2 splits in total, with amounts 120 and -120
    let split_amounts: Vec<rust_decimal::Decimal> = sqlx::query_scalar(
        "SELECT amount FROM splits WHERE transaction_id IN (SELECT id FROM transactions WHERE user_id = $1) ORDER BY amount ASC"
    )
    .bind(user_id)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(split_amounts.len(), 2, "Stale split s2 must have been deleted, leaving exactly 2 splits");
    assert_eq!(split_amounts[0], rust_decimal_macros::dec!(-120.00));
    assert_eq!(split_amounts[1], rust_decimal_macros::dec!(120.00));
}

#[tokio::test]
async fn test_importer_ignores_scheduled_transaction_templates() {
    // Issue #7: Importer ignores scheduled-transaction templates
    let pool = get_test_pool().await;
    let user_id = uuid::Uuid::new_v4();
    let username = format!("sxtest_{}", &user_id.to_string()[..8]);
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
        CREATE TABLE books (guid TEXT PRIMARY KEY, root_account_guid TEXT, root_template_guid TEXT);
        CREATE TABLE accounts (guid TEXT PRIMARY KEY, name TEXT, account_type TEXT, commodity_guid TEXT, parent_guid TEXT, code TEXT, description TEXT, hidden INT, placeholder INT);
        CREATE TABLE transactions (guid TEXT PRIMARY KEY, currency_guid TEXT, num TEXT, post_date TEXT, enter_date TEXT, description TEXT);
        CREATE TABLE splits (guid TEXT PRIMARY KEY, tx_guid TEXT, account_guid TEXT, memo TEXT, action TEXT, reconcile_state TEXT, reconcile_date TEXT, value_num INT, value_denom INT, quantity_num INT, quantity_denom INT, lot_guid TEXT);
        CREATE TABLE commodities (guid TEXT PRIMARY KEY, namespace TEXT, mnemonic TEXT, fullname TEXT, cusip TEXT, fraction INT, quote_flag INT, quote_source TEXT);

        INSERT INTO books VALUES ('b1', 'real_root', 'template_root');
        INSERT INTO commodities VALUES ('c1', 'CURRENCY', 'INR', 'INR', '', 100, 1, 'currency');
        INSERT INTO accounts VALUES ('real_root', 'Root Account', 'ROOT', 'c1', NULL, NULL, NULL, 0, 0);
        INSERT INTO accounts VALUES ('a1', 'Checking', 'BANK', 'c1', 'real_root', NULL, NULL, 0, 0);
        INSERT INTO accounts VALUES ('a2', 'Expenses', 'EXPENSE', 'c1', 'real_root', NULL, NULL, 0, 0);

        -- Template tree
        INSERT INTO accounts VALUES ('template_root', 'Template Root', 'ROOT', 'c1', NULL, NULL, NULL, 0, 0);
        INSERT INTO accounts VALUES ('sx_child_guid_1234567890abcdef', 'SX Account', 'BANK', 'c1', 'template_root', NULL, NULL, 0, 0);

        -- Real transaction
        INSERT INTO transactions VALUES ('t_real', 'c1', '', '20261101105900', '20261101105900', 'Real Transaction');
        INSERT INTO splits VALUES ('s1', 't_real', 'a1', '', '', 'n', NULL, -5000, 100, -5000, 100, NULL);
        INSERT INTO splits VALUES ('s2', 't_real', 'a2', '', '', 'n', NULL, 5000, 100, 5000, 100, NULL);

        -- Template transaction
        INSERT INTO transactions VALUES ('t_template', 'c1', '', '20261101105900', '20261101105900', 'Template Transaction');
        INSERT INTO splits VALUES ('s_tmp1', 't_template', 'sx_child_guid_1234567890abcdef', '', '', 'n', NULL, 0, 100, 0, 100, NULL);
        INSERT INTO splits VALUES ('s_tmp2', 't_template', 'a2', '', '', 'n', NULL, 0, 100, 0, 100, NULL);
    "#).unwrap();

    let bytes = std::fs::read(temp_db.path()).unwrap();
    let boundary = "---------------------------44556677889900112233";
    let mut body = Vec::new();
    body.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
    body.extend_from_slice(b"Content-Disposition: form-data; name=\"file\"; filename=\"sx.gnucash\"\r\nContent-Type: application/x-sqlite3\r\n\r\n");
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

    // Verify template account was NOT imported
    let sx_accts: Vec<String> = sqlx::query_scalar("SELECT name FROM accounts WHERE user_id = $1")
        .bind(user_id)
        .fetch_all(&pool)
        .await
        .unwrap();
    assert!(!sx_accts.contains(&"SX Account".to_string()), "Template accounts must be excluded");
    assert!(!sx_accts.contains(&"Template Root".to_string()), "Template root must be excluded");

    // Verify only the real transaction exists
    let tx_descs: Vec<String> = sqlx::query_scalar("SELECT description FROM transactions WHERE user_id = $1")
        .bind(user_id)
        .fetch_all(&pool)
        .await
        .unwrap();
    assert_eq!(tx_descs, vec!["Real Transaction".to_string()]);
}

#[tokio::test]
async fn test_stock_foreign_commodity_import_and_export() {
    // Issue #9: Stock / ISIN mnemonic accounts (>10 chars) import and export cleanly
    let pool = get_test_pool().await;
    let user_id = uuid::Uuid::new_v4();
    let username = format!("stock_{}", &user_id.to_string()[..8]);
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
        INSERT INTO commodities VALUES ('c_fund', 'FUND', 'INF209K01VA3', 'Aditya Birla Mutual Fund', 'INF209K01VA3', 1000, 1, 'financequote');

        INSERT INTO accounts VALUES ('a_bank', 'Bank Account', 'BANK', 'c_inr', NULL, NULL, NULL, 0, 0);
        INSERT INTO accounts VALUES ('a_stock', 'Mutual Fund Investment', 'STOCK', 'c_fund', NULL, NULL, NULL, 0, 0);

        INSERT INTO transactions VALUES ('t_buy', 'c_inr', '', '20261101105900', '20261101105900', 'SIP Purchase');
        INSERT INTO splits VALUES ('s1', 't_buy', 'a_bank', '', '', 'n', NULL, -150000, 100, -150000, 100, NULL);
        INSERT INTO splits VALUES ('s2', 't_buy', 'a_stock', '', '', 'n', NULL, 150000, 100, 10000, 1000, NULL);
    "#).unwrap();

    let bytes = std::fs::read(temp_db.path()).unwrap();
    let boundary = "---------------------------99887766554433221100";
    let mut body = Vec::new();
    body.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
    body.extend_from_slice(b"Content-Disposition: form-data; name=\"file\"; filename=\"stock.gnucash\"\r\nContent-Type: application/x-sqlite3\r\n\r\n");
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
    assert_eq!(resp.status(), StatusCode::OK, "Importing account with 12-char ISIN must succeed without VARCHAR truncation");

    let stock_currency: String = sqlx::query_scalar("SELECT currency FROM accounts WHERE name = 'Mutual Fund Investment' AND user_id = $1")
        .bind(user_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(stock_currency, "INF209K01VA3");

    // Verify account balance reflects investment amount (1500), not units (Issue #21 scope decision)
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
    let stock_acc = accs.iter().find(|a| a["name"] == "Mutual Fund Investment").unwrap();
    let bal: rust_decimal::Decimal = stock_acc["balance"].as_str().unwrap().parse().unwrap();
    assert_eq!(bal, rust_decimal::Decimal::from(1500), "Stock account balance must show investment amount, not units (Issue #21)");

    // Export again and verify FUND namespace and faithful quantities
    let app = create_app(pool.clone());
    let resp = app.oneshot(
        Request::builder()
            .method("GET")
            .uri("/api/export/gnucash-sqlite")
            .header("authorization", format!("Bearer {}", user_token))
            .body(Body::empty())
            .unwrap()
    ).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let exported_bytes = resp.into_body().collect().await.unwrap().to_bytes();

    let temp_exp = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(temp_exp.path(), &exported_bytes).unwrap();
    let exp_conn = rusqlite::Connection::open(temp_exp.path()).unwrap();
    let (namespace, fraction): (String, i64) = exp_conn.query_row(
        "SELECT namespace, fraction FROM commodities WHERE mnemonic = 'INF209K01VA3'",
        [],
        |row| Ok((row.get(0)?, row.get(1)?))
    ).unwrap();
    assert_eq!(namespace, "FUND", "Exported commodity with >3 chars must have namespace FUND");
    assert_eq!(fraction, 1000, "Exported commodity fraction must be preserved");

    let (val_num, val_denom, qty_num, qty_denom): (i64, i64, i64, i64) = exp_conn.query_row(
        "SELECT s.value_num, s.value_denom, s.quantity_num, s.quantity_denom
         FROM splits s
         JOIN accounts a ON s.account_guid = a.guid
         WHERE a.name = 'Mutual Fund Investment'",
        [],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
    ).unwrap();
    assert_eq!(val_num, 150000);
    assert_eq!(val_denom, 100);
    assert_eq!(qty_num, 10000, "Split quantity_num must be preserved on export");
    assert_eq!(qty_denom, 1000, "Split quantity_denom must be preserved on export");
}

#[tokio::test]
async fn test_issue_19_reject_unbalanced_import() {
    // Issue #19: Importer must strictly reject transactions unbalanced by even 0.01
    let pool = get_test_pool().await;
    let user_id = uuid::Uuid::new_v4();
    let username = format!("unbal_{}", &user_id.to_string()[..8]);
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
        INSERT INTO accounts VALUES ('a1', 'Assets', 'BANK', 'c_inr', NULL, NULL, NULL, 0, 0);
        INSERT INTO accounts VALUES ('a2', 'Expenses', 'EXPENSE', 'c_inr', NULL, NULL, NULL, 0, 0);

        INSERT INTO transactions VALUES ('t1', 'c_inr', '', '20261101105900', '20261101105900', 'Unbalanced tx');
        -- Off by 0.01: -50.00 and +50.01
        INSERT INTO splits VALUES ('s1', 't1', 'a1', '', '', 'n', NULL, -5000, 100, -5000, 100, NULL);
        INSERT INTO splits VALUES ('s2', 't1', 'a2', '', '', 'n', NULL, 5001, 100, 5001, 100, NULL);
    "#).unwrap();

    let bytes = std::fs::read(temp_db.path()).unwrap();
    let boundary = "---------------------------11223344556677889900";
    let mut body = Vec::new();
    body.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
    body.extend_from_slice(b"Content-Disposition: form-data; name=\"file\"; filename=\"unbalanced.gnucash\"\r\nContent-Type: application/x-sqlite3\r\n\r\n");
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

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST, "Importing transaction off by 0.01 must return 400 Bad Request");
    let resp_bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let resp_json: serde_json::Value = serde_json::from_slice(&resp_bytes).unwrap();
    assert!(resp_json["error"].as_str().unwrap().contains("Double-entry violation"));
}

#[tokio::test]
async fn test_issue_18_prevent_parent_cycle_hang() {
    // Issue #18: Importer detects and breaks parent cycles; update_account cycle check bounds recursion
    let pool = get_test_pool().await;
    let user_id = uuid::Uuid::new_v4();
    let username = format!("cycle_{}", &user_id.to_string()[..8]);
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
        -- Mutual cycle: Cycle A has parent Cycle B; Cycle B has parent Cycle A
        INSERT INTO accounts VALUES ('a_cycle1', 'Cycle A', 'BANK', 'c_inr', 'a_cycle2', NULL, NULL, 0, 0);
        INSERT INTO accounts VALUES ('a_cycle2', 'Cycle B', 'BANK', 'c_inr', 'a_cycle1', NULL, NULL, 0, 0);
    "#).unwrap();

    let bytes = std::fs::read(temp_db.path()).unwrap();
    let boundary = "---------------------------55443322110099887766";
    let mut body = Vec::new();
    body.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
    body.extend_from_slice(b"Content-Disposition: form-data; name=\"file\"; filename=\"cycle.gnucash\"\r\nContent-Type: application/x-sqlite3\r\n\r\n");
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

    assert_eq!(resp.status(), StatusCode::OK, "Import must break cycle without failing");

    // Verify at least one of the two accounts has parent_id IS NULL in Postgres
    let root_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM accounts WHERE user_id = $1 AND parent_id IS NULL AND name IN ('Cycle A', 'Cycle B')"
    )
    .bind(user_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(root_count >= 1, "At least one cyclic account must have its parent broken to NULL");

    // Query account tree: must succeed and not loop infinitely
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
}

#[tokio::test]
async fn test_issue_1_default_user_password123_fails() {
    // Issue #1: Verify that login with default kishan/password123 returns 401 Unauthorized
    let pool = get_test_pool().await;
    let app = create_app(pool.clone());

    let login_payload = json!({
        "username": "kishan",
        "password": "password123"
    });

    let resp = app.oneshot(
        Request::builder()
            .method("POST")
            .uri("/api/auth/login")
            .header("content-type", "application/json")
            .body(Body::from(login_payload.to_string()))
            .unwrap()
    ).await.unwrap();

    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED, "Default password123 must be rejected");
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
async fn test_cross_tenant_endpoint_isolation() {
    // SEC-10: Every query on accounts, transactions and splits is scoped to the authenticated user.
    // No user can read, modify, or infer the existence of another user's data.
    let pool = get_test_pool().await;

    // 1. Create User A
    let user_a_id = uuid::Uuid::new_v4();
    let username_a = format!("isola_{}", &user_a_id.to_string()[..8]);
    let email_a = format!("{}@example.com", username_a);
    sqlx::query("INSERT INTO users (id, username, email, password_hash) VALUES ($1, $2, $3, 'hash')")
        .bind(user_a_id)
        .bind(&username_a)
        .bind(&email_a)
        .execute(&pool)
        .await
        .unwrap();
    let token_a = gnucash_backend::auth::create_jwt(user_a_id, &username_a, &email_a, 0).unwrap();

    // 2. Create User B
    let user_b_id = uuid::Uuid::new_v4();
    let username_b = format!("isolb_{}", &user_b_id.to_string()[..8]);
    let email_b = format!("{}@example.com", username_b);
    sqlx::query("INSERT INTO users (id, username, email, password_hash) VALUES ($1, $2, $3, 'hash')")
        .bind(user_b_id)
        .bind(&username_b)
        .bind(&email_b)
        .execute(&pool)
        .await
        .unwrap();
    let token_b = gnucash_backend::auth::create_jwt(user_b_id, &username_b, &email_b, 0).unwrap();

    // 3. User A creates accounts and a transaction
    let app = create_app(pool.clone());
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/accounts")
                .header("authorization", format!("Bearer {}", token_a))
                .header("content-type", "application/json")
                .body(Body::from(json!({
                    "name": "User A Bank",
                    "account_type": "bank",
                    "currency": "INR"
                }).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let acc1_data: Value = serde_json::from_slice(&resp.into_body().collect().await.unwrap().to_bytes()).unwrap();
    let acc1_id = acc1_data["id"].as_str().unwrap();

    let app = create_app(pool.clone());
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/accounts")
                .header("authorization", format!("Bearer {}", token_a))
                .header("content-type", "application/json")
                .body(Body::from(json!({
                    "name": "User A Expense",
                    "account_type": "expense",
                    "currency": "INR"
                }).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let acc2_data: Value = serde_json::from_slice(&resp.into_body().collect().await.unwrap().to_bytes()).unwrap();
    let acc2_id = acc2_data["id"].as_str().unwrap();

    let app = create_app(pool.clone());
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/transactions")
                .header("authorization", format!("Bearer {}", token_a))
                .header("content-type", "application/json")
                .body(Body::from(json!({
                    "post_date": "2026-11-16",
                    "description": "User A Secret Transaction",
                    "splits": [
                        { "account_id": acc1_id, "amount": "-100.00" },
                        { "account_id": acc2_id, "amount": "100.00" }
                    ]
                }).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let tx_data: Value = serde_json::from_slice(&resp.into_body().collect().await.unwrap().to_bytes()).unwrap();
    let tx_id = tx_data["id"].as_str().unwrap();

    // 4. User B attempts to access User A's accounts via GET, PUT, DELETE -> all 404
    let app = create_app(pool.clone());
    let resp = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(&format!("/api/accounts/{}", acc1_id))
                .header("authorization", format!("Bearer {}", token_b))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND, "User B must not read User A's account");

    let app = create_app(pool.clone());
    let resp = app
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(&format!("/api/accounts/{}", acc1_id))
                .header("authorization", format!("Bearer {}", token_b))
                .header("content-type", "application/json")
                .body(Body::from(json!({ "name": "Hacked Name" }).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND, "User B must not modify User A's account");

    let app = create_app(pool.clone());
    let resp = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(&format!("/api/accounts/{}", acc1_id))
                .header("authorization", format!("Bearer {}", token_b))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND, "User B must not delete User A's account");

    // 5. User B attempts to access User A's account register -> 404
    let app = create_app(pool.clone());
    let resp = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(&format!("/api/accounts/{}/register", acc1_id))
                .header("authorization", format!("Bearer {}", token_b))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND, "User B must not read User A's account register");

    // 6. User B attempts to access User A's transaction via GET, PUT, DELETE -> all 404
    let app = create_app(pool.clone());
    let resp = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(&format!("/api/transactions/{}", tx_id))
                .header("authorization", format!("Bearer {}", token_b))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND, "User B must not read User A's transaction");

    let app = create_app(pool.clone());
    let resp = app
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(&format!("/api/transactions/{}", tx_id))
                .header("authorization", format!("Bearer {}", token_b))
                .header("content-type", "application/json")
                .body(Body::from(json!({ "description": "Hacked Transaction" }).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND, "User B must not modify User A's transaction");

    let app = create_app(pool.clone());
    let resp = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(&format!("/api/transactions/{}", tx_id))
                .header("authorization", format!("Bearer {}", token_b))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND, "User B must not delete User A's transaction");
}

#[tokio::test]
async fn test_reset_data_endpoint_wipes_user_records_only() {
    // DATA-1: Reset Entire Database (POST /api/reset-data) permanently deletes all of the user's
    // accounts and transactions, leaving 0 of each. Other users are unaffected.
    let pool = get_test_pool().await;

    // 1. Create User A and User B
    let user_a_id = uuid::Uuid::new_v4();
    let username_a = format!("reseta_{}", &user_a_id.to_string()[..8]);
    let email_a = format!("{}@example.com", username_a);
    sqlx::query("INSERT INTO users (id, username, email, password_hash) VALUES ($1, $2, $3, 'hash')")
        .bind(user_a_id)
        .bind(&username_a)
        .bind(&email_a)
        .execute(&pool)
        .await
        .unwrap();
    let token_a = gnucash_backend::auth::create_jwt(user_a_id, &username_a, &email_a, 0).unwrap();

    let user_b_id = uuid::Uuid::new_v4();
    let username_b = format!("resetb_{}", &user_b_id.to_string()[..8]);
    let email_b = format!("{}@example.com", username_b);
    sqlx::query("INSERT INTO users (id, username, email, password_hash) VALUES ($1, $2, $3, 'hash')")
        .bind(user_b_id)
        .bind(&username_b)
        .bind(&email_b)
        .execute(&pool)
        .await
        .unwrap();

    // 2. Insert records for both users
    let acc_a = uuid::Uuid::new_v4();
    sqlx::query("INSERT INTO accounts (id, name, account_type, user_id) VALUES ($1, 'User A Bank', 'bank', $2)")
        .bind(acc_a)
        .bind(user_a_id)
        .execute(&pool)
        .await
        .unwrap();

    let acc_b = uuid::Uuid::new_v4();
    sqlx::query("INSERT INTO accounts (id, name, account_type, user_id) VALUES ($1, 'User B Bank', 'bank', $2)")
        .bind(acc_b)
        .bind(user_b_id)
        .execute(&pool)
        .await
        .unwrap();

    let tx_a = uuid::Uuid::new_v4();
    sqlx::query("INSERT INTO transactions (id, post_date, description, user_id) VALUES ($1, '2026-11-16', 'Tx A', $2)")
        .bind(tx_a)
        .bind(user_a_id)
        .execute(&pool)
        .await
        .unwrap();

    let tx_b = uuid::Uuid::new_v4();
    sqlx::query("INSERT INTO transactions (id, post_date, description, user_id) VALUES ($1, '2026-11-16', 'Tx B', $2)")
        .bind(tx_b)
        .bind(user_b_id)
        .execute(&pool)
        .await
        .unwrap();

    // 3. User A calls POST /api/reset-data
    let app = create_app(pool.clone());
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/reset-data")
                .header("authorization", format!("Bearer {}", token_a))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["success"], true);

    // 4. Verify User A has 0 accounts and 0 transactions
    let count_acc_a: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM accounts WHERE user_id = $1")
        .bind(user_a_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count_acc_a, 0, "User A must have 0 accounts after reset");

    let count_tx_a: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM transactions WHERE user_id = $1")
        .bind(user_a_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count_tx_a, 0, "User A must have 0 transactions after reset");

    // 5. Verify User B records are completely untouched
    let count_acc_b: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM accounts WHERE user_id = $1")
        .bind(user_b_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count_acc_b, 1, "User B accounts must not be deleted by User A reset");

    let count_tx_b: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM transactions WHERE user_id = $1")
        .bind(user_b_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count_tx_b, 1, "User B transactions must not be deleted by User A reset");
}

#[tokio::test]
async fn test_split_count_and_scale_boundaries() {
    // ACC-1: Every transaction has 2-100 splits.
    // ACC-3: Split amounts have at most 2 decimal places.
    let pool = get_test_pool().await;
    let user_id = uuid::Uuid::new_v4();
    let username = format!("bounds_{}", &user_id.to_string()[..8]);
    let email = format!("{}@example.com", username);
    sqlx::query("INSERT INTO users (id, username, email, password_hash) VALUES ($1, $2, $3, 'hash')")
        .bind(user_id)
        .bind(&username)
        .bind(&email)
        .execute(&pool)
        .await
        .unwrap();
    let token = gnucash_backend::auth::create_jwt(user_id, &username, &email, 0).unwrap();

    let acc1 = uuid::Uuid::new_v4();
    let acc2 = uuid::Uuid::new_v4();
    sqlx::query("INSERT INTO accounts (id, name, account_type, user_id) VALUES ($1, 'Acct 1', 'bank', $3), ($2, 'Acct 2', 'expense', $3)")
        .bind(acc1)
        .bind(acc2)
        .bind(user_id)
        .execute(&pool)
        .await
        .unwrap();

    // 1. Split amount with 3 decimal places ("10.005" / "-10.005") -> 400 Bad Request (ACC-3)
    let app = create_app(pool.clone());
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/transactions")
                .header("authorization", format!("Bearer {}", token))
                .header("content-type", "application/json")
                .body(Body::from(json!({
                    "post_date": "2026-11-16",
                    "description": "3 Decimals Test",
                    "splits": [
                        { "account_id": acc1, "amount": "10.005" },
                        { "account_id": acc2, "amount": "-10.005" }
                    ]
                }).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST, "Split with >2 decimal places must be rejected");

    // 2. Transaction with 102 splits (> 100) -> 400 Bad Request (ACC-1)
    let mut splits = Vec::new();
    for _ in 0..51 {
        splits.push(json!({ "account_id": acc1, "amount": "1.00" }));
        splits.push(json!({ "account_id": acc2, "amount": "-1.00" }));
    }
    assert_eq!(splits.len(), 102);

    let app = create_app(pool.clone());
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/transactions")
                .header("authorization", format!("Bearer {}", token))
                .header("content-type", "application/json")
                .body(Body::from(json!({
                    "post_date": "2026-11-16",
                    "description": "Over 100 Splits Test",
                    "splits": splits
                }).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST, "Transaction with >100 splits must be rejected");
}

#[tokio::test]
async fn test_auth_and_input_length_boundaries() {
    // SEC-3: Usernames are at least 3 characters. Passwords are 6-72 characters.
    // TXN-1: Description 1-255 characters.
    let pool = get_test_pool().await;
    let jwt_secret = gnucash_backend::auth::get_jwt_secret();

    // 1. Username < 3 chars -> 400
    let app = create_app(pool.clone());
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/auth/signup")
                .header("content-type", "application/json")
                .header("x-admin-key", &jwt_secret)
                .body(Body::from(json!({
                    "username": "ab",
                    "email": "ab@example.com",
                    "password": "ValidPassword@123"
                }).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST, "Username < 3 characters must be rejected");

    // 2. Password < 6 chars -> 400
    let app = create_app(pool.clone());
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/auth/signup")
                .header("content-type", "application/json")
                .header("x-admin-key", &jwt_secret)
                .body(Body::from(json!({
                    "username": "valid_user_short_pw",
                    "email": "shortpw@example.com",
                    "password": "12345"
                }).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST, "Password < 6 characters must be rejected");

    // 3. Password > 72 chars -> 400
    let long_pw = "a".repeat(73);
    let app = create_app(pool.clone());
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/auth/signup")
                .header("content-type", "application/json")
                .header("x-admin-key", &jwt_secret)
                .body(Body::from(json!({
                    "username": "valid_user_long_pw",
                    "email": "longpw@example.com",
                    "password": long_pw
                }).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST, "Password > 72 characters must be rejected");

    // 4. Transaction with empty description -> 400 (TXN-1)
    let user_id = uuid::Uuid::new_v4();
    let username = format!("txlen_{}", &user_id.to_string()[..8]);
    let email = format!("{}@example.com", username);
    sqlx::query("INSERT INTO users (id, username, email, password_hash) VALUES ($1, $2, $3, 'hash')")
        .bind(user_id)
        .bind(&username)
        .bind(&email)
        .execute(&pool)
        .await
        .unwrap();
    let token = gnucash_backend::auth::create_jwt(user_id, &username, &email, 0).unwrap();

    let acc1 = uuid::Uuid::new_v4();
    let acc2 = uuid::Uuid::new_v4();
    sqlx::query("INSERT INTO accounts (id, name, account_type, user_id) VALUES ($1, 'Acct 1', 'bank', $3), ($2, 'Acct 2', 'expense', $3)")
        .bind(acc1)
        .bind(acc2)
        .bind(user_id)
        .execute(&pool)
        .await
        .unwrap();

    let app = create_app(pool.clone());
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/transactions")
                .header("authorization", format!("Bearer {}", token))
                .header("content-type", "application/json")
                .body(Body::from(json!({
                    "post_date": "2026-11-16",
                    "description": "   ",
                    "splits": [
                        { "account_id": acc1, "amount": "10.00" },
                        { "account_id": acc2, "amount": "-10.00" }
                    ]
                }).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST, "Empty transaction description must be rejected");
}

#[tokio::test]
async fn test_export_versions_table_and_neutral_time() {
    // EXP-2: The versions table contains:
    // Gnucash = 5000017, Gnucash-Resave = 19920, transactions = 4, splits = 5, accounts = 1, commodities = 1, books = 1.
    // EXP-3: Post dates are written at GnuCash neutral time (YYYYMMDD105900).
    let pool = get_test_pool().await;
    let user_id = uuid::Uuid::new_v4();
    let username = format!("expver_{}", &user_id.to_string()[..8]);
    let email = format!("{}@example.com", username);
    sqlx::query("INSERT INTO users (id, username, email, password_hash) VALUES ($1, $2, $3, 'hash')")
        .bind(user_id)
        .bind(&username)
        .bind(&email)
        .execute(&pool)
        .await
        .unwrap();
    let token = gnucash_backend::auth::create_jwt(user_id, &username, &email, 0).unwrap();

    // Create accounts and a transaction for user
    let acc1 = uuid::Uuid::new_v4();
    let acc2 = uuid::Uuid::new_v4();
    sqlx::query("INSERT INTO accounts (id, name, account_type, user_id) VALUES ($1, 'Export Bank', 'bank', $3), ($2, 'Export Expense', 'expense', $3)")
        .bind(acc1)
        .bind(acc2)
        .bind(user_id)
        .execute(&pool)
        .await
        .unwrap();

    let tx1 = uuid::Uuid::new_v4();
    sqlx::query("INSERT INTO transactions (id, post_date, description, user_id) VALUES ($1, '2026-11-16', 'Export Date Test', $2)")
        .bind(tx1)
        .bind(user_id)
        .execute(&pool)
        .await
        .unwrap();

    sqlx::query("INSERT INTO splits (id, transaction_id, account_id, amount, quantity, reconciled_state) VALUES ($1, $2, $3, -50.00, -50.00, 'n'), ($4, $2, $5, 50.00, 50.00, 'n')")
        .bind(uuid::Uuid::new_v4())
        .bind(tx1)
        .bind(acc1)
        .bind(uuid::Uuid::new_v4())
        .bind(acc2)
        .execute(&pool)
        .await
        .unwrap();

    // Export database
    let app = create_app(pool.clone());
    let resp = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/export/gnucash-sqlite")
                .header("authorization", format!("Bearer {}", token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();

    let temp_file = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(temp_file.path(), &bytes).unwrap();
    let conn = rusqlite::Connection::open(temp_file.path()).unwrap();

    // Verify versions table per EXP-2
    let mut stmt = conn.prepare("SELECT table_name, table_version FROM versions").unwrap();
    let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?))).unwrap();
    let versions: std::collections::HashMap<String, i64> = rows.filter_map(|r| r.ok()).collect();

    assert_eq!(versions.get("Gnucash"), Some(&5000017), "versions.Gnucash must be 5000017");
    assert_eq!(versions.get("Gnucash-Resave"), Some(&19920), "versions.Gnucash-Resave must be 19920");
    assert_eq!(versions.get("transactions"), Some(&4), "versions.transactions must be 4");
    assert_eq!(versions.get("splits"), Some(&5), "versions.splits must be 5");
    assert_eq!(versions.get("accounts"), Some(&1), "versions.accounts must be 1");
    assert_eq!(versions.get("commodities"), Some(&1), "versions.commodities must be 1");
    assert_eq!(versions.get("books"), Some(&1), "versions.books must be 1");

    // Verify post_date is written at GnuCash neutral time YYYYMMDD105900 per EXP-3
    let post_date: String = conn.query_row(
        "SELECT post_date FROM transactions LIMIT 1",
        [],
        |r| r.get(0)
    ).unwrap();
    assert!(post_date.ends_with("105900"), "post_date '{}' must end with neutral time 105900", post_date);
    assert_eq!(post_date, "20261116105900");
}

#[tokio::test]
async fn test_import_reconciled_state_normalization() {
    // IMP-8: GnuCash reconcile states other than n/c/y (e.g. f, v) are imported as n.
    let pool = get_test_pool().await;
    let user_id = uuid::Uuid::new_v4();
    let username = format!("imp_rec_{}", &user_id.to_string()[..8]);
    let email = format!("{}@example.com", username);
    sqlx::query("INSERT INTO users (id, username, email, password_hash) VALUES ($1, $2, $3, 'hash')")
        .bind(user_id)
        .bind(&username)
        .bind(&email)
        .execute(&pool)
        .await
        .unwrap();
    let token = gnucash_backend::auth::create_jwt(user_id, &username, &email, 0).unwrap();

    let temp_db = tempfile::NamedTempFile::new().unwrap();
    let conn = rusqlite::Connection::open(temp_db.path()).unwrap();
    conn.execute_batch(r#"
        CREATE TABLE accounts (guid TEXT PRIMARY KEY, name TEXT, account_type TEXT, commodity_guid TEXT, parent_guid TEXT, code TEXT, description TEXT, hidden INT, placeholder INT);
        CREATE TABLE transactions (guid TEXT PRIMARY KEY, currency_guid TEXT, num TEXT, post_date TEXT, enter_date TEXT, description TEXT);
        CREATE TABLE splits (guid TEXT PRIMARY KEY, tx_guid TEXT, account_guid TEXT, memo TEXT, action TEXT, reconcile_state TEXT, reconcile_date TEXT, value_num INT, value_denom INT, quantity_num INT, quantity_denom INT, lot_guid TEXT);
        CREATE TABLE commodities (guid TEXT PRIMARY KEY, namespace TEXT, mnemonic TEXT, fullname TEXT, cusip TEXT, fraction INT, quote_flag INT, quote_source TEXT);

        INSERT INTO commodities VALUES ('c_inr', 'CURRENCY', 'INR', 'INR', '', 100, 1, 'currency');
        INSERT INTO accounts VALUES ('a1', 'Bank Account', 'BANK', 'c_inr', NULL, NULL, NULL, 0, 0);
        INSERT INTO accounts VALUES ('a2', 'Expense Account', 'EXPENSE', 'c_inr', NULL, NULL, NULL, 0, 0);

        INSERT INTO transactions VALUES ('tx_rec', 'c_inr', '', '20261101105900', '20261101105900', 'Reconcile Normalization Tx');
        -- Split 1 has state 'f' (frozen), Split 2 has state 'v' (voided)
        INSERT INTO splits VALUES ('s1', 'tx_rec', 'a1', '', '', 'f', NULL, -2500, 100, -2500, 100, NULL);
        INSERT INTO splits VALUES ('s2', 'tx_rec', 'a2', '', '', 'v', NULL, 2500, 100, 2500, 100, NULL);
    "#).unwrap();

    let bytes = std::fs::read(temp_db.path()).unwrap();
    let boundary = "---------------------------recstateboundary12345";
    let mut body = Vec::new();
    body.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
    body.extend_from_slice(b"Content-Disposition: form-data; name=\"file\"; filename=\"reconcile_states.gnucash\"\r\nContent-Type: application/x-sqlite3\r\n\r\n");
    body.extend_from_slice(&bytes);
    body.extend_from_slice(format!("\r\n--{}--\r\n", boundary).as_bytes());

    let app = create_app(pool.clone());
    let resp = app.oneshot(
        Request::builder()
            .method("POST")
            .uri("/api/import/gnucash-sqlite?wipe=true")
            .header("authorization", format!("Bearer {}", token))
            .header("content-type", format!("multipart/form-data; boundary={}", boundary))
            .body(Body::from(body))
            .unwrap()
    ).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // Verify both splits in Postgres are normalized to 'n'
    let states: Vec<String> = sqlx::query_scalar(
        "SELECT reconciled_state FROM splits s JOIN transactions t ON s.transaction_id = t.id WHERE t.user_id = $1"
    )
    .bind(user_id)
    .fetch_all(&pool)
    .await
    .unwrap();

    assert_eq!(states.len(), 2);
    for state in states {
        assert_eq!(state, "n", "Non-standard reconcile state ('f' or 'v') must be normalized to 'n'");
    }
}

#[tokio::test]
async fn test_issue_34_account_parent_type_compatibility() {
    // Issue #34: ACC-12 - Reject account parents of an incompatible type
    let pool = get_test_pool().await;
    let user_id = uuid::Uuid::new_v4();
    let username = format!("acc12_{}", &user_id.to_string()[..8]);
    let email = format!("{}@example.com", username);
    sqlx::query("INSERT INTO users (id, username, email, password_hash) VALUES ($1, $2, $3, 'hash')")
        .bind(user_id)
        .bind(&username)
        .bind(&email)
        .execute(&pool)
        .await
        .unwrap();
    let token = gnucash_backend::auth::create_jwt(user_id, &username, &email, 0).unwrap();

    // Helper closure to create an account
    let post_account = |payload: serde_json::Value| {
        let app = create_app(pool.clone());
        let tok = token.clone();
        async move {
            app.oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/accounts")
                    .header("authorization", format!("Bearer {}", tok))
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&payload).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap()
        }
    };

    // Helper closure to update an account
    let put_account = |id: &str, payload: serde_json::Value| {
        let app = create_app(pool.clone());
        let tok = token.clone();
        let uri = format!("/api/accounts/{}", id);
        async move {
            app.oneshot(
                Request::builder()
                    .method("PUT")
                    .uri(&uri)
                    .header("authorization", format!("Bearer {}", tok))
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&payload).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap()
        }
    };

    // Top-level accounts of any type must succeed (ACC-12)
    let res_asset = post_account(json!({ "name": "Asset Root", "account_type": "asset" })).await;
    assert_eq!(res_asset.status(), StatusCode::CREATED);
    let asset_root_id = serde_json::from_slice::<Value>(&res_asset.into_body().collect().await.unwrap().to_bytes()).unwrap()["id"].as_str().unwrap().to_string();

    let res_expense = post_account(json!({ "name": "Expense Root", "account_type": "expense" })).await;
    assert_eq!(res_expense.status(), StatusCode::CREATED);
    let expense_root_id = serde_json::from_slice::<Value>(&res_expense.into_body().collect().await.unwrap().to_bytes()).unwrap()["id"].as_str().unwrap().to_string();

    let res_equity = post_account(json!({ "name": "Equity Root", "account_type": "equity" })).await;
    assert_eq!(res_equity.status(), StatusCode::CREATED);
    let equity_root_id = serde_json::from_slice::<Value>(&res_equity.into_body().collect().await.unwrap().to_bytes()).unwrap()["id"].as_str().unwrap().to_string();

    let res_income = post_account(json!({ "name": "Income Root", "account_type": "income" })).await;
    assert_eq!(res_income.status(), StatusCode::CREATED);
    let income_root_id = serde_json::from_slice::<Value>(&res_income.into_body().collect().await.unwrap().to_bytes()).unwrap()["id"].as_str().unwrap().to_string();

    // Incompatible parent requests must return 400 Bad Request (ACC-12)
    // 1. Create asset account under expense parent -> 400
    let res = post_account(json!({
        "name": "Bad Asset Under Expense",
        "account_type": "asset",
        "parent_id": expense_root_id
    })).await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);

    // 2. Create liability account under expense parent -> 400
    let res = post_account(json!({
        "name": "Bad Liability Under Expense",
        "account_type": "liability",
        "parent_id": expense_root_id
    })).await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);

    // 3. Create expense account under asset parent -> 400
    let res = post_account(json!({
        "name": "Bad Expense Under Asset",
        "account_type": "expense",
        "parent_id": asset_root_id
    })).await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);

    // 4. Move income account under equity parent -> 400
    let res = put_account(&income_root_id, json!({
        "parent_id": equity_root_id
    })).await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);

    // 5. Change account type to asset while parent is expense -> 400
    let res_sub_expense = post_account(json!({
        "name": "Sub Expense",
        "account_type": "expense",
        "parent_id": expense_root_id
    })).await;
    assert_eq!(res_sub_expense.status(), StatusCode::CREATED);
    let sub_expense_id = serde_json::from_slice::<Value>(&res_sub_expense.into_body().collect().await.unwrap().to_bytes()).unwrap()["id"].as_str().unwrap().to_string();

    let res = put_account(&sub_expense_id, json!({
        "account_type": "asset"
    })).await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);

    // 6. Change parent type to asset while it has expense children -> 400
    let res = put_account(&expense_root_id, json!({
        "account_type": "asset"
    })).await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);

    // Compatible parent requests must succeed (ACC-12)
    // 1. liability under asset (both in balance_sheet group) -> 201
    let res = post_account(json!({
        "name": "Credit Card",
        "account_type": "liability",
        "parent_id": asset_root_id
    })).await;
    assert_eq!(res.status(), StatusCode::CREATED);

    // 2. bank under asset (both in balance_sheet group) -> 201
    let res = post_account(json!({
        "name": "Checking Account",
        "account_type": "bank",
        "parent_id": asset_root_id
    })).await;
    assert_eq!(res.status(), StatusCode::CREATED);

    // 3. expense under income (both in income_expense group) -> 201
    let res = post_account(json!({
        "name": "Operating Expense",
        "account_type": "expense",
        "parent_id": income_root_id
    })).await;
    assert_eq!(res.status(), StatusCode::CREATED);
}

#[tokio::test]
async fn test_issue_35_import_field_length_and_error_identification() {
    // Issue #35: IMP-11, TXN-1, ACC-11, ACC-2, SEC-13
    // Validate field lengths before DB transaction and name offending items in error responses
    let pool = get_test_pool().await;
    let user_id = uuid::Uuid::new_v4();
    let username = format!("imp11_{}", &user_id.to_string()[..8]);
    let email = format!("{}@example.com", username);
    sqlx::query("INSERT INTO users (id, username, email, password_hash) VALUES ($1, $2, $3, 'hash')")
        .bind(user_id)
        .bind(&username)
        .bind(&email)
        .execute(&pool)
        .await
        .unwrap();
    let token = gnucash_backend::auth::create_jwt(user_id, &username, &email, 0).unwrap();

    let upload_sqlite = |db_bytes: Vec<u8>| {
        let app = create_app(pool.clone());
        let tok = token.clone();
        async move {
            let boundary = "---------------------------boundaryimp11test";
            let mut body = Vec::new();
            body.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
            body.extend_from_slice(b"Content-Disposition: form-data; name=\"file\"; filename=\"test.gnucash\"\r\nContent-Type: application/x-sqlite3\r\n\r\n");
            body.extend_from_slice(&db_bytes);
            body.extend_from_slice(format!("\r\n--{}--\r\n", boundary).as_bytes());

            app.oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/import/gnucash-sqlite?wipe=true")
                    .header("authorization", format!("Bearer {}", tok))
                    .header("content-type", format!("multipart/form-data; boundary={}", boundary))
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap()
        }
    };

    // 1. Transaction description > 255 chars (IMP-11, TXN-1)
    let temp_db1 = tempfile::NamedTempFile::new().unwrap();
    let conn1 = rusqlite::Connection::open(temp_db1.path()).unwrap();
    let long_desc = format!("LongTx_{}", "X".repeat(250)); // 257 characters
    conn1.execute_batch(&format!(r#"
        CREATE TABLE accounts (guid TEXT PRIMARY KEY, name TEXT, account_type TEXT, commodity_guid TEXT, parent_guid TEXT, code TEXT, description TEXT, hidden INT, placeholder INT);
        CREATE TABLE transactions (guid TEXT PRIMARY KEY, currency_guid TEXT, num TEXT, post_date TEXT, enter_date TEXT, description TEXT);
        CREATE TABLE splits (guid TEXT PRIMARY KEY, tx_guid TEXT, account_guid TEXT, memo TEXT, action TEXT, reconcile_state TEXT, reconcile_date TEXT, value_num INT, value_denom INT, quantity_num INT, quantity_denom INT, lot_guid TEXT);
        CREATE TABLE commodities (guid TEXT PRIMARY KEY, namespace TEXT, mnemonic TEXT, fullname TEXT, cusip TEXT, fraction INT, quote_flag INT, quote_source TEXT);

        INSERT INTO commodities VALUES ('c1', 'CURRENCY', 'USD', 'USD', '', 100, 1, 'currency');
        INSERT INTO accounts VALUES ('a1', 'Assets', 'ASSET', 'c1', NULL, NULL, NULL, 0, 0);
        INSERT INTO accounts VALUES ('a2', 'Checking', 'BANK', 'c1', 'a1', NULL, NULL, 0, 0);
        INSERT INTO accounts VALUES ('a3', 'Groceries', 'EXPENSE', 'c1', NULL, NULL, NULL, 0, 0);

        INSERT INTO transactions VALUES ('tx_long', 'c1', '', '20261101100000', '20261101100000', '{long_desc}');
        INSERT INTO splits VALUES ('s1', 'tx_long', 'a2', '', '', 'n', NULL, -5000, 100, -5000, 100, NULL);
        INSERT INTO splits VALUES ('s2', 'tx_long', 'a3', '', '', 'n', NULL, 5000, 100, 5000, 100, NULL);
    "#)).unwrap();

    let bytes1 = std::fs::read(temp_db1.path()).unwrap();
    let resp1 = upload_sqlite(bytes1).await;
    assert_eq!(resp1.status(), StatusCode::BAD_REQUEST);
    let body_bytes1 = resp1.into_body().collect().await.unwrap().to_bytes();
    let json1: Value = serde_json::from_slice(&body_bytes1).unwrap();
    let err_msg1 = json1["error"].as_str().unwrap();

    // Verify error identifies post date, start of description, split accounts, and limit (IMP-11)
    assert!(err_msg1.contains("2026-11-01"), "Error must contain post date: {}", err_msg1);
    assert!(err_msg1.contains("LongTx_"), "Error must contain start of description: {}", err_msg1);
    assert!(err_msg1.contains("Assets:Checking") || err_msg1.contains("Groceries"), "Error must contain split accounts: {}", err_msg1);
    assert!(err_msg1.contains("255"), "Error must name limit 255: {}", err_msg1);
    // Verify no raw DB error text leaked (SEC-13)
    assert!(!err_msg1.contains("error returned from database"));
    assert!(!err_msg1.contains("character varying"));

    // Verify nothing written to DB
    let tx_count1: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM transactions WHERE user_id = $1")
        .bind(user_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(tx_count1, 0, "No transactions should be written on validation error");

    // 2. Account code > 50 chars (IMP-11, ACC-11)
    let temp_db2 = tempfile::NamedTempFile::new().unwrap();
    let conn2 = rusqlite::Connection::open(temp_db2.path()).unwrap();
    let long_code = "C".repeat(55); // 55 characters
    conn2.execute_batch(&format!(r#"
        CREATE TABLE accounts (guid TEXT PRIMARY KEY, name TEXT, account_type TEXT, commodity_guid TEXT, parent_guid TEXT, code TEXT, description TEXT, hidden INT, placeholder INT);
        CREATE TABLE transactions (guid TEXT PRIMARY KEY, currency_guid TEXT, num TEXT, post_date TEXT, enter_date TEXT, description TEXT);
        CREATE TABLE splits (guid TEXT PRIMARY KEY, tx_guid TEXT, account_guid TEXT, memo TEXT, action TEXT, reconcile_state TEXT, reconcile_date TEXT, value_num INT, value_denom INT, quantity_num INT, quantity_denom INT, lot_guid TEXT);
        CREATE TABLE commodities (guid TEXT PRIMARY KEY, namespace TEXT, mnemonic TEXT, fullname TEXT, cusip TEXT, fraction INT, quote_flag INT, quote_source TEXT);

        INSERT INTO commodities VALUES ('c1', 'CURRENCY', 'USD', 'USD', '', 100, 1, 'currency');
        INSERT INTO accounts VALUES ('a1', 'Assets', 'ASSET', 'c1', NULL, NULL, NULL, 0, 0);
        INSERT INTO accounts VALUES ('a2', 'Savings', 'BANK', 'c1', 'a1', '{long_code}', NULL, 0, 0);
    "#)).unwrap();

    let bytes2 = std::fs::read(temp_db2.path()).unwrap();
    let resp2 = upload_sqlite(bytes2).await;
    assert_eq!(resp2.status(), StatusCode::BAD_REQUEST);
    let body_bytes2 = resp2.into_body().collect().await.unwrap().to_bytes();
    let json2: Value = serde_json::from_slice(&body_bytes2).unwrap();
    let err_msg2 = json2["error"].as_str().unwrap();

    // Verify error names full account name and limit 50 (IMP-11)
    assert!(err_msg2.contains("Assets:Savings"), "Error must name full account name: {}", err_msg2);
    assert!(err_msg2.contains("50"), "Error must name limit 50: {}", err_msg2);
    assert!(!err_msg2.contains("error returned from database"));
    assert!(!err_msg2.contains("character varying"));

    let acc_count2: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM accounts WHERE user_id = $1")
        .bind(user_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(acc_count2, 0, "No accounts should be written on validation error");

    // 3. Unbalanced transaction names date, description, and accounts (ACC-2, IMP-11)
    let temp_db3 = tempfile::NamedTempFile::new().unwrap();
    let conn3 = rusqlite::Connection::open(temp_db3.path()).unwrap();
    conn3.execute_batch(r#"
        CREATE TABLE accounts (guid TEXT PRIMARY KEY, name TEXT, account_type TEXT, commodity_guid TEXT, parent_guid TEXT, code TEXT, description TEXT, hidden INT, placeholder INT);
        CREATE TABLE transactions (guid TEXT PRIMARY KEY, currency_guid TEXT, num TEXT, post_date TEXT, enter_date TEXT, description TEXT);
        CREATE TABLE splits (guid TEXT PRIMARY KEY, tx_guid TEXT, account_guid TEXT, memo TEXT, action TEXT, reconcile_state TEXT, reconcile_date TEXT, value_num INT, value_denom INT, quantity_num INT, quantity_denom INT, lot_guid TEXT);
        CREATE TABLE commodities (guid TEXT PRIMARY KEY, namespace TEXT, mnemonic TEXT, fullname TEXT, cusip TEXT, fraction INT, quote_flag INT, quote_source TEXT);

        INSERT INTO commodities VALUES ('c1', 'CURRENCY', 'USD', 'USD', '', 100, 1, 'currency');
        INSERT INTO accounts VALUES ('a1', 'Assets', 'ASSET', 'c1', NULL, NULL, NULL, 0, 0);
        INSERT INTO accounts VALUES ('a2', 'Cash', 'CASH', 'c1', 'a1', NULL, NULL, 0, 0);
        INSERT INTO accounts VALUES ('a3', 'Coffee', 'EXPENSE', 'c1', NULL, NULL, NULL, 0, 0);

        INSERT INTO transactions VALUES ('tx_unbalanced', 'c1', '', '20261102120000', '20261102120000', 'Unbalanced Coffee');
        INSERT INTO splits VALUES ('s1', 'tx_unbalanced', 'a2', '', '', 'n', NULL, -450, 100, -450, 100, NULL);
        INSERT INTO splits VALUES ('s2', 'tx_unbalanced', 'a3', '', '', 'n', NULL, 500, 100, 500, 100, NULL);
    "#).unwrap();

    let bytes3 = std::fs::read(temp_db3.path()).unwrap();
    let resp3 = upload_sqlite(bytes3).await;
    assert_eq!(resp3.status(), StatusCode::BAD_REQUEST);
    let body_bytes3 = resp3.into_body().collect().await.unwrap().to_bytes();
    let json3: Value = serde_json::from_slice(&body_bytes3).unwrap();
    let err_msg3 = json3["error"].as_str().unwrap();

    assert!(err_msg3.contains("Double-entry violation"), "Must maintain compatibility with Double-entry violation: {}", err_msg3);
    assert!(err_msg3.contains("tx_unbalanced"), "Must identify transaction GUID: {}", err_msg3);
    assert!(err_msg3.contains("2026-11-02"), "Must identify post date: {}", err_msg3);
    assert!(err_msg3.contains("Unbalanced Coffee"), "Must identify description: {}", err_msg3);
    assert!(err_msg3.contains("Assets:Cash") || err_msg3.contains("Coffee"), "Must identify accounts: {}", err_msg3);
    assert!(err_msg3.contains("0.5"), "Must identify imbalance: {}", err_msg3);
    assert!(!err_msg3.contains("error returned from database"));
}
