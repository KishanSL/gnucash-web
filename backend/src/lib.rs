pub mod auth;
pub mod db;
pub mod handlers;
pub mod models;

use axum::{
    routing::{delete, get, post, put},
    Json, Router,
};
use serde_json::json;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;

pub fn create_app(pool: sqlx::PgPool) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
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
        // Seed & Import
        .route("/api/seed", post(handlers::seed::seed_database))
        .route(
            "/api/import/gnucash-sqlite",
            post(handlers::importer::import_gnucash_sqlite),
        )
        .route(
            "/api/import/local-files",
            get(handlers::importer::list_local_gnucash_files),
        )
        .route(
            "/api/import/from-path",
            post(handlers::importer::import_from_path),
        )
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .with_state(pool)
}

pub async fn health_check() -> Json<serde_json::Value> {
    Json(json!({
        "status": "ok",
        "service": "gnucash-backend",
        "version": "0.1.0"
    }))
}
