//! Health check, account listing/tree, hierarchy and account field validation (ACC-*)

mod common;
use common::*;

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
async fn test_issue_29_currency_length_boundaries() {
    let pool = get_test_pool().await;
    let user_id = uuid::Uuid::new_v4();
    let username = format!("user_curr_len_{}", user_id.simple());
    let email = format!("user_curr_len_{}@example.com", user_id.simple());

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

    // 1. 64-char currency succeeds (ACC-11)
    let curr_64 = "A".repeat(64);
    let app1 = create_app(pool.clone());
    let resp1 = app1.oneshot(
        Request::builder()
            .method("POST")
            .uri("/api/accounts")
            .header("authorization", format!("Bearer {}", token))
            .header("content-type", "application/json")
            .body(Body::from(json!({
                "name": "64 Char Account",
                "account_type": "asset",
                "currency": curr_64
            }).to_string()))
            .unwrap()
    ).await.unwrap();
    assert_eq!(resp1.status(), StatusCode::CREATED);
    let body_bytes1 = resp1.into_body().collect().await.unwrap().to_bytes();
    let acc_json: Value = serde_json::from_slice(&body_bytes1).unwrap();
    let acc_id = acc_json["id"].as_str().unwrap();

    // 2. 65-char currency fails with 400 (ACC-11)
    let curr_65 = "A".repeat(65);
    let app2 = create_app(pool.clone());
    let resp2 = app2.oneshot(
        Request::builder()
            .method("POST")
            .uri("/api/accounts")
            .header("authorization", format!("Bearer {}", token))
            .header("content-type", "application/json")
            .body(Body::from(json!({
                "name": "65 Char Account",
                "account_type": "asset",
                "currency": curr_65
            }).to_string()))
            .unwrap()
    ).await.unwrap();
    assert_eq!(resp2.status(), StatusCode::BAD_REQUEST);

    // 3. Blank currency fails with 400
    let app3 = create_app(pool.clone());
    let resp3 = app3.oneshot(
        Request::builder()
            .method("POST")
            .uri("/api/accounts")
            .header("authorization", format!("Bearer {}", token))
            .header("content-type", "application/json")
            .body(Body::from(json!({
                "name": "Blank Currency Account",
                "account_type": "asset",
                "currency": "   "
            }).to_string()))
            .unwrap()
    ).await.unwrap();
    assert_eq!(resp3.status(), StatusCode::BAD_REQUEST);

    // 4. PUT with 65-char currency fails with 400
    let app4 = create_app(pool.clone());
    let resp4 = app4.oneshot(
        Request::builder()
            .method("PUT")
            .uri(format!("/api/accounts/{}", acc_id))
            .header("authorization", format!("Bearer {}", token))
            .header("content-type", "application/json")
            .body(Body::from(json!({
                "name": "64 Char Account Renamed",
                "account_type": "asset",
                "currency": curr_65
            }).to_string()))
            .unwrap()
    ).await.unwrap();
    assert_eq!(resp4.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_issue_37_reject_root_account_type() {
    let pool = get_test_pool().await;
    let user_id = uuid::Uuid::new_v4();
    let username = format!("user_no_root_{}", user_id.simple());
    let email = format!("user_no_root_{}@example.com", user_id.simple());

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

    for bad_type in ["root", "ROOT", " Root ", "root "] {
        let app = create_app(pool.clone());
        let resp = app.oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/accounts")
                .header("authorization", format!("Bearer {}", token))
                .header("content-type", "application/json")
                .body(Body::from(json!({
                    "name": "Disallowed Root Account",
                    "account_type": bad_type,
                    "currency": "INR"
                }).to_string()))
                .unwrap()
        ).await.unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST, "Account type '{}' must be rejected (ACC-9)", bad_type);
    }

    // Create valid account then try PUT to root
    let app_create = create_app(pool.clone());
    let resp_create = app_create.oneshot(
        Request::builder()
            .method("POST")
            .uri("/api/accounts")
            .header("authorization", format!("Bearer {}", token))
            .header("content-type", "application/json")
            .body(Body::from(json!({
                "name": "Valid Account",
                "account_type": "asset",
                "currency": "INR"
            }).to_string()))
            .unwrap()
    ).await.unwrap();
    assert_eq!(resp_create.status(), StatusCode::CREATED);
    let body_bytes = resp_create.into_body().collect().await.unwrap().to_bytes();
    let acc_json: Value = serde_json::from_slice(&body_bytes).unwrap();
    let acc_id = acc_json["id"].as_str().unwrap();

    let app_put = create_app(pool.clone());
    let resp_put = app_put.oneshot(
        Request::builder()
            .method("PUT")
            .uri(format!("/api/accounts/{}", acc_id))
            .header("authorization", format!("Bearer {}", token))
            .header("content-type", "application/json")
            .body(Body::from(json!({
                "name": "Valid Account",
                "account_type": "root",
                "currency": "INR"
            }).to_string()))
            .unwrap()
    ).await.unwrap();
    assert_eq!(resp_put.status(), StatusCode::BAD_REQUEST, "PUT account_type to root must be rejected (ACC-9)");
}
