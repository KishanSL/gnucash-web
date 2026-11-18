use axum::{
    async_trait,
    extract::{FromRef, FromRequestParts},
    http::{request::Parts, StatusCode},
    Json,
};
use chrono::{DateTime, Utc};
use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, Validation};
use rand::rngs::OsRng;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use sqlx::FromRow;
use std::sync::OnceLock;
use uuid::Uuid;

static ENCODING_KEY: OnceLock<EncodingKey> = OnceLock::new();
static DECODING_KEY: OnceLock<DecodingKey> = OnceLock::new();

fn get_encoding_key() -> &'static EncodingKey {
    ENCODING_KEY.get_or_init(|| {
        EncodingKey::from_secret(get_jwt_secret().as_bytes())
    })
}

fn get_decoding_key() -> &'static DecodingKey {
    DECODING_KEY.get_or_init(|| {
        DecodingKey::from_secret(get_jwt_secret().as_bytes())
    })
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct User {
    pub id: Uuid,
    pub username: String,
    pub email: String,
    #[serde(default)]
    pub token_version: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow)]
pub struct UserWithPassword {
    pub id: Uuid,
    pub username: String,
    pub email: String,
    pub password_hash: String,
    #[sqlx(default)]
    pub token_version: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct SignupRequest {
    pub username: String,
    pub email: String,
    pub password: String,
}

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub username: String, // username or email
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct AuthResponse {
    pub token: String,
    pub user: User,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String, // user_id as string for standard JWT compliance
    pub username: String,
    pub email: String,
    #[serde(default)]
    pub token_version: i32,
    pub exp: usize,
}

pub fn get_jwt_secret() -> String {
    match std::env::var("JWT_SECRET") {
        Ok(secret) if secret.trim().len() >= 32 => secret.trim().to_string(),
        _ => {
            // Strictly restricted to debug/test builds; never available in release builds
            if cfg!(debug_assertions) && (cfg!(test) || std::env::var("GNUCASH_TEST_MODE").map(|v| v == "1" || v == "true").unwrap_or(false)) {
                "test-secret-key-must-be-at-least-32-chars-long!".to_string()
            } else {
                panic!("FATAL: JWT_SECRET environment variable is unset or shorter than 32 characters. Server cannot start securely.");
            }
        }
    }
}

pub fn create_jwt(user_id: Uuid, username: &str, email: &str, token_version: i32) -> Result<String, jsonwebtoken::errors::Error> {
    let expiration = chrono::Utc::now()
        .checked_add_signed(chrono::Duration::days(30))
        .expect("valid timestamp")
        .timestamp() as usize;

    let claims = Claims {
        sub: user_id.to_string(),
        username: username.to_owned(),
        email: email.to_string(),
        token_version,
        exp: expiration,
    };

    jsonwebtoken::encode(&Header::new(Algorithm::HS256), &claims, get_encoding_key())
}

pub fn verify_jwt(token: &str) -> Result<Claims, jsonwebtoken::errors::Error> {
    let mut validation = Validation::new(Algorithm::HS256);
    validation.validate_exp = true;
    let token_data = jsonwebtoken::decode::<Claims>(token, get_decoding_key(), &validation)?;
    Ok(token_data.claims)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthMethod {
    /// A browser/app session JWT (SEC-5, SEC-14).
    Session,
    /// A long-lived API token for a service client (SEC-16).
    ApiToken,
}

#[derive(Debug, Clone)]
pub struct AuthUser {
    pub id: Uuid,
    pub username: String,
    pub email: String,
    pub method: AuthMethod,
}

/// A listed API token. Never contains the token itself or its hash (SEC-19).
#[derive(Debug, Clone, Serialize, FromRow)]
pub struct ApiTokenSummary {
    pub id: Uuid,
    pub name: String,
    pub token_prefix: String,
    pub created_at: DateTime<Utc>,
    pub last_used_at: Option<DateTime<Utc>>,
    pub expires_at: Option<DateTime<Utc>>,
    pub revoked_at: Option<DateTime<Utc>>,
}

#[derive(Debug, FromRow)]
struct ApiTokenOwner {
    token_id: Uuid,
    user_id: Uuid,
    username: String,
    email: String,
    last_used_at: Option<DateTime<Utc>>,
}

/// Generates a `gcw_` + 32 CSPRNG bytes token (SEC-16). Shown to the user once; only the hash is stored.
pub fn generate_api_token() -> String {
    let mut bytes = [0u8; 32];
    OsRng.fill_bytes(&mut bytes);
    format!("gcw_{}", hex::encode(bytes))
}

/// Hex SHA-256 of a token, used for the indexed lookup in `api_tokens.token_hash`.
pub fn hash_api_token(token: &str) -> String {
    hex::encode(Sha256::digest(token.as_bytes()))
}

/// The display prefix kept for the token list (`gcw_` plus 6 hex characters).
pub fn api_token_prefix(token: &str) -> String {
    token.chars().take(10).collect()
}

/// Rejects API tokens on session-only endpoints (SEC-18).
pub fn require_session(
    auth: &AuthUser,
) -> Result<(), (StatusCode, Json<serde_json::Value>)> {
    if auth.method == AuthMethod::ApiToken {
        return Err((
            StatusCode::FORBIDDEN,
            Json(json!({ "error": "This endpoint is not available to API tokens" })),
        ));
    }
    Ok(())
}

fn invalid_token() -> (StatusCode, Json<serde_json::Value>) {
    (
        StatusCode::UNAUTHORIZED,
        Json(json!({ "error": "Invalid or expired token" })),
    )
}

#[async_trait]
impl<S> FromRequestParts<S> for AuthUser
where
    S: Send + Sync,
    sqlx::PgPool: FromRef<S>,
{
    type Rejection = (StatusCode, Json<serde_json::Value>);

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let auth_header = parts
            .headers
            .get(axum::http::header::AUTHORIZATION)
            .and_then(|val| val.to_str().ok());

        let token = match auth_header {
            Some(h) if h.starts_with("Bearer ") => &h[7..],
            _ => {
                // In test execution strictly within debug builds, allow unauthenticated requests to default to the primary test user
                if cfg!(debug_assertions) && (cfg!(test) || std::env::var("GNUCASH_TEST_MODE").map(|v| v == "1" || v == "true").unwrap_or(false)) {
                    return Ok(AuthUser {
                        id: Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap(),
                        username: "kishan".to_string(),
                        email: "kishan@localhost".to_string(),
                        method: AuthMethod::Session,
                    });
                }
                return Err((
                    StatusCode::UNAUTHORIZED,
                    Json(json!({ "error": "Missing or invalid Authorization header" })),
                ));
            }
        };

        // API tokens and session JWTs share the Bearer header, so the prefix decides (SEC-17).
        if token.starts_with("gcw_") {
            return authenticate_api_token(token, state).await;
        }

        let claims = verify_jwt(token).map_err(|e| {
            tracing::warn!("Invalid or expired token: {}", e);
            invalid_token()
        })?;

        let user_id = Uuid::parse_str(&claims.sub).map_err(|_| {
            (
                StatusCode::UNAUTHORIZED,
                Json(json!({ "error": "Invalid user ID in token" })),
            )
        })?;

        let pool = sqlx::PgPool::from_ref(state);
        let current_version = sqlx::query_scalar::<_, i32>(
            "SELECT token_version FROM users WHERE id = $1"
        )
        .bind(user_id)
        .fetch_optional(&pool)
        .await
        .map_err(|e| {
            tracing::error!("Database error during authentication: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "Internal server error" })),
            )
        })?
        .ok_or_else(|| {
            (
                StatusCode::UNAUTHORIZED,
                Json(json!({ "error": "User does not exist" })),
            )
        })?;

        if claims.token_version != current_version {
            return Err((
                StatusCode::UNAUTHORIZED,
                Json(json!({ "error": "Token has been revoked. Please log in again." })),
            ));
        }

        Ok(AuthUser {
            id: user_id,
            username: claims.username,
            email: claims.email,
            method: AuthMethod::Session,
        })
    }
}

/// Resolves a `gcw_` bearer token to its owning user (SEC-17). Unknown, revoked and expired tokens
/// all produce the same generic 401 as an invalid JWT, so nothing can be probed.
async fn authenticate_api_token<S>(
    token: &str,
    state: &S,
) -> Result<AuthUser, (StatusCode, Json<serde_json::Value>)>
where
    S: Send + Sync,
    sqlx::PgPool: FromRef<S>,
{
    let pool = sqlx::PgPool::from_ref(state);
    let token_hash = hash_api_token(token);

    let owner = sqlx::query_as::<_, ApiTokenOwner>(
        r#"SELECT t.id AS token_id, t.user_id, u.username, u.email, t.last_used_at
           FROM api_tokens t
           JOIN users u ON u.id = t.user_id
           WHERE t.token_hash = $1
             AND t.revoked_at IS NULL
             AND (t.expires_at IS NULL OR t.expires_at > NOW())"#,
    )
    .bind(&token_hash)
    .fetch_optional(&pool)
    .await
    .map_err(|e| {
        tracing::error!("Database error during API token authentication: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": "Internal server error" })),
        )
    })?
    .ok_or_else(invalid_token)?;

    // Record usage, but at most once a minute per token, and only when the stored value is stale,
    // so ordinary reads do not write on every request.
    let stale = owner
        .last_used_at
        .map(|t| Utc::now().signed_duration_since(t) >= chrono::Duration::seconds(60))
        .unwrap_or(true);
    if stale {
        if let Err(e) = sqlx::query("UPDATE api_tokens SET last_used_at = NOW() WHERE id = $1")
            .bind(owner.token_id)
            .execute(&pool)
            .await
        {
            // A failed bookkeeping update must not break the request.
            tracing::warn!("Failed to update api_tokens.last_used_at: {}", e);
        }
    }

    Ok(AuthUser {
        id: owner.user_id,
        username: owner.username,
        email: owner.email,
        method: AuthMethod::ApiToken,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static ENV_MUTEX: Mutex<()> = Mutex::new(());

    #[test]
    fn test_jwt_secret_validation() {
        let _guard = ENV_MUTEX.lock().unwrap();
        let valid_secret = "0123456789abcdef0123456789abcdef";
        std::env::set_var("JWT_SECRET", valid_secret);
        assert_eq!(get_jwt_secret(), valid_secret);

        // Short secret falls back to safe dummy in test mode
        std::env::set_var("JWT_SECRET", "too-short");
        assert_eq!(get_jwt_secret(), "test-secret-key-must-be-at-least-32-chars-long!");

        // Restore valid secret
        std::env::set_var("JWT_SECRET", valid_secret);
    }

    #[test]
    fn test_create_and_verify_jwt_token_version() {
        let _guard = ENV_MUTEX.lock().unwrap();
        let user_id = Uuid::new_v4();
        let token = create_jwt(user_id, "alice", "alice@example.com", 2).unwrap();
        let claims = verify_jwt(&token).unwrap();
        assert_eq!(claims.token_version, 2);
        assert_eq!(claims.username, "alice");
    }
}
