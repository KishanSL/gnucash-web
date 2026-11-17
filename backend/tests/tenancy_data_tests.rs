//! Multi-tenant isolation, reset/seed and migrations

mod common;
use common::*;

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
                .uri(format!("/api/accounts/{}", acc1_id))
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
                .uri(format!("/api/accounts/{}", acc1_id))
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
                .uri(format!("/api/accounts/{}", acc1_id))
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
                .uri(format!("/api/accounts/{}/register", acc1_id))
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
                .uri(format!("/api/transactions/{}", tx_id))
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
                .uri(format!("/api/transactions/{}", tx_id))
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
                .uri(format!("/api/transactions/{}", tx_id))
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
async fn test_issue_30_reconciled_state_migration_idempotent() {
    let base_url = gnucash_backend::db::get_test_database_url();
    let unique_id = uuid::Uuid::new_v4().simple();
    let throwaway_db = format!("gnucash_mig_{}_test", unique_id);
    let mut admin_url = url::Url::parse(&base_url).unwrap();
    admin_url.set_path("/postgres");
    let admin_pool = sqlx::PgPool::connect(admin_url.as_str()).await.unwrap();

    // Clean up any stale migration test databases from previous/interrupted runs (Issue 4)
    if let Ok(stale_dbs) = sqlx::query_scalar::<_, String>(
        "SELECT datname FROM pg_database WHERE datname LIKE 'gnucash_mig_%_test'"
    )
    .fetch_all(&admin_pool)
    .await {
        for stale in stale_dbs {
            let _ = sqlx::query(&format!("DROP DATABASE IF EXISTS \"{}\" WITH (FORCE)", stale)).execute(&admin_pool).await;
        }
    }

    // Create throwaway test database to test the actual migration files from disk
    sqlx::query(&format!("CREATE DATABASE \"{}\"", throwaway_db)).execute(&admin_pool).await.unwrap();

    let mut throwaway_url = url::Url::parse(&base_url).unwrap();
    throwaway_url.set_path(&format!("/{}", throwaway_db));
    let test_pool = sqlx::PgPool::connect(throwaway_url.as_str()).await.unwrap();

    // Apply baseline migrations 001 and 002 directly from file
    let mig_001 = include_str!("../migrations/001_initial_schema.sql");
    let mig_002 = include_str!("../migrations/002_multi_user.sql");
    sqlx::raw_sql(mig_001).execute(&test_pool).await.unwrap();
    sqlx::raw_sql(mig_002).execute(&test_pool).await.unwrap();

    // Insert legacy user, account, transaction, and splits with non-normalized reconciled states
    let user_id = uuid::Uuid::new_v4();
    sqlx::query("INSERT INTO users (id, username, email, password_hash) VALUES ($1, 'legacy_u', 'legacy@example.com', 'hash')")
        .bind(user_id).execute(&test_pool).await.unwrap();
    let acc_id = uuid::Uuid::new_v4();
    sqlx::query("INSERT INTO accounts (id, name, account_type, currency, placeholder, user_id) VALUES ($1, 'Legacy Acc', 'bank', 'INR', false, $2)")
        .bind(acc_id).bind(user_id).execute(&test_pool).await.unwrap();
    let tx_id = uuid::Uuid::new_v4();
    sqlx::query("INSERT INTO transactions (id, user_id, post_date, description) VALUES ($1, $2, '2026-11-01', 'Legacy Tx')")
        .bind(tx_id).bind(user_id).execute(&test_pool).await.unwrap();

    let sp_y = uuid::Uuid::new_v4();
    let sp_c = uuid::Uuid::new_v4();
    let sp_f = uuid::Uuid::new_v4();
    sqlx::query("INSERT INTO splits (id, transaction_id, account_id, amount, reconciled_state, reconciled_date) VALUES ($1, $4, $5, 10, 'Y', NOW()), ($2, $4, $5, -10, 'C', NOW()), ($3, $4, $5, 0, 'f', NOW())")
        .bind(sp_y).bind(sp_c).bind(sp_f).bind(tx_id).bind(acc_id).execute(&test_pool).await.unwrap();

    // Execute the ACTUAL migration file 003_optimizations_and_constraints.sql!
    let mig_003 = include_str!("../migrations/003_optimizations_and_constraints.sql");
    sqlx::raw_sql(mig_003).execute(&test_pool).await.unwrap();

    // Verify row normalization from running migration 003
    let state_y: String = sqlx::query_scalar("SELECT reconciled_state FROM splits WHERE id = $1").bind(sp_y).fetch_one(&test_pool).await.unwrap();
    assert_eq!(state_y, "y");
    let state_c: String = sqlx::query_scalar("SELECT reconciled_state FROM splits WHERE id = $1").bind(sp_c).fetch_one(&test_pool).await.unwrap();
    assert_eq!(state_c, "c");
    let (state_f, rec_date_f): (String, Option<chrono::DateTime<chrono::Utc>>) = sqlx::query_as("SELECT reconciled_state, reconciled_date FROM splits WHERE id = $1")
        .bind(sp_f).fetch_one(&test_pool).await.unwrap();
    assert_eq!(state_f, "n");
    assert!(rec_date_f.is_none(), "reconciled_state 'n' must have reconcile_date NULL (DATA-3, REC-1)");

    // Get constraint OID
    let oid1: i64 = sqlx::query_scalar("SELECT oid::int8 FROM pg_constraint WHERE conname = 'chk_splits_reconciled_state'")
        .fetch_one(&test_pool).await.unwrap();

    // Run migration 003 a SECOND time to verify it is completely idempotent!
    sqlx::raw_sql(mig_003).execute(&test_pool).await.unwrap();

    let oid2: i64 = sqlx::query_scalar("SELECT oid::int8 FROM pg_constraint WHERE conname = 'chk_splits_reconciled_state'")
        .fetch_one(&test_pool).await.unwrap();
    assert_eq!(oid1, oid2, "Constraint must not be dropped and re-created when valid (idempotent)");

    // Verify invalid state 'Z' fails the constraint
    let err = sqlx::query("INSERT INTO splits (id, transaction_id, account_id, amount, reconciled_state) VALUES ($1, $2, $3, 10, 'Z')")
        .bind(uuid::Uuid::new_v4()).bind(tx_id).bind(acc_id).execute(&test_pool).await;
    assert!(err.is_err(), "Inserting invalid state 'Z' must violate constraint");

    // Close pool and clean up throwaway database with FORCE
    test_pool.close().await;
    let _ = sqlx::query(&format!("DROP DATABASE IF EXISTS \"{}\" WITH (FORCE)", throwaway_db)).execute(&admin_pool).await;
}

#[tokio::test]
async fn test_issue_31_seed_database_atomic_rollback() {
    let pool = get_test_pool().await;
    let user_id = uuid::Uuid::new_v4();
    let username = format!("user_seed_rb_{}", user_id.simple());
    let email = format!("user_seed_rb_{}@example.com", user_id.simple());

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

    // Pre-create account and transaction
    let acc_id = uuid::Uuid::new_v4();
    sqlx::query("INSERT INTO accounts (id, name, account_type, currency, placeholder, user_id) VALUES ($1, 'Precious Savings', 'bank', 'INR', false, $2)")
        .bind(acc_id).bind(user_id).execute(&pool).await.unwrap();

    let tx_id = uuid::Uuid::new_v4();
    sqlx::query("INSERT INTO transactions (id, user_id, post_date, description) VALUES ($1, $2, '2026-11-01', 'Critical Transfer')")
        .bind(tx_id).bind(user_id).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO splits (id, transaction_id, account_id, amount, quantity, reconciled_state) VALUES ($1, $2, $3, 100.00, 100.00, 'n')")
        .bind(uuid::Uuid::new_v4()).bind(tx_id).bind(acc_id).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO splits (id, transaction_id, account_id, amount, quantity, reconciled_state) VALUES ($1, $2, $3, -100.00, -100.00, 'n')")
        .bind(uuid::Uuid::new_v4()).bind(tx_id).bind(acc_id).execute(&pool).await.unwrap();

    // Trigger failure in seed_database after delete using test hook targeted at this user ID (DATA-2)
    std::env::set_var("FAIL_SEED_TEST_USER_ID", user_id.to_string());

    let app = create_app(pool.clone());
    let resp = app.oneshot(
        Request::builder()
            .method("POST")
            .uri("/api/seed")
            .header("authorization", format!("Bearer {}", token))
            .body(Body::empty())
            .unwrap()
    ).await.unwrap();

    std::env::remove_var("FAIL_SEED_TEST_USER_ID");

    assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);

    // Verify pre-existing account and transaction were NOT deleted (DATA-2 rollback preservation)
    let acc_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM accounts WHERE user_id = $1")
        .bind(user_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(acc_count, 1, "Pre-existing accounts must survive failed seed attempt");

    let tx_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM transactions WHERE user_id = $1")
        .bind(user_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(tx_count, 1, "Pre-existing transactions must survive failed seed attempt");
}
