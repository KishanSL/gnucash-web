use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use std::time::Duration;
use tracing::info;
use url::Url;
use uuid::Uuid;

/// Resolves a guaranteed safe test database URL.
/// Strictly asserts that the target database name ends with '_test' to prevent any possibility
/// of tests executing against production or live application data.
pub fn get_test_database_url() -> String {
    let raw = std::env::var("TEST_DATABASE_URL").unwrap_or_else(|_| {
        std::env::var("DATABASE_URL").unwrap_or_else(|_| {
            "postgres://gnucash:gnucash_secret@127.0.0.1:5432/gnucash_test".to_string()
        })
    });

    let mut parsed = Url::parse(&raw).unwrap_or_else(|e| {
        panic!("Invalid database connection URL '{}': {}", raw, e)
    });

    let path_segments: Vec<&str> = parsed.path().trim_start_matches('/').split('/').collect();
    let db_name = path_segments.first().copied().unwrap_or("gnucash");
    let test_db_name = if db_name.ends_with("_test") {
        db_name.to_string()
    } else {
        format!("{}_test", db_name)
    };

    parsed.set_path(&format!("/{}", test_db_name));
    let final_url = parsed.to_string();

    assert!(
        test_db_name.ends_with("_test"),
        "Safety guard violation: test database name '{}' must end with '_test'",
        test_db_name
    );

    final_url
}

/// Ensures the test database exists on the PostgreSQL server, automatically creating it
/// via the maintenance 'postgres' database if running against a brand-new instance.
pub async fn ensure_test_database_exists(test_url: &str) {
    if let Ok(mut admin_url) = Url::parse(test_url) {
        let test_db_name = admin_url.path().trim_start_matches('/').to_string();
        if test_db_name.is_empty() {
            return;
        }

        admin_url.set_path("/postgres");
        if let Ok(admin_pool) = sqlx::PgPool::connect(admin_url.as_str()).await {
            let exists: Result<(bool,), sqlx::Error> = sqlx::query_as(
                "SELECT EXISTS (SELECT 1 FROM pg_database WHERE datname = $1)"
            )
            .bind(&test_db_name)
            .fetch_one(&admin_pool)
            .await;

            if let Ok((false,)) = exists {
                let create_sql = format!("CREATE DATABASE \"{}\"", test_db_name.replace('"', "\"\""));
                if let Err(e) = sqlx::query(&create_sql).execute(&admin_pool).await {
                    tracing::warn!("Could not auto-create test database '{}': {}", test_db_name, e);
                } else {
                    tracing::info!("Auto-created test database '{}'", test_db_name);
                }
            }
        }
    }
}

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
    sqlx::migrate!("./migrations").run(&pool).await?;

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
                .expect("Failed to hash initial admin password")
        })
        .await
        .expect("Password hashing task panicked");

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
