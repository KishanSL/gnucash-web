import React, { useMemo, useState } from 'react';
import { AccountTreeNode } from '../../types';

// Collapse/expand, hidden-account and search state for the accounts tree. Collapse state persists
// in localStorage (gnucash_accounts_collapsed_keys) across reloads (UI-5).

// Recursively collect all account IDs that have children
function getAllParentIds(nodes: AccountTreeNode[]): string[] {
  let ids: string[] = [];
  for (const node of nodes) {
    if (node.children && node.children.length > 0) {
      ids.push(node.account.id);
      ids = ids.concat(getAllParentIds(node.children));
    }
  }
  return ids;
}


// Parses stored collapsed state (supporting both array of keys and map of booleans)
function parseCollapsedState(raw: string | null): Record<string, boolean> | null {
  if (!raw) return null;
  try {
    const parsed = JSON.parse(raw);
    if (Array.isArray(parsed)) {
      if (parsed.length === 0) return {};
      const map: Record<string, boolean> = {};
      parsed.forEach((k: string) => {
        if (typeof k === 'string') map[k] = true;
      });
      return map;
    }
    if (typeof parsed === 'object' && parsed !== null) {
      return parsed;
    }
  } catch {
    // Ignore JSON parse error and fall back to default expansion
  }
  return null;
}

function persistCollapsedState(map: Record<string, boolean>) {
  const collapsedKeys = Object.keys(map).filter((k) => !!map[k]);
  localStorage.setItem('gnucash_accounts_collapsed_keys', JSON.stringify(collapsedKeys));
  localStorage.setItem('gnucash_accounts_collapsed_map', JSON.stringify(map));
}

export function useAccountsTreeState(tree: AccountTreeNode[]) {
  const [searchTerm, setSearchTerm] = useState('');

  const [collapsed, setCollapsed] = useState<Record<string, boolean>>(() => {
    const fromKeys = parseCollapsedState(localStorage.getItem('gnucash_accounts_collapsed_keys'));
    if (fromKeys && Object.keys(fromKeys).length > 0) {
      return fromKeys;
    }
    const fromMap = parseCollapsedState(localStorage.getItem('gnucash_accounts_collapsed_map'));
    if (fromMap && Object.keys(fromMap).length > 0) {
      return fromMap;
    }
    return fromKeys || fromMap || {};
  });
  const [hideSubaccounts, setHideSubaccounts] = useState<boolean>(() => {
    return localStorage.getItem('gnucash_accounts_hide_subaccounts') === 'true';
  });
  const [showHidden, setShowHidden] = useState<boolean>(() => {
    return localStorage.getItem('gnucash_show_hidden') === 'true';
  });

  const handleToggleShowHidden = () => {
    const next = !showHidden;
    setShowHidden(next);
    localStorage.setItem('gnucash_show_hidden', String(next));
  };

  const toggleCollapse = (id: string) => {
    setCollapsed((prev) => {
      const next = { ...prev, [id]: !prev[id] };
      persistCollapsedState(next);
      return next;
    });
  };

  // Synchronize collapsed state on initial load / refresh if hideSubaccounts was requested
  React.useEffect(() => {
    if (tree.length > 0 && hideSubaccounts) {
      const parentIds = getAllParentIds(tree);
      const newCollapsed: Record<string, boolean> = {};
      parentIds.forEach((id) => {
        newCollapsed[id] = true;
      });
      setCollapsed(newCollapsed);
      persistCollapsedState(newCollapsed);
    }
  }, [tree, hideSubaccounts]);

  // Check if all first-level root accounts are currently collapsed
  const allRootsCollapsed = useMemo(() => {
    if (!tree || tree.length === 0) return false;
    return tree.every((node) => collapsed[node.account.id]);
  }, [tree, collapsed]);

  // Toggle hiding / unhiding all sub-accounts (collapse tree to first level only vs expand all)
  const handleToggleCollapseAll = () => {
    if (allRootsCollapsed) {
      // Unhide / Expand all accounts
      setCollapsed({});
      setHideSubaccounts(false);
      localStorage.setItem('gnucash_accounts_hide_subaccounts', 'false');
      persistCollapsedState({});
    } else {
      // Hide all sub-accounts: collapse tree so only the first level is showing
      const parentIds = getAllParentIds(tree);
      const newCollapsed: Record<string, boolean> = {};
      parentIds.forEach((id) => {
        newCollapsed[id] = true;
      });
      setCollapsed(newCollapsed);
      setHideSubaccounts(true);
      localStorage.setItem('gnucash_accounts_hide_subaccounts', 'true');
      persistCollapsedState(newCollapsed);
    }
  };

  // Count total hidden accounts in the tree
  const hiddenCount = useMemo(() => {
    const countNodes = (nodes: AccountTreeNode[]): number => {
      let count = 0;
      for (const node of nodes) {
        if (node.account.hidden) count++;
        if (node.children && node.children.length > 0) {
          count += countNodes(node.children);
        }
      }
      return count;
    };
    return countNodes(tree);
  }, [tree]);

  return {
    searchTerm,
    setSearchTerm,
    collapsed,
    showHidden,
    handleToggleShowHidden,
    toggleCollapse,
    allRootsCollapsed,
    handleToggleCollapseAll,
    hiddenCount,
  };
}
