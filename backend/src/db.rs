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
    let migration_1 = include_str!("../migrations/001_initial_schema.sql");
    sqlx::raw_sql(migration_1).execute(&pool).await?;

    let migration_2 = include_str!("../migrations/002_multi_user.sql");
    sqlx::raw_sql(migration_2).execute(&pool).await?;

    // Ensure at least one default user exists for pre-existing or imported data
    let user_count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM users")
        .fetch_one(&pool)
        .await
        .unwrap_or(0);

    if user_count == 0 {
        let password_hash = bcrypt::hash("password123", bcrypt::DEFAULT_COST)
            .unwrap_or_else(|_| "$2b$12$e8YgN17Pj9QGqZ2eCq.X3OMWj944j/mYFkU5nZ7cI19j1r5u0gJvG".to_string());

        sqlx::query(
            r#"INSERT INTO users (id, username, email, password_hash)
               VALUES ('00000000-0000-0000-0000-000000000001', 'kishan', 'kishan@localhost', $1)
               ON CONFLICT (username) DO NOTHING"#
        )
        .bind(password_hash)
        .execute(&pool)
        .await?;
        info!("Created default user: kishan (password: password123)");
    }

    // Assign existing orphaned accounts/transactions to the first user
    sqlx::query(
        r#"UPDATE accounts
           SET user_id = (SELECT id FROM users ORDER BY created_at ASC LIMIT 1)
           WHERE user_id IS NULL"#
    )
    .execute(&pool)
    .await?;

    sqlx::query(
        r#"UPDATE transactions
           SET user_id = (SELECT id FROM users ORDER BY created_at ASC LIMIT 1)
           WHERE user_id IS NULL"#
    )
    .execute(&pool)
    .await?;

    info!("Database initialized successfully with multi-user support.");
    Ok(pool)
}
