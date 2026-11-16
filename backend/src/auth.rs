use axum::{
    async_trait,
    extract::{FromRef, FromRequestParts},
    http::{request::Parts, StatusCode},
    Json,
};
use chrono::{DateTime, Utc};
use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use serde_json::json;
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

#[derive(Debug, Clone)]
pub struct AuthUser {
    pub id: Uuid,
    pub username: String,
    pub email: String,
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
                    });
                }
                return Err((
                    StatusCode::UNAUTHORIZED,
                    Json(json!({ "error": "Missing or invalid Authorization header" })),
                ));
            }
        };

        let claims = verify_jwt(token).map_err(|e| {
            tracing::warn!("Invalid or expired token: {}", e);
            (
                StatusCode::UNAUTHORIZED,
                Json(json!({ "error": "Invalid or expired token" })),
            )
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
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_jwt_secret_validation() {
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
        let user_id = Uuid::new_v4();
        let token = create_jwt(user_id, "alice", "alice@example.com", 2).unwrap();
        let claims = verify_jwt(&token).unwrap();
        assert_eq!(claims.token_version, 2);
        assert_eq!(claims.username, "alice");
    }
}
