pub mod auth;
pub mod db;
pub mod handlers;
pub mod models;

use axum::{
    routing::{delete, get, post, put},
    Json, Router,
};
use axum::http::HeaderValue;
use serde_json::json;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;

pub fn create_app(pool: sqlx::PgPool) -> Router {
    let cors = if let Ok(origins) = std::env::var("CORS_ALLOWED_ORIGINS") {
        if origins.trim() == "*" {
            CorsLayer::new().allow_origin(Any)
        } else {
            let parsed: Vec<HeaderValue> = origins
                .split(',')
                .filter_map(|s| s.trim().parse::<HeaderValue>().ok())
                .collect();
            CorsLayer::new().allow_origin(parsed)
        }
    } else {
        CorsLayer::new().allow_origin([
            "http://localhost:8081".parse::<HeaderValue>().unwrap(),
            "http://127.0.0.1:8081".parse::<HeaderValue>().unwrap(),
            "http://localhost:5173".parse::<HeaderValue>().unwrap(),
            "http://127.0.0.1:5173".parse::<HeaderValue>().unwrap(),
        ])
    }
    .allow_methods(Any)
    .allow_headers(Any);

    Router::new()
        // Health check
        .route("/health", get(health_check))
        .route("/api/health", get(health_check))
        // Authentication
        .route("/api/auth/signup", post(handlers::auth::signup))
        .route("/api/auth/login", post(handlers::auth::login))
        .route("/api/auth/reset-password", post(handlers::auth::reset_password))
        .route("/api/auth/change-password", post(handlers::auth::change_password))
        .route("/api/auth/me", get(handlers::auth::get_current_user))
        // Accounts
        .route("/api/accounts", get(handlers::accounts::list_accounts))
        .route("/api/accounts/tree", get(handlers::accounts::get_account_tree))
        .route("/api/accounts", post(handlers::accounts::create_account))
        .route("/api/accounts/:id", get(handlers::accounts::get_account))
        .route("/api/accounts/:id", put(handlers::accounts::update_account))
        .route("/api/accounts/:id", delete(handlers::accounts::delete_account))
        .route(
            "/api/accounts/:id/register",
            get(handlers::transactions::get_account_register),
        )
        // Transactions
        .route(
            "/api/transactions",
            get(handlers::transactions::list_transactions),
        )
        .route(
            "/api/transactions",
            post(handlers::transactions::create_transaction),
        )
        .route(
            "/api/transactions/:id",
            get(handlers::transactions::get_transaction),
        )
        .route(
            "/api/transactions/:id",
            put(handlers::transactions::update_transaction),
        )
        .route(
            "/api/transactions/:id",
            delete(handlers::transactions::delete_transaction),
        )
        // Reconcile
        .route(
            "/api/reconcile/:id",
            get(handlers::reconcile::get_reconcile_info),
        )
        .route(
            "/api/reconcile/:id",
            post(handlers::reconcile::finish_reconcile),
        )
        // Reports
        .route(
            "/api/reports/balance-sheet",
            get(handlers::reports::get_balance_sheet),
        )
        .route(
            "/api/reports/income-statement",
            get(handlers::reports::get_income_statement),
        )
        .route(
            "/api/reports/trial-balance",
            get(handlers::reports::get_trial_balance),
        )
        // Seed, Import & Export
        .route("/api/seed", post(handlers::seed::seed_database))
        .route("/api/reset-data", post(handlers::seed::reset_dataset))
        .route(
            "/api/import/gnucash-sqlite",
            post(handlers::importer::import_gnucash_sqlite)
                .layer(axum::extract::DefaultBodyLimit::max(50 * 1024 * 1024)),
        )
        .route(
            "/api/export/gnucash-sqlite",
            get(handlers::exporter::export_gnucash_sqlite),
        )
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .with_state(pool)
}

pub async fn health_check() -> Json<serde_json::Value> {
    let allow_signup = std::env::var("ALLOW_SIGNUP")
        .map(|v| v == "true" || v == "1")
        .unwrap_or(false);

    Json(json!({
        "status": "ok",
        "service": "gnucash-backend",
        "version": "0.1.0",
        "allow_signup": allow_signup
    }))
}
