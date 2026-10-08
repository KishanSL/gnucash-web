use axum::{
    async_trait,
    extract::FromRequestParts,
    http::{request::Parts, StatusCode},
    Json,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct User {
    pub id: Uuid,
    pub username: String,
    pub email: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow)]
pub struct UserWithPassword {
    pub id: Uuid,
    pub username: String,
    pub email: String,
    pub password_hash: String,
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
    pub exp: usize,
}

pub fn get_jwt_secret() -> String {
    std::env::var("JWT_SECRET").unwrap_or_else(|_| "gnucash-super-secret-jwt-key-2026".to_string())
}

pub fn create_jwt(user_id: Uuid, username: &str, email: &str) -> Result<String, jsonwebtoken::errors::Error> {
    let expiration = chrono::Utc::now()
        .checked_add_signed(chrono::Duration::days(30))
        .expect("valid timestamp")
        .timestamp() as usize;

    let claims = Claims {
        sub: user_id.to_string(),
        username: username.to_string(),
        email: email.to_string(),
        exp: expiration,
    };

    let key = jsonwebtoken::EncodingKey::from_secret(get_jwt_secret().as_bytes());
    jsonwebtoken::encode(&jsonwebtoken::Header::default(), &claims, &key)
}

pub fn verify_jwt(token: &str) -> Result<Claims, jsonwebtoken::errors::Error> {
    let key = jsonwebtoken::DecodingKey::from_secret(get_jwt_secret().as_bytes());
    let validation = jsonwebtoken::Validation::default();
    let token_data = jsonwebtoken::decode::<Claims>(token, &key, &validation)?;
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
{
    type Rejection = (StatusCode, Json<serde_json::Value>);

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let auth_header = parts
            .headers
            .get(axum::http::header::AUTHORIZATION)
            .and_then(|val| val.to_str().ok());

        let token = match auth_header {
            Some(h) if h.starts_with("Bearer ") => &h[7..],
            _ => {
                // In test execution, allow unauthenticated requests to default to the primary test user
                if cfg!(test) || std::env::var("GNUCASH_TEST_MODE").map(|v| v == "1" || v == "true").unwrap_or(false) {
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
            (
                StatusCode::UNAUTHORIZED,
                Json(json!({ "error": format!("Invalid or expired token: {}", e) })),
            )
        })?;

        let user_id = Uuid::parse_str(&claims.sub).map_err(|_| {
            (
                StatusCode::UNAUTHORIZED,
                Json(json!({ "error": "Invalid user ID in token" })),
            )
        })?;

        Ok(AuthUser {
            id: user_id,
            username: claims.username,
            email: claims.email,
        })
    }
}
