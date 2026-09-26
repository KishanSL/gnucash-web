import React, { useState, useMemo } from 'react';
import {
  ChevronRight,
  ChevronDown,
  Folder,
  FolderOpen,
  FolderTree,
  Landmark,
  Wallet,
  CreditCard,
  TrendingUp,
  Receipt,
  Scale,
  BookOpen,
  CheckCircle2,
  Trash2,
  Search,
  Plus,
  Eye,
  EyeOff,
  Edit3,
} from 'lucide-react';
import { Account, AccountTreeNode } from '../types';
import { formatCurrency } from '../api';

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
  const [searchTerm, setSearchTerm] = useState('');
  const [collapsed, setCollapsed] = useState<Record<string, boolean>>({});
  const [showHidden, setShowHidden] = useState<boolean>(() => {
    return localStorage.getItem('gnucash_show_hidden') === 'true';
  });

  const handleToggleShowHidden = () => {
    const next = !showHidden;
    setShowHidden(next);
    localStorage.setItem('gnucash_show_hidden', String(next));
  };

  const toggleCollapse = (id: string) => {
    setCollapsed((prev) => ({ ...prev, [id]: !prev[id] }));
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

  // Compute summary stats
  const findRootTotal = (name: string): number => {
    const root = tree.find((node) => node.account.name.toLowerCase().includes(name.toLowerCase()));
    return root ? parseFloat(root.account.display_balance || '0') : 0;
  };

  const assetsTotal = findRootTotal('Assets');
  const liabTotal = findRootTotal('Liabilities');
  const equityTotal = findRootTotal('Equity');
  const netWorth = assetsTotal - liabTotal;

  const getAccountTypeIcon = (type: string, isPlaceholder: boolean) => {
    switch (type.toLowerCase()) {
      case 'bank':
        return <Landmark className="w-4 h-4 text-sky-600" />;
      case 'cash':
        return <Wallet className="w-4 h-4 text-emerald-600" />;
      case 'credit':
        return <CreditCard className="w-4 h-4 text-amber-600" />;
      case 'income':
        return <TrendingUp className="w-4 h-4 text-emerald-600" />;
      case 'expense':
        return <Receipt className="w-4 h-4 text-rose-500" />;
      case 'equity':
        return <Scale className="w-4 h-4 text-purple-600" />;
      case 'asset':
      case 'liability':
      default:
        return isPlaceholder ? (
          <Folder className="w-4 h-4 text-amber-500" />
        ) : (
          <FolderOpen className="w-4 h-4 text-slate-500" />
        );
    }
  };

  const getBadgeStyle = (type: string) => {
    switch (type.toLowerCase()) {
      case 'bank':
      case 'asset':
      case 'cash':
      case 'receivable':
        return 'bg-sky-50 text-sky-700 border-sky-200';
      case 'liability':
      case 'credit':
      case 'payable':
        return 'bg-amber-50 text-amber-700 border-amber-200';
      case 'equity':
        return 'bg-purple-50 text-purple-700 border-purple-200';
      case 'income':
        return 'bg-emerald-50 text-emerald-700 border-emerald-200';
      case 'expense':
        return 'bg-rose-50 text-rose-700 border-rose-200';
      default:
        return 'bg-slate-100 text-slate-700 border-slate-200';
    }
  };

  const hasNonHiddenDescendant = (childrenList: AccountTreeNode[]): boolean => {
    return childrenList.some(
      (c) => !c.account.hidden || hasNonHiddenDescendant(c.children)
    );
  };

  const renderNode = (node: AccountTreeNode, depth = 0): React.ReactNode => {
    const { account, children } = node;
    const isCollapsed = collapsed[account.id] || false;
    const hasChildren = children && children.length > 0;

    // Hidden account filter
    // If hidden and showHidden is false, omit unless it contains non-hidden descendants
    if (!showHidden && account.hidden && !hasNonHiddenDescendant(children)) {
      return null;
    }

    // Search filter
    const matchesSearch =
      !searchTerm ||
      account.name.toLowerCase().includes(searchTerm.toLowerCase()) ||
      (account.code && account.code.toLowerCase().includes(searchTerm.toLowerCase())) ||
      account.account_type.toLowerCase().includes(searchTerm.toLowerCase());

    const childrenHaveMatches = (childrenList: AccountTreeNode[]): boolean => {
      return childrenList.some(
        (c) =>
          ((showHidden || !c.account.hidden || hasNonHiddenDescendant(c.children)) &&
            (c.account.name.toLowerCase().includes(searchTerm.toLowerCase()) ||
              (c.account.code && c.account.code.toLowerCase().includes(searchTerm.toLowerCase())) ||
              c.account.account_type.toLowerCase().includes(searchTerm.toLowerCase()))) ||
          childrenHaveMatches(c.children)
      );
    };

    if (searchTerm && !matchesSearch && !childrenHaveMatches(children)) {
      return null;
    }

    const isHiddenAccount = account.hidden;

    return (
      <React.Fragment key={account.id}>
        <tr
          className={`border-b border-slate-100 transition-colors ${
            isHiddenAccount
              ? 'bg-amber-50/50 hover:bg-amber-100/40 text-slate-600 italic'
              : account.placeholder
              ? 'bg-slate-50/80 font-semibold text-slate-800'
              : 'hover:bg-emerald-50/40 text-slate-700'
          }`}
        >
          {/* Account Name with responsive tree indentation */}
          <td className="py-2.5 px-3 sm:px-4">
            <div
              className="flex items-center"
              style={{ paddingLeft: `${Math.min(depth * 14, 42)}px` }}
            >
              {hasChildren ? (
                <button
                  onClick={() => toggleCollapse(account.id)}
                  className="p-1 mr-1 text-slate-400 hover:text-slate-700 rounded hover:bg-slate-200/60 transition shrink-0"
                  title={isCollapsed ? 'Expand' : 'Collapse'}
                >
                  {isCollapsed ? (
                    <ChevronRight className="w-4 h-4" />
                  ) : (
                    <ChevronDown className="w-4 h-4" />
                  )}
                </button>
              ) : (
                <span className="w-5 inline-block shrink-0" />
              )}

              <span className="mr-2 shrink-0">{getAccountTypeIcon(account.account_type, account.placeholder)}</span>

              <div className="flex flex-col min-w-0">
                <div className="flex items-center space-x-1.5 flex-wrap gap-y-0.5">
                  {!account.placeholder ? (
                    <button
                      onClick={() => onSelectAccountForRegister(account)}
                      className={`font-medium hover:text-emerald-700 text-left truncate text-xs sm:text-sm hover:underline ${
                        isHiddenAccount ? 'text-slate-600' : 'text-slate-800'
                      }`}
                      title="Open in Ledger Register"
                    >
                      {account.name}
                    </button>
                  ) : (
                    <span className="font-bold text-slate-800 truncate text-xs sm:text-sm">
                      {account.name}
                    </span>
                  )}

                  {/* Hidden badge */}
                  {isHiddenAccount && (
                    <span
                      className="inline-flex items-center space-x-0.5 text-[9px] uppercase font-bold tracking-wider px-1.5 py-0.2 rounded bg-amber-100 text-amber-800 border border-amber-300 shrink-0 not-italic"
                      title="This account is hidden"
                    >
                      <EyeOff className="w-2.5 h-2.5 text-amber-700" />
                      <span>Hidden</span>
                    </span>
                  )}

                  {/* Placeholder Folder badge */}
                  {account.placeholder && (
                    <span className="text-[9px] uppercase font-bold tracking-wider px-1 py-0.2 rounded bg-slate-200 text-slate-600 shrink-0 not-italic">
                      Folder
                    </span>
                  )}

                  {/* Mobile Type badge */}
                  <span
                    className={`sm:hidden inline-block px-1.5 py-0.2 rounded text-[10px] font-medium border capitalize not-italic ${getBadgeStyle(
                      account.account_type
                    )} shrink-0`}
                  >
                    {account.account_type}
                  </span>
                </div>
                {account.description && (
                  <span className="text-[10px] sm:text-[11px] text-slate-400 font-normal truncate max-w-xs not-italic">
                    {account.description}
                  </span>
                )}
              </div>
            </div>
          </td>

          {/* Account Code (Hidden on mobile) */}
          <td className="hidden md:table-cell py-2.5 px-3 text-slate-500 font-mono text-xs">
            {account.code || '—'}
          </td>

          {/* Account Type (Hidden on mobile, shown inline) */}
          <td className="hidden sm:table-cell py-2.5 px-3">
            <span
              className={`inline-block px-2 py-0.5 rounded-md text-xs font-medium border capitalize not-italic ${getBadgeStyle(
                account.account_type
              )}`}
            >
              {account.account_type}
            </span>
          </td>

          {/* Direct Balance (Hidden on tablet/mobile) */}
          <td className="hidden lg:table-cell py-2.5 px-4 text-right font-mono text-xs not-italic">
            {!account.placeholder ? (
              <span
                className={
                  parseFloat(account.display_balance) < 0
                    ? 'text-rose-600 font-semibold'
                    : isHiddenAccount
                    ? 'text-slate-500'
                    : 'text-slate-700'
                }
              >
                {formatCurrency(account.display_balance, currentCurrency || account.currency)}
              </span>
            ) : (
              <span className="text-slate-300">—</span>
            )}
          </td>

          {/* Total Subtree Balance */}
          <td className="py-2.5 px-2 sm:px-4 text-right font-mono text-xs font-semibold whitespace-nowrap not-italic">
            <span
              className={
                parseFloat(account.display_balance) < 0
                  ? 'text-rose-600'
                  : isHiddenAccount
                  ? 'text-slate-600'
                  : 'text-slate-900'
              }
            >
              {formatCurrency(account.display_balance, currentCurrency || account.currency)}
            </span>
          </td>

          {/* Actions */}
          <td className="py-2.5 px-2 sm:px-4 text-right whitespace-nowrap not-italic">
            <div className="flex items-center justify-end space-x-1">
              {!account.placeholder && (
                <>
                  <button
                    onClick={() => onSelectAccountForRegister(account)}
                    className="flex items-center space-x-1 px-2 py-1 text-xs font-medium text-emerald-700 bg-emerald-50 hover:bg-emerald-100 rounded border border-emerald-200 transition"
                    title="Open Ledger Register"
                  >
                    <BookOpen className="w-3.5 h-3.5" />
                    <span className="hidden sm:inline">Register</span>
                  </button>

                  {['bank', 'credit', 'cash'].includes(account.account_type.toLowerCase()) && (
                    <button
                      onClick={() => onSelectAccountForReconcile(account)}
                      className="p-1 text-slate-500 hover:text-emerald-600 rounded hover:bg-emerald-50 border border-slate-200 transition"
                      title="Reconcile Account"
                    >
                      <CheckCircle2 className="w-3.5 h-3.5" />
                    </button>
                  )}
                </>
              )}

              {account.placeholder && (
                <button
                  onClick={() => onOpenNewAccount(account.id)}
                  className="flex items-center space-x-1 px-2 py-1 text-[11px] font-medium text-slate-600 bg-slate-100 hover:bg-slate-200 rounded border border-slate-200 transition"
                  title="Add Subaccount"
                >
                  <Plus className="w-3 h-3" />
                  <span className="hidden sm:inline">Subaccount</span>
                </button>
              )}

              {/* 1-Click Hide/Unhide Quick Toggle */}
              <button
                onClick={() => onToggleHideAccount(account)}
                className={`p-1 rounded transition border ${
                  account.hidden
                    ? 'text-amber-700 bg-amber-100 hover:bg-amber-200 border-amber-300'
                    : 'text-slate-400 hover:text-slate-700 hover:bg-slate-100 border-transparent hover:border-slate-200'
                }`}
                title={account.hidden ? 'Unhide Account (Make visible)' : 'Hide Account (Omit from tree)'}
              >
                {account.hidden ? <EyeOff className="w-3.5 h-3.5" /> : <Eye className="w-3.5 h-3.5" />}
              </button>

              {/* Edit Account */}
              <button
                onClick={() => onEditAccount(account)}
                className="p-1 text-slate-400 hover:text-emerald-700 rounded hover:bg-emerald-50 border border-transparent hover:border-emerald-200 transition"
                title="Edit Account Details"
              >
                <Edit3 className="w-3.5 h-3.5" />
              </button>

              {!account.placeholder && parseFloat(account.balance) === 0 && (
                <button
                  onClick={() => onDeleteAccount(account.id)}
                  className="p-1 text-slate-400 hover:text-rose-600 rounded hover:bg-rose-50 border border-transparent hover:border-rose-200 transition"
                  title="Delete empty account"
                >
                  <Trash2 className="w-3.5 h-3.5" />
                </button>
              )}
            </div>
          </td>
        </tr>

        {/* Children Rows */}
        {!isCollapsed &&
          children &&
          children.map((child) => renderNode(child, depth + 1))}
      </React.Fragment>
    );
  };

  return (
    <div className="space-y-4 sm:space-y-6">
      {/* Top Summary Cards */}
      <div className="grid grid-cols-2 lg:grid-cols-4 gap-2.5 sm:gap-4">
        {/* Net Worth */}
        <div className="bg-white p-3 sm:p-4 rounded-xl border border-slate-200 shadow-sm flex items-center justify-between">
          <div>
            <p className="text-[10px] sm:text-xs font-semibold text-slate-500 uppercase tracking-wider">Net Worth</p>
            <h3 className="text-base sm:text-xl font-bold font-mono text-slate-900 mt-0.5 sm:mt-1 truncate">
              {formatCurrency(netWorth, currentCurrency)}
            </h3>
            <p className="text-[10px] sm:text-[11px] text-slate-400 mt-0.5 hidden xs:block">Assets minus Liabilities</p>
          </div>
          <div className="w-8 h-8 sm:w-10 sm:h-10 rounded-lg bg-emerald-50 border border-emerald-100 flex items-center justify-center text-emerald-600 shrink-0">
            <Scale className="w-4 h-4 sm:w-5 sm:h-5" />
          </div>
        </div>

        {/* Total Assets */}
        <div className="bg-white p-3 sm:p-4 rounded-xl border border-slate-200 shadow-sm flex items-center justify-between">
          <div>
            <p className="text-[10px] sm:text-xs font-semibold text-slate-500 uppercase tracking-wider">Total Assets</p>
            <h3 className="text-base sm:text-xl font-bold font-mono text-sky-700 mt-0.5 sm:mt-1 truncate">
              {formatCurrency(assetsTotal, currentCurrency)}
            </h3>
            <p className="text-[10px] sm:text-[11px] text-slate-400 mt-0.5 hidden xs:block">Cash, Bank, Property</p>
          </div>
          <div className="w-8 h-8 sm:w-10 sm:h-10 rounded-lg bg-sky-50 border border-sky-100 flex items-center justify-center text-sky-600 shrink-0">
            <Landmark className="w-4 h-4 sm:w-5 sm:h-5" />
          </div>
        </div>

        {/* Total Liabilities */}
        <div className="bg-white p-3 sm:p-4 rounded-xl border border-slate-200 shadow-sm flex items-center justify-between">
          <div>
            <p className="text-[10px] sm:text-xs font-semibold text-slate-500 uppercase tracking-wider">Liabilities</p>
            <h3 className="text-base sm:text-xl font-bold font-mono text-amber-700 mt-0.5 sm:mt-1 truncate">
              {formatCurrency(liabTotal, currentCurrency)}
            </h3>
            <p className="text-[10px] sm:text-[11px] text-slate-400 mt-0.5 hidden xs:block">Credit Cards, Payables</p>
          </div>
          <div className="w-8 h-8 sm:w-10 sm:h-10 rounded-lg bg-amber-50 border border-amber-100 flex items-center justify-center text-amber-600 shrink-0">
            <CreditCard className="w-4 h-4 sm:w-5 sm:h-5" />
          </div>
        </div>

        {/* Total Equity */}
        <div className="bg-white p-3 sm:p-4 rounded-xl border border-slate-200 shadow-sm flex items-center justify-between">
          <div>
            <p className="text-[10px] sm:text-xs font-semibold text-slate-500 uppercase tracking-wider">Equity</p>
            <h3 className="text-base sm:text-xl font-bold font-mono text-purple-700 mt-0.5 sm:mt-1 truncate">
              {formatCurrency(equityTotal, currentCurrency)}
            </h3>
            <p className="text-[10px] sm:text-[11px] text-slate-400 mt-0.5 hidden xs:block">Opening Balances & Earnings</p>
          </div>
          <div className="w-8 h-8 sm:w-10 sm:h-10 rounded-lg bg-purple-50 border border-purple-100 flex items-center justify-center text-purple-600 shrink-0">
            <Scale className="w-4 h-4 sm:w-5 sm:h-5" />
          </div>
        </div>
      </div>

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
                tree.map((rootNode) => renderNode(rootNode, 0))
              )}
            </tbody>
          </table>
        </div>
      </div>
    </div>
  );
};
