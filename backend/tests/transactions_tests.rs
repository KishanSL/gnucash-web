//! Double-entry validation, transaction create/update, register and reconciliation (ACC-*, REC-*)

mod common;
use common::*;

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
                .uri(format!("/api/accounts/{}/register", checking.0))
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
                .uri(format!("/api/reconcile/{}", checking.0))
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
                .uri(format!("/api/reconcile/{}", acc1))
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
                .uri(format!("/api/reconcile/{}", acc2))
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
                .uri(format!("/api/transactions/{}", tx_id))
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
                .uri(format!("/api/transactions/{}", tx_id))
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
                .uri(format!("/api/transactions/{}", tx_id))
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
                .uri(format!("/api/transactions/{}", tx_id))
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
            .uri(format!("/api/accounts/{}", child_id))
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
            .uri(format!("/api/accounts/{}", child_id))
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
            .uri(format!("/api/transactions/{}", tx_id))
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
            .uri(format!("/api/transactions/{}", tx_id))
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
async fn test_fund_account_transaction_fixed_2_decimals() {
    // ACC-3 & INV-5: Transactions on fund accounts (commodity_fraction = 1000)
    // must strictly enforce 2-decimal scale. 2-decimal splits succeed, >2 decimals fail.
    let pool = get_test_pool().await;
    let user_id = uuid::Uuid::new_v4();
    let username = format!("fund_{}", &user_id.to_string()[..8]);
    let email = format!("{}@example.com", username);
    sqlx::query("INSERT INTO users (id, username, email, password_hash) VALUES ($1, $2, $3, 'hash')")
        .bind(user_id)
        .bind(&username)
        .bind(&email)
        .execute(&pool)
        .await
        .unwrap();
    let user_token = gnucash_backend::auth::create_jwt(user_id, &username, &email, 0).unwrap();

    let bank_id = uuid::Uuid::new_v4();
    let fund_id = uuid::Uuid::new_v4();
    sqlx::query(
        "INSERT INTO accounts (id, name, account_type, currency, user_id, commodity_namespace, commodity_fraction)
         VALUES ($1, 'Checking', 'bank', 'INR', $3, 'CURRENCY', 100),
                ($2, 'Mutual Fund', 'asset', 'INF209K01VA3', $3, 'FUND', 1000)"
    )
    .bind(bank_id)
    .bind(fund_id)
    .bind(user_id)
    .execute(&pool)
    .await
    .unwrap();

    let app = create_app(pool.clone());

    // 1. Transaction with 2-decimal splits on fund account must succeed (ACC-3)
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/transactions")
                .header("authorization", format!("Bearer {}", user_token))
                .header("content-type", "application/json")
                .body(Body::from(json!({
                    "post_date": "2026-11-17",
                    "description": "Fund SIP 2-decimals",
                    "splits": [
                        { "account_id": fund_id, "amount": "5000.50" },
                        { "account_id": bank_id, "amount": "-5000.50" }
                    ]
                }).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED, "2-decimal transaction on fund account must succeed");

    // 2. Transaction with 3-decimal splits on fund account must be rejected with 400 (ACC-3)
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/transactions")
                .header("authorization", format!("Bearer {}", user_token))
                .header("content-type", "application/json")
                .body(Body::from(json!({
                    "post_date": "2026-11-17",
                    "description": "Fund SIP 3-decimals",
                    "splits": [
                        { "account_id": fund_id, "amount": "5000.500" },
                        { "account_id": bank_id, "amount": "-5000.500" }
                    ]
                }).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST, "3-decimal transaction on fund account must be rejected per ACC-3");
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
