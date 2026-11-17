//! Matching submitted splits to a transaction's existing splits on update, so split ids and
//! reconciled state are preserved where nothing material changed (Issue #16, REC-*).

use chrono::{DateTime, Utc};
use std::collections::HashSet;
use uuid::Uuid;

use crate::models::{CreateSplitRequest, SplitRow};

/// For each new split, the existing split it replaces (if any): first by explicit split id,
/// then by identical account and amount. Each existing split is matched at most once.
pub fn match_existing_splits(new_splits: &[CreateSplitRequest], existing: &[SplitRow]) -> Vec<Option<SplitRow>> {
    let mut matched: Vec<Option<SplitRow>> = vec![None; new_splits.len()];
    let mut used: HashSet<Uuid> = HashSet::new();

    // Pass 1: Match by explicit split id
    for (i, s) in new_splits.iter().enumerate() {
        if let Some(split_id) = s.id {
            if let Some(e) = existing.iter().find(|e| e.id == split_id && !used.contains(&e.id)) {
                matched[i] = Some(e.clone());
                used.insert(e.id);
            }
        }
    }

    // Pass 2: Match by account_id and amount for remaining unmatched splits
    for (i, s) in new_splits.iter().enumerate() {
        if matched[i].is_none() {
            if let Some(e) = existing
                .iter()
                .find(|e| !used.contains(&e.id) && e.account_id == s.account_id && e.amount == s.amount)
            {
                matched[i] = Some(e.clone());
                used.insert(e.id);
            }
        }
    }

    matched
}

/// Reconciled state and date for a submitted split. Reconciled status is kept only when the
/// split's account and amount are unchanged (Issue #16); otherwise it resets to 'n'.
pub fn resolve_reconcile_state(
    new_split: &CreateSplitRequest,
    matched: Option<&SplitRow>,
) -> (String, Option<DateTime<Utc>>) {
    match matched {
        Some(existing) if existing.account_id == new_split.account_id && existing.amount == new_split.amount => {
            match &new_split.reconciled_state {
                Some(req_state) => {
                    let norm = req_state.to_lowercase();
                    let date = if norm == "y" {
                        existing.reconciled_date.or_else(|| Some(Utc::now()))
                    } else {
                        None
                    };
                    (norm, date)
                }
                None => (existing.reconciled_state.clone(), existing.reconciled_date),
            }
        }
        // Amount or account changed -> must reset to unreconciled 'n'
        Some(_) => ("n".to_string(), None),
        None => {
            let norm = new_split.reconciled_state.as_deref().unwrap_or("n").to_lowercase();
            let date = if norm == "y" { Some(Utc::now()) } else { None };
            (norm, date)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::Decimal;
    use rust_decimal_macros::dec;

    fn existing(account_id: Uuid, amount: Decimal, state: &str) -> SplitRow {
        SplitRow {
            id: Uuid::new_v4(),
            transaction_id: Uuid::new_v4(),
            account_id,
            amount,
            memo: None,
            reconciled_state: state.to_string(),
            reconciled_date: if state == "y" { Some(Utc::now()) } else { None },
        }
    }

    fn new_split(id: Option<Uuid>, account_id: Uuid, amount: Decimal, state: Option<&str>) -> CreateSplitRequest {
        CreateSplitRequest { id, account_id, amount, memo: None, reconciled_state: state.map(str::to_string) }
    }

    #[test]
    fn matches_by_id_first_then_by_account_and_amount_once_each() {
        let (a, b) = (Uuid::new_v4(), Uuid::new_v4());
        let e1 = existing(a, dec!(10), "y");
        let e2 = existing(b, dec!(-10), "c");
        let news = vec![
            new_split(None, b, dec!(-10), None),     // matches e2 by account + amount
            new_split(Some(e1.id), a, dec!(20), None), // matches e1 by id even though amount changed
            new_split(None, b, dec!(-10), None),     // e2 already used -> no match
        ];
        let matched = match_existing_splits(&news, &[e1.clone(), e2.clone()]);
        assert_eq!(matched[0].as_ref().map(|m| m.id), Some(e2.id));
        assert_eq!(matched[1].as_ref().map(|m| m.id), Some(e1.id));
        assert!(matched[2].is_none());
    }

    #[test]
    fn keeps_reconciled_state_only_when_account_and_amount_unchanged() {
        let a = Uuid::new_v4();
        let e = existing(a, dec!(10), "y");
        let unchanged = resolve_reconcile_state(&new_split(Some(e.id), a, dec!(10), None), Some(&e));
        assert_eq!(unchanged, ("y".to_string(), e.reconciled_date));

        let changed = resolve_reconcile_state(&new_split(Some(e.id), a, dec!(11), Some("y")), Some(&e));
        assert_eq!(changed, ("n".to_string(), None));
    }

    #[test]
    fn new_splits_use_requested_state_and_only_y_gets_a_date() {
        let a = Uuid::new_v4();
        let (state, date) = resolve_reconcile_state(&new_split(None, a, dec!(5), Some("Y")), None);
        assert_eq!(state, "y");
        assert!(date.is_some());
        assert_eq!(resolve_reconcile_state(&new_split(None, a, dec!(5), Some("c")), None), ("c".to_string(), None));
        assert_eq!(resolve_reconcile_state(&new_split(None, a, dec!(5), None), None), ("n".to_string(), None));
    }
}
