//! Shared imports and the test-database pool for the integration test crates.
//! Each `tests/*.rs` file is its own crate, so helpers and imports unused by one of them are expected.
#![allow(dead_code, unused_imports)]

pub use axum::{
    body::Body,
    http::{Request, StatusCode},
};
pub use http_body_util::BodyExt;
pub use serde_json::{json, Value};
pub use sqlx::PgPool;
pub use tower::ServiceExt;

pub use gnucash_backend::create_app;

pub async fn get_test_pool() -> PgPool {
    std::env::set_var("GNUCASH_TEST_MODE", "1");
    std::env::set_var("JWT_SECRET", "test-secret-key-must-be-at-least-32-chars-long!");
    let database_url = gnucash_backend::db::get_test_database_url();
    gnucash_backend::db::ensure_test_database_exists(&database_url).await;
    let pool = gnucash_backend::db::init_db(&database_url)
        .await
        .expect("Failed to connect to and init test database");

    // Ensure baseline fixtures exist for primary test user on a fresh database
    let default_user = uuid::Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap();
    let acc_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM accounts WHERE user_id = $1")
        .bind(default_user)
        .fetch_one(&pool)
        .await
        .unwrap_or((0,));

    if acc_count.0 == 0 {
        let mut tx = pool.begin().await.expect("Failed to begin transaction for test fixtures");
        gnucash_backend::handlers::seed::run_seed_tx(&mut tx, default_user)
            .await
            .expect("Failed to seed baseline test fixtures");
        tx.commit().await.expect("Failed to commit test fixtures");
    }

    pool
}
