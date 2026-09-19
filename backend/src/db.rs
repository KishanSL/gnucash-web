use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use std::time::Duration;
use tracing::info;

pub async fn init_db(database_url: &str) -> Result<PgPool, sqlx::Error> {
    info!("Connecting to database...");
    let mut retries = 15;
    let pool = loop {
        match PgPoolOptions::new()
            .max_connections(20)
            .acquire_timeout(Duration::from_secs(5))
            .connect(database_url)
            .await
        {
            Ok(p) => break p,
            Err(e) if retries > 0 => {
                retries -= 1;
                tracing::warn!("Database not ready yet ({}). Retrying in 1s... ({} attempts remaining)", e, retries);
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
            Err(e) => return Err(e),
        }
    };

    info!("Running database schema migrations...");
    let migration_sql = include_str!("../migrations/001_initial_schema.sql");
    sqlx::raw_sql(migration_sql).execute(&pool).await?;

    info!("Database initialized successfully.");
    Ok(pool)
}
