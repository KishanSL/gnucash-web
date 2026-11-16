pub mod accounts;
pub mod auth;
pub mod exporter;
pub mod importer;
pub mod reconcile;
pub mod reports;
pub mod seed;
pub mod transactions;

use axum::{http::StatusCode, Json};
use serde_json::{json, Value};
use std::fmt::Display;

/// Sanitizes backend errors for API responses while logging the underlying cause server-side (SEC-13).
pub fn internal_error<E: Display>(context: &str, err: E) -> (StatusCode, Json<Value>) {
    tracing::error!("{}: {}", context, err);
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({ "error": "Internal server error" })),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_internal_error_sanitization() {
        let (status, Json(body)) = internal_error("Failed query", "syntax error at or near 'SELECT'");
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(body, json!({ "error": "Internal server error" }));
        assert!(!body.to_string().contains("syntax error"));
    }
}

