use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use std::time::Duration;
use tracing::info;
use uuid::Uuid;

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

    let migration_3 = include_str!("../migrations/003_optimizations_and_constraints.sql");
    sqlx::raw_sql(migration_3).execute(&pool).await?;

    // Ensure at least one default user exists for pre-existing or imported data
    let user_count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM users")
        .fetch_one(&pool)
        .await
        .unwrap_or(0);

    if user_count == 0 {
        let initial_username = std::env::var("INITIAL_ADMIN_USER")
            .ok()
            .map(|u| u.trim().to_string())
            .filter(|u| !u.is_empty())
            .unwrap_or_else(|| "kishan".to_string());

        let env_pw = std::env::var("INITIAL_ADMIN_PASSWORD")
            .ok()
            .map(|p| p.trim().to_string())
            .filter(|p| !p.is_empty());

        let is_test_mode = cfg!(test)
            || std::env::var("GNUCASH_TEST_MODE")
                .map(|v| v == "1" || v == "true")
                .unwrap_or(false);

        let (raw_password, is_generated) = if let Some(pw) = env_pw {
            (pw, false)
        } else if is_test_mode {
            ("Kishan@123".to_string(), false)
        } else {
            let gen = format!("admin_{}", Uuid::new_v4().simple());
            (gen, true)
        };

        if is_generated {
            let cred_path = std::env::var("SECURITY_DIR")
                .unwrap_or_else(|_| "/app/security".to_string());
            let _ = std::fs::create_dir_all(&cred_path);
            let file_path = format!("{}/initial_admin_credentials.txt", cred_path);
            let cred_content = format!(
                "Initial Admin Credentials:\nUsername: {}\nPassword: {}\nGenerated At: {}\n",
                initial_username, raw_password, chrono::Utc::now()
            );
            if let Err(e) = std::fs::write(&file_path, cred_content) {
                tracing::warn!("Failed to write initial admin credentials to {}: {}", file_path, e);
            } else {
                info!("Initial admin credentials written to {}", file_path);
            }
        }

        let password_hash = tokio::task::spawn_blocking(move || {
            bcrypt::hash(&raw_password, bcrypt::DEFAULT_COST)
                .unwrap_or_else(|_| "$2b$12$e8YgN17Pj9QGqZ2eCq.X3OMWj944j/mYFkU5nZ7cI19j1r5u0gJvG".to_string())
        })
        .await
        .unwrap_or_else(|_| "$2b$12$e8YgN17Pj9QGqZ2eCq.X3OMWj944j/mYFkU5nZ7cI19j1r5u0gJvG".to_string());

        let email = format!("{}@localhost", initial_username);
        sqlx::query(
            r#"INSERT INTO users (id, username, email, password_hash)
               VALUES ('00000000-0000-0000-0000-000000000001', $1, $2, $3)
               ON CONFLICT (username) DO NOTHING"#
        )
        .bind(&initial_username)
        .bind(&email)
        .bind(password_hash)
        .execute(&pool)
        .await?;
        info!("Created default initial user: {}", initial_username);
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
