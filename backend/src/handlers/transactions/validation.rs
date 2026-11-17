//! Validation shared by transaction create and update (ACC-1 to ACC-4).

use rust_decimal::Decimal;
use sqlx::PgConnection;
use std::collections::HashMap;
use uuid::Uuid;

use crate::handlers::{bad_request, internal_error, ApiError};
use crate::models::CreateSplitRequest;

/// Trims the description and checks it is 1-255 characters.
pub fn validate_description(description: &str) -> Result<String, ApiError> {
    let trimmed = description.trim();
    if trimmed.is_empty() || trimmed.len() > 255 {
        return Err(bad_request("Transaction description must be between 1 and 255 characters"));
    }
    Ok(trimmed.to_string())
}

/// Trims the num/reference (empty becomes None) and checks it is at most 50 characters.
pub fn validate_num(num: Option<String>) -> Result<Option<String>, ApiError> {
    let num = num.map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
    if num.as_ref().is_some_and(|n| n.len() > 50) {
        return Err(bad_request("Transaction num/reference cannot exceed 50 characters"));
    }
    Ok(num)
}

/// 2-100 splits, at most 2 decimal places, valid reconciled states, and amounts summing to exactly zero.
pub fn validate_splits(splits: &[CreateSplitRequest]) -> Result<(), ApiError> {
    if splits.len() < 2 || splits.len() > 100 {
        return Err(bad_request("A double-entry transaction must have between 2 and 100 splits"));
    }

    let mut sum = Decimal::ZERO;
    for split in splits {
        if split.amount.scale() > 2 {
            return Err(bad_request(format!(
                "Split amount {} cannot have more than 2 decimal places",
                split.amount
            )));
        }
        if let Some(ref rec) = split.reconciled_state {
            if !matches!(rec.to_lowercase().as_str(), "n" | "c" | "y") {
                return Err(bad_request(format!(
                    "Invalid reconciled_state '{}': must be 'n', 'c', or 'y'",
                    rec
                )));
            }
        }
        sum += split.amount;
    }

    if sum != Decimal::ZERO {
        return Err(bad_request(format!(
            "Transaction is not balanced! Sum of splits is {}, must be 0.00",
            sum
        )));
    }
    Ok(())
}

/// Checks every split account exists, belongs to the user and is not a placeholder.
/// Returns account id -> name for building responses.
pub async fn verify_split_accounts(
    conn: &mut PgConnection,
    user_id: Uuid,
    splits: &[CreateSplitRequest],
) -> Result<HashMap<Uuid, String>, ApiError> {
    let account_ids: Vec<Uuid> = splits.iter().map(|s| s.account_id).collect();
    let accounts: Vec<(Uuid, String, bool)> = sqlx::query_as(
        "SELECT id, name, placeholder FROM accounts WHERE id = ANY($1) AND user_id = $2",
    )
    .bind(&account_ids)
    .bind(user_id)
    .fetch_all(&mut *conn)
    .await
    .map_err(|e| internal_error("Failed to verify transaction accounts", e))?;

    let account_map: HashMap<Uuid, (String, bool)> = accounts
        .into_iter()
        .map(|(id, name, placeholder)| (id, (name, placeholder)))
        .collect();

    for split in splits {
        match account_map.get(&split.account_id) {
            None => {
                return Err(bad_request(format!(
                    "Account {} does not exist or does not belong to you",
                    split.account_id
                )));
            }
            Some((name, true)) => {
                return Err(bad_request(format!(
                    "Cannot post transactions to placeholder account '{}'",
                    name
                )));
            }
            Some(_) => {}
        }
    }

    Ok(account_map.into_iter().map(|(id, (name, _))| (id, name)).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::StatusCode;
    use rust_decimal_macros::dec;

    fn split(amount: Decimal, rec: Option<&str>) -> CreateSplitRequest {
        CreateSplitRequest {
            id: None,
            account_id: Uuid::new_v4(),
            amount,
            memo: None,
            reconciled_state: rec.map(str::to_string),
        }
    }

    fn error_text(err: ApiError) -> String {
        assert_eq!(err.0, StatusCode::BAD_REQUEST);
        err.1 .0["error"].as_str().unwrap().to_string()
    }

    #[test]
    fn accepts_balanced_two_decimal_splits() {
        assert!(validate_splits(&[split(dec!(10.50), None), split(dec!(-10.50), Some("Y"))]).is_ok());
    }

    #[test]
    fn rejects_wrong_split_counts() {
        assert!(error_text(validate_splits(&[split(dec!(0), None)]).unwrap_err()).contains("between 2 and 100"));
        let many: Vec<_> = (0..101).map(|_| split(dec!(0), None)).collect();
        assert!(validate_splits(&many).is_err());
    }

    #[test]
    fn rejects_more_than_two_decimals_unbalanced_and_bad_state() {
        let three_dp = validate_splits(&[split(dec!(1.005), None), split(dec!(-1.005), None)]).unwrap_err();
        assert!(error_text(three_dp).contains("more than 2 decimal places"));
        let unbalanced = validate_splits(&[split(dec!(10), None), split(dec!(-9.99), None)]).unwrap_err();
        assert!(error_text(unbalanced).contains("not balanced"));
        let bad_state = validate_splits(&[split(dec!(1), Some("x")), split(dec!(-1), None)]).unwrap_err();
        assert!(error_text(bad_state).contains("Invalid reconciled_state"));
    }

    #[test]
    fn description_and_num_are_trimmed_and_bounded() {
        assert_eq!(validate_description("  Rent  ").unwrap(), "Rent");
        assert!(validate_description("   ").is_err());
        assert!(validate_description(&"x".repeat(256)).is_err());
        assert_eq!(validate_num(Some("  ".into())).unwrap(), None);
        assert_eq!(validate_num(Some(" 42 ".into())).unwrap(), Some("42".into()));
        assert!(validate_num(Some("x".repeat(51))).is_err());
    }
}
