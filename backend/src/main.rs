use gnucash_backend::{create_app, db, handlers};
use std::net::SocketAddr;
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "gnucash_backend=debug,tower_http=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://gnucash:gnucash_secret@127.0.0.1:5432/gnucash".to_string());

    let pool = db::init_db(&database_url).await?;

    // Auto-seed if accounts table is empty
    let account_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM accounts")
        .fetch_one(&pool)
        .await
        .unwrap_or((0,));

    if account_count.0 == 0 {
        info!("No accounts found. Seeding initial Chart of Accounts and sample transactions...");
        if let Err(e) = handlers::seed::run_seed(&pool).await {
            tracing::error!("Failed to auto-seed: {}", e);
        } else {
            info!("Successfully auto-seeded database!");
        }
    } else {
        info!("Database already contains {} accounts.", account_count.0);
    }

    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8082);

    let app = create_app(pool);
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    info!("GnuCash Backend server listening on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
