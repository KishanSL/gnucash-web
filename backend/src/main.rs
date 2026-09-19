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

    // Auto-seed or auto-import if accounts table is empty
    let account_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM accounts")
        .fetch_one(&pool)
        .await
        .unwrap_or((0,));

    if account_count.0 == 0 {
        let mut imported_file = false;
        let home = std::env::var("HOME").unwrap_or_else(|_| "/home/kishan".to_string());
        let codebase_dir = std::path::PathBuf::from(format!("{}/codebase", home));

        if let Ok(entries) = std::fs::read_dir(codebase_dir) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.is_file()
                    && p.extension()
                        .and_then(|e| e.to_str())
                        .map(|ext| ext.eq_ignore_ascii_case("gnucash"))
                        .unwrap_or(false)
                {
                    info!("Found GnuCash file at {:?}, auto-importing on first start...", p);
                    if let Ok(bytes) = std::fs::read(&p) {
                        let default_curr = "INR".to_string();
                        if let Ok((accs, txs, splits, fmt)) = handlers::importer::parse_gnucash_file(&bytes, &default_curr) {
                            if let Ok(res) = handlers::importer::save_gnucash_data_to_db(&pool, true, accs, txs, splits, fmt).await {
                                info!("Auto-imported {:?}: {}", p, res["message"]);
                                imported_file = true;
                                break;
                            }
                        }
                    }
                }
            }
        }

        if !imported_file {
            info!("No .gnucash file found. Seeding initial Chart of Accounts and sample transactions...");
            if let Err(e) = handlers::seed::run_seed(&pool).await {
                tracing::error!("Failed to auto-seed: {}", e);
            } else {
                info!("Successfully auto-seeded database!");
            }
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
