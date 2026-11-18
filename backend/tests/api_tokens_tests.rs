//! API tokens for service clients (SEC-16 to SEC-20, Issue #43).

mod common;
use common::*;

use chrono::{Duration, Utc};
use gnucash_backend::auth::hash_api_token;
use uuid::Uuid;

/// Creates a dedicated user so tests never share credentials or data.
async fn create_user(pool: &PgPool) -> (Uuid, String, String) {
    let id = Uuid::new_v4();
    let username = format!("token_user_{}", id.simple());
    let password = "TokenTest@123".to_string();
    let hash = bcrypt::hash(&password, bcrypt::DEFAULT_COST).unwrap();

    sqlx::query(
        "INSERT INTO users (id, username, email, password_hash, token_version) VALUES ($1, $2, $3, $4, 0)",
    )
    .bind(id)
    .bind(&username)
    .bind(format!("{}@example.com", username))
    .bind(hash)
    .execute(pool)
    .await
    .unwrap();

    (id, username, password)
}

/// Creates a real account row for the user, returning its id.
async fn create_account(pool: &PgPool, user_id: Uuid, name: &str) -> Uuid {
    sqlx::query_scalar::<_, Uuid>(
        "INSERT INTO accounts (name, code, account_type, currency, placeholder, user_id) \
         VALUES ($1, NULL, 'bank', 'INR', false, $2) RETURNING id",
    )
    .bind(name)
    .bind(user_id)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn call(
    pool: &PgPool,
    method: &str,
    uri: &str,
    bearer: Option<&str>,
    extra_headers: &[(&str, &str)],
    body: Option<Value>,
) -> (StatusCode, Value) {
    let mut builder = Request::builder().method(method).uri(uri);
    if let Some(token) = bearer {
        builder = builder.header("authorization", format!("Bearer {}", token));
    }
    for (name, value) in extra_headers {
        builder = builder.header(*name, *value);
    }
    let body = match body {
        Some(value) => {
            builder = builder.header("content-type", "application/json");
            Body::from(value.to_string())
        }
        None => Body::empty(),
    };

    let res = create_app(pool.clone())
        .oneshot(builder.body(body).unwrap())
        .await
        .unwrap();
    let status = res.status();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let json = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap_or(Value::Null)
    };
    (status, json)
}

async fn login(pool: &PgPool, username: &str, password: &str) -> String {
    let (status, json) = call(
        pool,
        "POST",
        "/api/auth/login",
        None,
        &[],
        Some(json!({ "username": username, "password": password })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "login failed: {}", json);
    json["token"].as_str().unwrap().to_string()
}

async fn create_token(pool: &PgPool, jwt: &str, name: &str) -> (String, Uuid, String) {
    let (status, json) = call(
        pool,
        "POST",
        "/api/auth/tokens",
        Some(jwt),
        &[],
        Some(json!({ "name": name, "expires_in_days": null })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "create token failed: {}", json);

    (
        json["token"].as_str().unwrap().to_string(),
        Uuid::parse_str(json["id"].as_str().unwrap()).unwrap(),
        json["token_prefix"].as_str().unwrap().to_string(),
    )
}

#[tokio::test]
async fn test_token_authenticates_as_owner_and_posts_transactions() {
    let pool = get_test_pool().await;
    let (user_a, name_a, password_a) = create_user(&pool).await;
    let (user_b, name_b, password_b) = create_user(&pool).await;

    let account_a = create_account(&pool, user_a, "A Checking").await;
    let account_a_income = create_account(&pool, user_a, "A Income").await;
    let account_b = create_account(&pool, user_b, "B Checking").await;

    let jwt_a = login(&pool, &name_a, &password_a).await;
    let (token, token_id, prefix) = create_token(&pool, &jwt_a, "sms_webhook").await;
    // Format: `gcw_` + 64 hex chars, prefix shown to the user, plaintext absent from the database.
    assert!(token.starts_with("gcw_"), "unexpected token format: {}", token);
    assert_eq!(token.len(), 68);
    assert_eq!(prefix, token[..10]);
    assert!(token_id != Uuid::nil());

    let stored_hash: String = sqlx::query_scalar("SELECT token_hash FROM api_tokens WHERE id = $1")
        .bind(token_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(stored_hash, hash_api_token(&token));
    assert_eq!(stored_hash.len(), 64);
    assert_ne!(stored_hash, token);

    // The token reads only its owner's accounts (SEC-10, SEC-17).
    let (status, accounts) = call(&pool, "GET", "/api/accounts", Some(&token), &[], None).await;
    assert_eq!(status, StatusCode::OK, "token read failed: {}", accounts);
    let ids: Vec<&str> = accounts
        .as_array()
        .unwrap()
        .iter()
        .map(|a| a["id"].as_str().unwrap())
        .collect();
    assert!(ids.contains(&account_a.to_string().as_str()));
    assert!(ids.contains(&account_a_income.to_string().as_str()));
    assert!(!ids.contains(&account_b.to_string().as_str()));

    // The token can post a transaction, stored under its owner.
    let (status, created) = call(
        &pool,
        "POST",
        "/api/transactions",
        Some(&token),
        &[],
        Some(json!({
            "post_date": "2026-11-18",
            "description": "Token posted transaction",
            "splits": [
                { "account_id": account_a, "amount": "125.50" },
                { "account_id": account_a_income, "amount": "-125.50" }
            ]
        })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "token transaction failed: {}", created);
    let transaction_id = Uuid::parse_str(created["id"].as_str().unwrap()).unwrap();

    let owner: Uuid = sqlx::query_scalar("SELECT user_id FROM transactions WHERE id = $1")
        .bind(transaction_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(owner, user_a);

    // A different user's token never sees A's data.
    let jwt_b = login(&pool, &name_b, &password_b).await;
    let (token_b, _, _) = create_token(&pool, &jwt_b, "other_service").await;
    let (status, accounts_b) = call(&pool, "GET", "/api/accounts", Some(&token_b), &[], None).await;
    assert_eq!(status, StatusCode::OK);
    let ids_b: Vec<&str> = accounts_b
        .as_array()
        .unwrap()
        .iter()
        .map(|a| a["id"].as_str().unwrap())
        .collect();
    let account_b_id = account_b.to_string();
    assert_eq!(ids_b, vec![account_b_id.as_str()]);
}

#[tokio::test]
async fn test_token_least_privilege_and_management() {
    let pool = get_test_pool().await;
    let (_user_a, name_a, password_a) = create_user(&pool).await;
    let (_user_b, name_b, password_b) = create_user(&pool).await;

    let jwt_a = login(&pool, &name_a, &password_a).await;
    let (token, token_id, _) = create_token(&pool, &jwt_a, "restricted_service").await;

    // Session-only endpoints refuse a token with 403 (SEC-18).
    let revoke_uri = format!("/api/auth/tokens/{}", token_id);
    let forbidden: Vec<(&str, String, Option<Value>)> = vec![
        ("GET", "/api/auth/tokens".to_string(), None),
        (
            "POST",
            "/api/auth/tokens".to_string(),
            Some(json!({ "name": "minted" })),
        ),
        ("DELETE", revoke_uri, None),
        (
            "POST",
            "/api/auth/change-password".to_string(),
            Some(json!({ "current_password": password_a, "new_password": "NewToken@123" })),
        ),
        ("POST", "/api/reset-data".to_string(), None),
        ("POST", "/api/seed".to_string(), None),
    ];

    for (method, uri, body) in forbidden {
        let (status, json) = call(&pool, method, &uri, Some(&token), &[], body).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{} {} -> {}", method, uri, json);
    }

    // The import endpoint rejects tokens before reading the multipart body.
    let boundary = "gcw-test-boundary";
    let multipart_body = format!(
        "--{b}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"book.gnucash\"\r\nContent-Type: application/octet-stream\r\n\r\nnot-a-real-file\r\n--{b}--\r\n",
        b = boundary
    );
    let res = create_app(pool.clone())
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/import/gnucash-sqlite")
                .header("authorization", format!("Bearer {}", token))
                .header(
                    "content-type",
                    format!("multipart/form-data; boundary={}", boundary),
                )
                .body(Body::from(multipart_body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);

    // Listing (with a session JWT) returns metadata only — never the token or its hash.
    let (status, list) = call(&pool, "GET", "/api/auth/tokens", Some(&jwt_a), &[], None).await;
    assert_eq!(status, StatusCode::OK);
    let tokens = list.as_array().unwrap();
    assert_eq!(tokens.len(), 1);
    assert_eq!(tokens[0]["id"], token_id.to_string().as_str());
    assert_eq!(tokens[0]["name"], "restricted_service");
    assert!(tokens[0].get("token").is_none());
    assert!(tokens[0].get("token_hash").is_none());

    // Another user cannot list or revoke A's token (404 for the revoke, and it is not in their list).
    let jwt_b = login(&pool, &name_b, &password_b).await;
    let (status, list_b) = call(&pool, "GET", "/api/auth/tokens", Some(&jwt_b), &[], None).await;
    assert_eq!(status, StatusCode::OK);
    assert!(list_b.as_array().unwrap().is_empty());

    let (status, json) = call(
        &pool,
        "DELETE",
        &format!("/api/auth/tokens/{}", token_id),
        Some(&jwt_b),
        &[],
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "cross-user revoke: {}", json);

    // Unknown, malformed and expired tokens share the generic 401 body with an invalid JWT.
    let (status, malformed) = call(&pool, "GET", "/api/accounts", Some("gcw_deadbeef"), &[], None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(malformed, json!({ "error": "Invalid or expired token" }));

    let (status, bad_jwt) = call(&pool, "GET", "/api/accounts", Some("not.a.jwt"), &[], None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(bad_jwt, malformed);

    sqlx::query("UPDATE api_tokens SET expires_at = $1 WHERE id = $2")
        .bind(Utc::now() - Duration::days(1))
        .bind(token_id)
        .execute(&pool)
        .await
        .unwrap();
    let (status, expired) = call(&pool, "GET", "/api/accounts", Some(&token), &[], None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(expired, malformed);

    // Owner revoke works, then the token is rejected and a second revoke is a 404.
    sqlx::query("UPDATE api_tokens SET expires_at = NULL WHERE id = $1")
        .bind(token_id)
        .execute(&pool)
        .await
        .unwrap();
    let (status, revoked) = call(
        &pool,
        "DELETE",
        &format!("/api/auth/tokens/{}", token_id),
        Some(&jwt_a),
        &[],
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "own revoke: {}", revoked);

    let (status, _) = call(&pool, "GET", "/api/accounts", Some(&token), &[], None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    let (status, _) = call(
        &pool,
        "DELETE",
        &format!("/api/auth/tokens/{}", token_id),
        Some(&jwt_a),
        &[],
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_password_change_keeps_tokens_and_admin_reset_revokes_them() {
    let pool = get_test_pool().await;
    let (username, password) = {
        let (_id, username, password) = create_user(&pool).await;
        (username, password)
    };
    let jwt = login(&pool, &username, &password).await;
    let (token, token_id, _) = create_token(&pool, &jwt, "integration").await;

    // A user's own password change keeps the token working (SEC-20) but revokes the old JWT (SEC-5).
    let new_password = "ChangedToken@123";
    let (status, json) = call(
        &pool,
        "POST",
        "/api/auth/change-password",
        Some(&jwt),
        &[],
        Some(json!({ "current_password": password, "new_password": new_password })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "password change failed: {}", json);

    let (status, _) = call(&pool, "GET", "/api/accounts", Some(&token), &[], None).await;
    assert_eq!(status, StatusCode::OK);

    let (status, _) = call(&pool, "GET", "/api/auth/me", Some(&jwt), &[], None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    // An admin reset revokes every API token for that user (SEC-20).
    let admin_key = "test-secret-key-must-be-at-least-32-chars-long!";
    let (status, json) = call(
        &pool,
        "POST",
        "/api/auth/reset-password",
        None,
        &[("x-admin-key", admin_key)],
        Some(json!({ "username": username, "new_password": "ResetToken@123" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "admin reset failed: {}", json);

    let (status, _) = call(&pool, "GET", "/api/accounts", Some(&token), &[], None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    let revoked_at: Option<chrono::DateTime<Utc>> =
        sqlx::query_scalar("SELECT revoked_at FROM api_tokens WHERE id = $1")
            .bind(token_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(revoked_at.is_some());

    // Sanity: a new session after the reset can still create a working token.
    let jwt_reset = login(&pool, &username, "ResetToken@123").await;
    let (fresh, _, _) = create_token(&pool, &jwt_reset, "after_reset").await;
    let (status, _) = call(&pool, "GET", "/api/accounts", Some(&fresh), &[], None).await;
    assert_eq!(status, StatusCode::OK);
}
