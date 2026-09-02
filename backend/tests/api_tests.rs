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
                gnucash_backend::handlers::seed::run_seed(&pool)
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
        "SELECT id, name FROM accounts WHERE placeholder = FALSE LIMIT 2",
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
        "SELECT id, name FROM accounts WHERE placeholder = FALSE LIMIT 1",
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
        "SELECT id FROM accounts WHERE placeholder = TRUE LIMIT 1",
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    let regular = sqlx::query_as::<_, (uuid::Uuid,)>(
        "SELECT id FROM accounts WHERE placeholder = FALSE LIMIT 1",
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
        "SELECT id FROM accounts WHERE name ILIKE '%Checking%' LIMIT 1",
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    let groceries = sqlx::query_as::<_, (uuid::Uuid,)>(
        "SELECT id FROM accounts WHERE name ILIKE '%Groceries%' LIMIT 1",
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
        "SELECT id FROM accounts WHERE name ILIKE '%Checking%' LIMIT 1",
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
        "SELECT id FROM accounts WHERE name ILIKE '%Checking%' LIMIT 1",
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
