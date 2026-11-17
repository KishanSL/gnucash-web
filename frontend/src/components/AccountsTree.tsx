import React from 'react';
import {
  FolderTree,
  Search,
  Plus,
  Eye,
  EyeOff,
  ChevronsDownUp,
  ChevronsUpDown,
} from 'lucide-react';
import { Account, AccountTreeNode } from '../types';
import { useAccountsTreeState } from './accounts_tree/useAccountsTreeState';
import { AccountTreeRow } from './accounts_tree/AccountTreeRow';
import { AccountSummaryCards } from './accounts_tree/AccountSummaryCards';

interface AccountsTreeProps {
  tree: AccountTreeNode[];
  currentCurrency: string;
  onSelectAccountForRegister: (account: Account) => void;
  onSelectAccountForReconcile: (account: Account) => void;
  onDeleteAccount: (id: string) => void;
  onOpenNewAccount: (parentId?: string) => void;
  onEditAccount: (account: Account) => void;
  onToggleHideAccount: (account: Account) => void;
}

export const AccountsTree: React.FC<AccountsTreeProps> = ({
  tree,
  currentCurrency,
  onSelectAccountForRegister,
  onSelectAccountForReconcile,
  onDeleteAccount,
  onOpenNewAccount,
  onEditAccount,
  onToggleHideAccount,
}) => {
  const {
    searchTerm,
    setSearchTerm,
    collapsed,
    showHidden,
    handleToggleShowHidden,
    toggleCollapse,
    allRootsCollapsed,
    handleToggleCollapseAll,
    hiddenCount,
  } = useAccountsTreeState(tree);

  // Compute summary stats
  const findRootTotal = (name: string): number => {
    const root = tree.find((node) => node.account.name.toLowerCase().includes(name.toLowerCase()));
    return root ? parseFloat(root.account.display_balance || '0') : 0;
  };

  const assetsTotal = findRootTotal('Assets');
  const liabTotal = findRootTotal('Liabilities');
  const equityTotal = findRootTotal('Equity');
  const netWorth = assetsTotal - liabTotal;

  const rowProps = {
    collapsed,
    showHidden,
    searchTerm,
    currentCurrency,
    toggleCollapse,
    onSelectAccountForRegister,
    onSelectAccountForReconcile,
    onDeleteAccount,
    onOpenNewAccount,
    onEditAccount,
    onToggleHideAccount,
  };

  return (
    <div className="space-y-4 sm:space-y-6">
      {/* Top Summary Cards */}
      <AccountSummaryCards
        netWorth={netWorth}
        assetsTotal={assetsTotal}
        liabTotal={liabTotal}
        equityTotal={equityTotal}
        currentCurrency={currentCurrency}
      />

      {/* Main Table Container */}
      <div className="bg-white rounded-xl border border-slate-200 shadow-sm overflow-hidden">
        {/* Table Toolbar */}
        <div className="p-3 sm:p-4 border-b border-slate-200 bg-slate-50/50 flex flex-col sm:flex-row items-stretch sm:items-center justify-between gap-2.5 sm:gap-3">
          <div className="flex items-center space-x-2 flex-wrap gap-y-1">
            <FolderTree className="w-5 h-5 text-emerald-600 shrink-0" />
            <h2 className="font-bold text-slate-800 text-sm sm:text-base">Chart of Accounts</h2>
            <span className="text-[11px] text-slate-500 bg-slate-200 px-2 py-0.5 rounded-full font-medium">
              Tree Hierarchy
            </span>
          </div>

          <div className="flex items-center space-x-2 w-full sm:w-auto flex-wrap sm:flex-nowrap gap-y-2">
            {/* Search Input */}
            <div className="relative flex-1 sm:w-60 min-w-[150px]">
              <Search className="w-3.5 h-3.5 text-slate-400 absolute left-3 top-1/2 -translate-y-1/2" />
              <input
                type="text"
                placeholder="Filter accounts..."
                value={searchTerm}
                onChange={(e) => setSearchTerm(e.target.value)}
                className="w-full pl-8 pr-3 py-1.5 text-xs bg-white border border-slate-300 rounded-lg focus:outline-none focus:ring-2 focus:ring-emerald-500/20 focus:border-emerald-500"
              />
            </div>

            {/* Hide / Unhide Sub-accounts (Collapse to 1st Level / Expand All) */}
            <button
              type="button"
              onClick={handleToggleCollapseAll}
              className={`flex items-center space-x-1.5 px-2.5 sm:px-3 py-1.5 text-xs font-semibold rounded-lg border transition shrink-0 ${
                allRootsCollapsed
                  ? 'bg-slate-800 text-white border-slate-900 shadow-sm hover:bg-slate-700'
                  : 'bg-white text-slate-700 border-slate-300 hover:bg-slate-50'
              }`}
              title={
                allRootsCollapsed
                  ? 'Unhide / Expand all accounts (show full tree structure)'
                  : 'Hide all sub-accounts (collapse tree to first level only)'
              }
            >
              {allRootsCollapsed ? (
                <ChevronsUpDown className="w-3.5 h-3.5 text-emerald-400" />
              ) : (
                <ChevronsDownUp className="w-3.5 h-3.5 text-slate-500" />
              )}
              <span>{allRootsCollapsed ? 'Show All Accounts' : 'Hide Subaccounts'}</span>
            </button>

            {/* Show / Hide Hidden Accounts Toggle Button */}
            <button
              type="button"
              onClick={handleToggleShowHidden}
              className={`flex items-center space-x-1.5 px-2.5 sm:px-3 py-1.5 text-xs font-semibold rounded-lg border transition shrink-0 ${
                showHidden
                  ? 'bg-amber-500 text-white border-amber-600 shadow-sm hover:bg-amber-600'
                  : 'bg-white text-slate-600 border-slate-300 hover:bg-slate-50'
              }`}
              title={showHidden ? 'Click to hide hidden accounts' : 'Click to show hidden accounts'}
            >
              {showHidden ? (
                <Eye className="w-3.5 h-3.5" />
              ) : (
                <EyeOff className="w-3.5 h-3.5 text-slate-400" />
              )}
              <span>{showHidden ? 'Hidden Accounts Visible' : 'Show Hidden'}</span>
              {hiddenCount > 0 && (
                <span
                  className={`text-[10px] px-1.5 py-0.2 rounded-full font-bold ${
                    showHidden ? 'bg-amber-700 text-amber-100' : 'bg-slate-200 text-slate-600'
                  }`}
                >
                  {hiddenCount}
                </span>
              )}
            </button>

            <button
              onClick={() => onOpenNewAccount()}
              className="flex items-center space-x-1 px-3 py-1.5 text-xs font-semibold bg-emerald-600 hover:bg-emerald-500 text-white rounded-lg transition shrink-0"
            >
              <Plus className="w-4 h-4" />
              <span className="hidden xs:inline">Add Account</span>
            </button>
          </div>
        </div>

        {/* Table */}
        <div className="overflow-x-auto">
          <table className="w-full text-left border-collapse">
            <thead>
              <tr className="bg-slate-100/80 border-b border-slate-200 text-slate-600 text-[11px] sm:text-xs font-semibold uppercase tracking-wider">
                <th className="py-2.5 sm:py-3 px-3 sm:px-4">Account Name</th>
                <th className="hidden md:table-cell py-2.5 sm:py-3 px-3 w-24">Code</th>
                <th className="hidden sm:table-cell py-2.5 sm:py-3 px-3 w-28">Type</th>
                <th className="hidden lg:table-cell py-2.5 sm:py-3 px-4 w-36 text-right">Direct Balance</th>
                <th className="py-2.5 sm:py-3 px-2 sm:px-4 w-28 sm:w-36 text-right">Total Balance</th>
                <th className="py-2.5 sm:py-3 px-2 sm:px-4 w-24 sm:w-44 text-right">Actions</th>
              </tr>
            </thead>
            <tbody>
              {tree.length === 0 ? (
                <tr>
                  <td colSpan={6} className="py-12 text-center text-slate-400">
                    No accounts found. Click "Seed Data" to create the standard Chart of Accounts.
                  </td>
                </tr>
              ) : (
                tree.map((rootNode) => (
                  <AccountTreeRow key={rootNode.account.id} {...rowProps} node={rootNode} depth={0} />
                ))
              )}
            </tbody>
          </table>
        </div>
      </div>
    </div>
  );
};
