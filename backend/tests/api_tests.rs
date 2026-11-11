use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use sqlx::PgPool;
use tokio::sync::OnceCell;
use tower::ServiceExt;

use gnucash_backend::{create_app, db};

static TEST_POOL: OnceCell<PgPool> = OnceCell::const_new();

async fn get_test_pool() -> PgPool {
    std::env::set_var("GNUCASH_TEST_MODE", "1");
    TEST_POOL
        .get_or_init(|| async {
            let database_url = std::env::var("DATABASE_URL").unwrap_or_else(|_| {
                "postgres://gnucash:gnucash_secret@127.0.0.1:5432/gnucash".to_string()
            });
            let pool = db::init_db(&database_url)
                .await
                .expect("Failed to init database");

            let account_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM accounts")
                .fetch_one(&pool)
                .await
                .unwrap_or((0,));

            if account_count.0 == 0 {
                let default_user_id: uuid::Uuid = sqlx::query_scalar("SELECT id FROM users LIMIT 1")
                    .fetch_optional(&pool)
                    .await
                    .unwrap_or(None)
                    .unwrap_or_else(uuid::Uuid::new_v4);
                gnucash_backend::handlers::seed::run_seed(&pool, default_user_id)
                    .await
                    .expect("Failed to seed database");
            }

            pool
        })
        .await
        .clone()
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

    // 1. Successful login with default user (tries Kishan@123 then password123)
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
            "password": "password123"
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

    // Login to get token (try Kishan@123, then password123)
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
            "password": "password123"
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
        "password": "password123"
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
        "password": "password123"
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
                    "password": "wrongpassword123"
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
