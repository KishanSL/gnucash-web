//! Authentication, password reset/change, signup and input length limits (SEC-*)

mod common;
use common::*;

#[tokio::test]
async fn test_auth_login_and_reset_password() {
    let pool = get_test_pool().await;

    // Dedicated user so a failure mid-test never leaves a shared user with an unknown password
    let user_id = uuid::Uuid::new_v4();
    let test_user = format!("reset_pw_{}", user_id.simple());
    let test_email = format!("{}@example.com", test_user);
    let initial_pwd = "Initial@Secret123";
    let pwd_hash = bcrypt::hash(initial_pwd, bcrypt::DEFAULT_COST).unwrap();
    sqlx::query(
        "INSERT INTO users (id, username, email, password_hash, token_version) VALUES ($1, $2, $3, $4, 0)"
    )
    .bind(user_id)
    .bind(&test_user)
    .bind(&test_email)
    .bind(pwd_hash)
    .execute(&pool)
    .await
    .unwrap();

    // 1. Successful login
    let app = create_app(pool.clone());
    let login_payload = json!({
        "username": test_user,
        "password": initial_pwd
    });

    let res = app
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

    assert_eq!(res.status(), StatusCode::OK);
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&body).unwrap();
    assert!(json.get("token").is_some());
    assert_eq!(json["user"]["username"], test_user.as_str());

    // 2. Request password reset without admin key (must be rejected 403 Forbidden)
    let reset_payload = json!({
        "username": test_user
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
        "username": test_user,
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

    // 4. In logged-in user space, set a new permanent password
    let app4 = create_app(pool);
    let change_pwd_payload = json!({
        "current_password": temp_password,
        "new_password": "Permanent@Secret456"
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
    let user_id = uuid::Uuid::new_v4();
    let test_user = format!("chg_pw_{}", user_id.simple());
    let test_email = format!("{}@example.com", test_user);
    let initial_pwd = "Initial@Secret123";
    let pwd_hash = bcrypt::hash(initial_pwd, bcrypt::DEFAULT_COST).unwrap();

    // Insert dedicated test user so parallel tests never share or mutate user session/token_version
    sqlx::query(
        "INSERT INTO users (id, username, email, password_hash, token_version) VALUES ($1, $2, $3, $4, 0)"
    )
    .bind(user_id)
    .bind(&test_user)
    .bind(&test_email)
    .bind(pwd_hash)
    .execute(&pool)
    .await
    .unwrap();

    // Login to get token
    let app1 = create_app(pool.clone());
    let login_payload = json!({
        "username": test_user,
        "password": initial_pwd
    });
    let login_res = app1
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

    assert_eq!(login_res.status(), StatusCode::OK);
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
                        "current_password": initial_pwd,
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
                        "username": test_user,
                        "password": "updated_password_456"
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(new_login.status(), StatusCode::OK);
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
