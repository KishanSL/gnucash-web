//! Account field and hierarchy validation shared by create and update (ACC-*, Issue #18, #28).

use sqlx::PgPool;
use uuid::Uuid;

use crate::handlers::{bad_request, internal_error, ApiError};

pub const VALID_ACCOUNT_TYPES: &[&str] = &[
    "bank", "cash", "asset", "credit", "liability", "equity", "income", "expense", "receivable", "payable"
];

pub fn account_type_group(account_type: &str) -> Option<&'static str> {
    match account_type.trim().to_lowercase().as_str() {
        "asset" | "bank" | "cash" | "receivable" | "credit" | "liability" | "payable" => {
            Some("balance_sheet")
        }
        "income" | "expense" => Some("income_expense"),
        "equity" => Some("equity"),
        _ => None,
    }
}

pub fn are_account_types_compatible(type_a: &str, type_b: &str) -> bool {
    let g_a = account_type_group(type_a);
    let g_b = account_type_group(type_b);
    match (g_a, g_b) {
        (Some(a), Some(b)) => a == b,
        _ => false,
    }
}

/// Validates and trims account code (SEC-13, Issue #28).
/// Returns 400 Bad Request if code exceeds 50 characters.
pub fn sanitize_account_code(code: Option<&str>) -> Result<Option<String>, ApiError> {
    match code {
        Some(c) => {
            let trimmed = c.trim();
            if trimmed.chars().count() > 50 {
                return Err(bad_request("Account code cannot exceed 50 characters"));
            }
            if trimmed.is_empty() {
                Ok(None)
            } else {
                Ok(Some(trimmed.to_string()))
            }
        }
        None => Ok(None),
    }
}

/// Trims the name and checks it is 1-255 characters.
pub fn validate_account_name(name: &str) -> Result<String, ApiError> {
    let trimmed = name.trim();
    if trimmed.is_empty() || trimmed.len() > 255 {
        return Err(bad_request("Account name must be between 1 and 255 characters"));
    }
    Ok(trimmed.to_string())
}

/// Lower-cases the account type and checks it is one of VALID_ACCOUNT_TYPES (never `root`).
pub fn validate_account_type(account_type: &str) -> Result<String, ApiError> {
    let normalized = account_type.trim().to_lowercase();
    if !VALID_ACCOUNT_TYPES.contains(&normalized.as_str()) {
        return Err(bad_request(format!("Invalid account type '{}'", normalized)));
    }
    Ok(normalized)
}

/// Upper-cases the currency/commodity code and checks it is 1-64 characters (ISINs, tickers).
pub fn validate_currency(currency: &str) -> Result<String, ApiError> {
    let normalized = currency.trim().to_uppercase();
    if normalized.is_empty() || normalized.len() > 64 {
        return Err(bad_request("Currency code must be between 1 and 64 characters"));
    }
    Ok(normalized)
}

/// The parent must belong to the user and have a compatible type (ACC-12). When moving an
/// existing account, the parent must also not be the account itself or one of its descendants.
pub async fn verify_parent(
    pool: &PgPool,
    user_id: Uuid,
    parent_id: Uuid,
    account_type: &str,
    moving_account: Option<Uuid>,
) -> Result<(), ApiError> {
    if moving_account == Some(parent_id) {
        return Err(bad_request("Account cannot be its own parent"));
    }

    let parent_type = sqlx::query_scalar::<_, String>(
        "SELECT account_type FROM accounts WHERE id = $1 AND user_id = $2",
    )
    .bind(parent_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| internal_error("Failed to query parent account", e))?
    .ok_or_else(|| bad_request("Parent account not found"))?;

    if !are_account_types_compatible(account_type, &parent_type) {
        return Err(bad_request(format!(
            "Incompatible account type '{}' for parent of type '{}' (ACC-12)",
            account_type, parent_type
        )));
    }

    if let Some(id) = moving_account {
        // Cycle check: verify that 'id' is NOT an ancestor of the new parent (Issue #18)
        let has_cycle = sqlx::query_scalar::<_, bool>(
            r#"WITH RECURSIVE anc AS (
                SELECT id, parent_id, 1 as depth FROM accounts WHERE id = $1 AND user_id = $2
                UNION
                SELECT a.id, a.parent_id, anc.depth + 1 FROM accounts a JOIN anc ON a.id = anc.parent_id WHERE anc.depth < 100
            )
            SELECT EXISTS (SELECT 1 FROM anc WHERE id = $3)"#
        )
        .bind(parent_id)
        .bind(user_id)
        .bind(id)
        .fetch_one(pool)
        .await
        .map_err(|e| internal_error("Failed to check account hierarchy for cycles", e))?;

        if has_cycle {
            return Err(bad_request("Cannot assign a descendant as parent (cycle detected)"));
        }
    }
    Ok(())
}

/// Every direct child account must stay type-compatible with the account (ACC-12).
pub async fn verify_children_compatible(
    pool: &PgPool,
    user_id: Uuid,
    account_id: Uuid,
    account_type: &str,
) -> Result<(), ApiError> {
    let child_types = sqlx::query_scalar::<_, String>(
        "SELECT DISTINCT account_type FROM accounts WHERE parent_id = $1 AND user_id = $2",
    )
    .bind(account_id)
    .bind(user_id)
    .fetch_all(pool)
    .await
    .map_err(|e| internal_error("Failed to query child accounts for update", e))?;

    for ct in child_types {
        if !are_account_types_compatible(account_type, &ct) {
            return Err(bad_request(format!(
                "Account type '{}' is incompatible with child account of type '{}' (ACC-12)",
                account_type, ct
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::StatusCode;
    use serde_json::json;

    #[test]
    fn test_valid_account_types_does_not_contain_root() {
        assert!(!VALID_ACCOUNT_TYPES.contains(&"root"));
    }

    #[test]
    fn test_account_type_group_rejects_root() {
        assert_eq!(account_type_group("root"), None);
        assert_eq!(account_type_group("ROOT"), None);
    }

    #[test]
    fn test_are_account_types_compatible_rejects_root() {
        assert!(!are_account_types_compatible("root", "asset"));
        assert!(!are_account_types_compatible("asset", "root"));
        assert!(!are_account_types_compatible("root", "root"));
    }

    #[test]
    fn test_are_account_types_compatible_groups() {
        // Balance sheet accounts compatible with each other
        assert!(are_account_types_compatible("bank", "asset"));
        assert!(are_account_types_compatible("asset", "liability"));
        assert!(are_account_types_compatible("credit", "payable"));

        // Income/expense compatible with each other
        assert!(are_account_types_compatible("income", "expense"));

        // Equity only compatible with equity
        assert!(are_account_types_compatible("equity", "equity"));

        // Incompatible across groups
        assert!(!are_account_types_compatible("asset", "expense"));
        assert!(!are_account_types_compatible("income", "liability"));
        assert!(!are_account_types_compatible("equity", "bank"));
    }

    #[test]
    fn test_sanitize_account_code_valid() {
        assert_eq!(sanitize_account_code(Some("1000")).unwrap(), Some("1000".to_string()));
        assert_eq!(sanitize_account_code(Some("  1010  ")).unwrap(), Some("1010".to_string()));
        assert_eq!(sanitize_account_code(Some("")).unwrap(), None);
        assert_eq!(sanitize_account_code(Some("   ")).unwrap(), None);
        assert_eq!(sanitize_account_code(None).unwrap(), None);
    }

    #[test]
    fn test_sanitize_account_code_boundary_50() {
        let code_50 = "a".repeat(50);
        assert_eq!(sanitize_account_code(Some(&code_50)).unwrap(), Some(code_50));

        let code_51 = "a".repeat(51);
        let err = sanitize_account_code(Some(&code_51)).unwrap_err();
        assert_eq!(err.0, StatusCode::BAD_REQUEST);
        assert_eq!(err.1 .0, json!({ "error": "Account code cannot exceed 50 characters" }));
    }
}
