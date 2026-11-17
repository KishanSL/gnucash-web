//! Recursive balance roll-up and account tree construction.

use rust_decimal::Decimal;
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

use crate::models::{calculate_display_balance, Account, AccountRow, AccountTreeNode};

pub fn build_account_list(rows: Vec<AccountRow>, balance_map: &HashMap<Uuid, Decimal>) -> Vec<Account> {
    // Map of id -> direct balance
    let mut direct_balances: HashMap<Uuid, Decimal> = HashMap::new();
    let mut parent_to_children: HashMap<Option<Uuid>, Vec<Uuid>> = HashMap::new();

    for r in &rows {
        let b = balance_map.get(&r.id).copied().unwrap_or(Decimal::ZERO);
        direct_balances.insert(r.id, b);
        parent_to_children
            .entry(r.parent_id)
            .or_default()
            .push(r.id);
    }

    // Compute total recursive balance with cycle guard
    fn calc_total_balance(
        acc_id: Uuid,
        direct_balances: &HashMap<Uuid, Decimal>,
        parent_to_children: &HashMap<Option<Uuid>, Vec<Uuid>>,
        visited: &mut HashSet<Uuid>,
    ) -> Decimal {
        if !visited.insert(acc_id) {
            return Decimal::ZERO;
        }
        let direct = direct_balances.get(&acc_id).copied().unwrap_or(Decimal::ZERO);
        let children = parent_to_children.get(&Some(acc_id));
        let children_total: Decimal = match children {
            Some(c_ids) => c_ids
                .iter()
                .map(|&cid| calc_total_balance(cid, direct_balances, parent_to_children, visited))
                .sum(),
            None => Decimal::ZERO,
        };
        visited.remove(&acc_id);
        direct + children_total
    }

    let mut accounts = Vec::new();
    for r in rows {
        let direct_b = direct_balances.get(&r.id).copied().unwrap_or(Decimal::ZERO);
        let mut visited = HashSet::new();
        let total_b = calc_total_balance(r.id, &direct_balances, &parent_to_children, &mut visited);
        let display_b = calculate_display_balance(&r.account_type, total_b);

        accounts.push(Account {
            id: r.id,
            name: r.name,
            code: r.code,
            account_type: r.account_type,
            parent_id: r.parent_id,
            currency: r.currency,
            description: r.description,
            placeholder: r.placeholder,
            hidden: r.hidden,
            commodity_fraction: r.commodity_fraction,
            balance: direct_b,
            total_balance: total_b,
            display_balance: display_b,
            created_at: r.created_at,
            updated_at: r.updated_at,
        });
    }

    accounts
}

pub fn build_tree_structure(accounts: Vec<Account>) -> Vec<AccountTreeNode> {
    let mut account_map: HashMap<Uuid, Account> = HashMap::new();
    let mut parent_to_children: HashMap<Option<Uuid>, Vec<Uuid>> = HashMap::new();

    for acc in accounts {
        let id = acc.id;
        let parent_id = acc.parent_id;
        account_map.insert(id, acc);
        parent_to_children.entry(parent_id).or_default().push(id);
    }

    fn build_node(
        acc_id: Uuid,
        account_map: &HashMap<Uuid, Account>,
        parent_to_children: &HashMap<Option<Uuid>, Vec<Uuid>>,
        visited: &mut HashSet<Uuid>,
    ) -> Option<AccountTreeNode> {
        if !visited.insert(acc_id) {
            return None;
        }
        let account = account_map.get(&acc_id)?.clone();
        let child_ids = parent_to_children.get(&Some(acc_id));
        let mut children = Vec::new();
        if let Some(c_ids) = child_ids {
            for &cid in c_ids {
                if let Some(child_node) = build_node(cid, account_map, parent_to_children, visited) {
                    children.push(child_node);
                }
            }
        }
        visited.remove(&acc_id);
        Some(AccountTreeNode { account, children })
    }

    let mut roots = Vec::new();
    if let Some(root_ids) = parent_to_children.get(&None) {
        for &rid in root_ids {
            let mut visited = HashSet::new();
            if let Some(root_node) = build_node(rid, &account_map, &parent_to_children, &mut visited) {
                roots.push(root_node);
            }
        }
    }

    roots
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use rust_decimal_macros::dec;

    fn row(id: Uuid, parent_id: Option<Uuid>, account_type: &str) -> AccountRow {
        AccountRow {
            id,
            name: format!("acc-{}", id.simple()),
            code: None,
            account_type: account_type.to_string(),
            parent_id,
            currency: "INR".to_string(),
            description: None,
            placeholder: false,
            hidden: false,
            commodity_fraction: Some(100),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn totals_roll_up_to_parents_and_tree_nests_children() {
        let (assets, bank, cash) = (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());
        let rows = vec![row(assets, None, "asset"), row(bank, Some(assets), "bank"), row(cash, Some(assets), "cash")];
        let balances = HashMap::from([(assets, dec!(1.00)), (bank, dec!(100.50)), (cash, dec!(-20.25))]);

        let accounts = build_account_list(rows, &balances);
        let parent = accounts.iter().find(|a| a.id == assets).unwrap();
        assert_eq!(parent.balance, dec!(1.00));
        assert_eq!(parent.total_balance, dec!(81.25));

        let tree = build_tree_structure(accounts);
        assert_eq!(tree.len(), 1);
        assert_eq!(tree[0].account.id, assets);
        assert_eq!(tree[0].children.len(), 2);
    }

    #[test]
    fn a_parent_cycle_does_not_recurse_forever() {
        let (a, b) = (Uuid::new_v4(), Uuid::new_v4());
        let rows = vec![row(a, Some(b), "asset"), row(b, Some(a), "asset")];
        let balances = HashMap::from([(a, dec!(5)), (b, dec!(7))]);
        let accounts = build_account_list(rows, &balances);
        assert_eq!(accounts.iter().find(|x| x.id == a).unwrap().total_balance, dec!(12));
        // Neither account is a root, so the tree is empty rather than infinite
        assert!(build_tree_structure(accounts).is_empty());
    }
}
