import React, { useState } from 'react';
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
}

export const AccountsTree: React.FC<AccountsTreeProps> = ({
  tree,
  currentCurrency,
  onSelectAccountForRegister,
  onSelectAccountForReconcile,
  onDeleteAccount,
  onOpenNewAccount,
}) => {
  const [searchTerm, setSearchTerm] = useState('');
  const [collapsed, setCollapsed] = useState<Record<string, boolean>>({});

  const toggleCollapse = (id: string) => {
    setCollapsed((prev) => ({ ...prev, [id]: !prev[id] }));
  };

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

  const renderNode = (node: AccountTreeNode, depth = 0): React.ReactNode => {
    const { account, children } = node;
    const isCollapsed = collapsed[account.id] || false;
    const hasChildren = children && children.length > 0;

    // Search filter
    const matchesSearch =
      !searchTerm ||
      account.name.toLowerCase().includes(searchTerm.toLowerCase()) ||
      (account.code && account.code.toLowerCase().includes(searchTerm.toLowerCase())) ||
      account.account_type.toLowerCase().includes(searchTerm.toLowerCase());

    const childrenHaveMatches = (childrenList: AccountTreeNode[]): boolean => {
      return childrenList.some(
        (c) =>
          c.account.name.toLowerCase().includes(searchTerm.toLowerCase()) ||
          childrenHaveMatches(c.children)
      );
    };

    if (searchTerm && !matchesSearch && !childrenHaveMatches(children)) {
      return null;
    }

    return (
      <React.Fragment key={account.id}>
        <tr
          className={`border-b border-slate-100 transition-colors ${
            account.placeholder
              ? 'bg-slate-50/80 font-semibold text-slate-800'
              : 'hover:bg-emerald-50/40 text-slate-700'
          }`}
        >
          {/* Account Name with tree indentation */}
          <td className="py-2.5 px-4">
            <div className="flex items-center" style={{ paddingLeft: `${depth * 24}px` }}>
              {hasChildren ? (
                <button
                  onClick={() => toggleCollapse(account.id)}
                  className="p-1 mr-1 text-slate-400 hover:text-slate-700 rounded hover:bg-slate-200/60 transition"
                  title={isCollapsed ? 'Expand' : 'Collapse'}
                >
                  {isCollapsed ? (
                    <ChevronRight className="w-4 h-4" />
                  ) : (
                    <ChevronDown className="w-4 h-4" />
                  )}
                </button>
              ) : (
                <span className="w-6 inline-block" />
              )}

              <span className="mr-2.5">{getAccountTypeIcon(account.account_type, account.placeholder)}</span>

              <div className="flex flex-col">
                <div className="flex items-center space-x-2">
                  <span className={account.placeholder ? 'font-bold' : 'font-medium'}>
                    {account.name}
                  </span>
                  {account.placeholder && (
                    <span className="text-[10px] uppercase font-bold tracking-wider px-1.5 py-0.2 rounded bg-slate-200 text-slate-600">
                      Placeholder
                    </span>
                  )}
                </div>
                {account.description && (
                  <span className="text-[11px] text-slate-400 font-normal">
                    {account.description}
                  </span>
                )}
              </div>
            </div>
          </td>

          {/* Account Code */}
          <td className="py-2.5 px-3 text-slate-500 font-mono text-xs">
            {account.code || '—'}
          </td>

          {/* Account Type */}
          <td className="py-2.5 px-3">
            <span
              className={`inline-block px-2 py-0.5 rounded-md text-xs font-medium border capitalize ${getBadgeStyle(
                account.account_type
              )}`}
            >
              {account.account_type}
            </span>
          </td>

          {/* Direct Balance */}
          <td className="py-2.5 px-4 text-right font-mono text-xs">
            {!account.placeholder ? (
              <span
                className={
                  parseFloat(account.display_balance) < 0
                    ? 'text-rose-600 font-semibold'
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
          <td className="py-2.5 px-4 text-right font-mono text-xs font-semibold">
            <span
              className={
                parseFloat(account.display_balance) < 0 ? 'text-rose-600' : 'text-slate-900'
              }
            >
              {formatCurrency(account.display_balance, currentCurrency || account.currency)}
            </span>
          </td>

          {/* Actions */}
          <td className="py-2.5 px-4 text-right">
            <div className="flex items-center justify-end space-x-1">
              {!account.placeholder && (
                <>
                  <button
                    onClick={() => onSelectAccountForRegister(account)}
                    className="flex items-center space-x-1 px-2.5 py-1 text-xs font-medium text-emerald-700 bg-emerald-50 hover:bg-emerald-100 rounded border border-emerald-200 transition"
                    title="Open Ledger Register"
                  >
                    <BookOpen className="w-3.5 h-3.5" />
                    <span>Register</span>
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
                  <span>Subaccount</span>
                </button>
              )}

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
    <div className="space-y-6">
      {/* Top Summary Cards */}
      <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-4">
        {/* Net Worth */}
        <div className="bg-white p-4 rounded-xl border border-slate-200 shadow-sm flex items-center justify-between">
          <div>
            <p className="text-xs font-medium text-slate-500 uppercase tracking-wider">Net Worth</p>
            <h3 className="text-xl font-bold text-slate-900 mt-1">
              {formatCurrency(netWorth, currentCurrency)}
            </h3>
            <p className="text-[11px] text-slate-400 mt-0.5">Assets minus Liabilities</p>
          </div>
          <div className="w-10 h-10 rounded-lg bg-emerald-50 border border-emerald-100 flex items-center justify-center text-emerald-600">
            <Scale className="w-5 h-5" />
          </div>
        </div>

        {/* Total Assets */}
        <div className="bg-white p-4 rounded-xl border border-slate-200 shadow-sm flex items-center justify-between">
          <div>
            <p className="text-xs font-medium text-slate-500 uppercase tracking-wider">Total Assets</p>
            <h3 className="text-xl font-bold text-sky-700 mt-1">
              {formatCurrency(assetsTotal, currentCurrency)}
            </h3>
            <p className="text-[11px] text-slate-400 mt-0.5">Cash, Bank, Property</p>
          </div>
          <div className="w-10 h-10 rounded-lg bg-sky-50 border border-sky-100 flex items-center justify-center text-sky-600">
            <Landmark className="w-5 h-5" />
          </div>
        </div>

        {/* Total Liabilities */}
        <div className="bg-white p-4 rounded-xl border border-slate-200 shadow-sm flex items-center justify-between">
          <div>
            <p className="text-xs font-medium text-slate-500 uppercase tracking-wider">Liabilities</p>
            <h3 className="text-xl font-bold text-amber-700 mt-1">
              {formatCurrency(liabTotal, currentCurrency)}
            </h3>
            <p className="text-[11px] text-slate-400 mt-0.5">Credit Cards, Payables</p>
          </div>
          <div className="w-10 h-10 rounded-lg bg-amber-50 border border-amber-100 flex items-center justify-center text-amber-600">
            <CreditCard className="w-5 h-5" />
          </div>
        </div>

        {/* Total Equity */}
        <div className="bg-white p-4 rounded-xl border border-slate-200 shadow-sm flex items-center justify-between">
          <div>
            <p className="text-xs font-medium text-slate-500 uppercase tracking-wider">Equity</p>
            <h3 className="text-xl font-bold text-purple-700 mt-1">
              {formatCurrency(equityTotal, currentCurrency)}
            </h3>
            <p className="text-[11px] text-slate-400 mt-0.5">Opening Balances & Earnings</p>
          </div>
          <div className="w-10 h-10 rounded-lg bg-purple-50 border border-purple-100 flex items-center justify-center text-purple-600">
            <Scale className="w-5 h-5" />
          </div>
        </div>
      </div>

      {/* Main Table Container */}
      <div className="bg-white rounded-xl border border-slate-200 shadow-sm overflow-hidden">
        {/* Table Toolbar */}
        <div className="p-4 border-b border-slate-200 bg-slate-50/50 flex flex-col sm:flex-row items-center justify-between gap-3">
          <div className="flex items-center space-x-2">
            <FolderTree className="w-5 h-5 text-emerald-600" />
            <h2 className="font-bold text-slate-800 text-base">Chart of Accounts</h2>
            <span className="text-xs text-slate-500 bg-slate-200 px-2 py-0.5 rounded-full font-medium">
              Tree Hierarchy
            </span>
          </div>

          <div className="flex items-center space-x-2 w-full sm:w-auto">
            {/* Search Input */}
            <div className="relative w-full sm:w-64">
              <Search className="w-4 h-4 text-slate-400 absolute left-3 top-1/2 -translate-y-1/2" />
              <input
                type="text"
                placeholder="Filter accounts..."
                value={searchTerm}
                onChange={(e) => setSearchTerm(e.target.value)}
                className="w-full pl-9 pr-3 py-1.5 text-xs bg-white border border-slate-300 rounded-lg focus:outline-none focus:ring-2 focus:ring-emerald-500/20 focus:border-emerald-500"
              />
            </div>

            <button
              onClick={() => onOpenNewAccount()}
              className="flex items-center space-x-1 px-3 py-1.5 text-xs font-semibold bg-emerald-600 hover:bg-emerald-500 text-white rounded-lg transition shrink-0"
            >
              <Plus className="w-4 h-4" />
              <span>Add Account</span>
            </button>
          </div>
        </div>

        {/* Table */}
        <div className="overflow-x-auto">
          <table className="w-full text-left border-collapse">
            <thead>
              <tr className="bg-slate-100/80 border-b border-slate-200 text-slate-600 text-xs font-semibold uppercase tracking-wider">
                <th className="py-3 px-4">Account Name</th>
                <th className="py-3 px-3 w-24">Code</th>
                <th className="py-3 px-3 w-28">Type</th>
                <th className="py-3 px-4 w-36 text-right">Direct Balance</th>
                <th className="py-3 px-4 w-36 text-right">Total Balance</th>
                <th className="py-3 px-4 w-40 text-right">Actions</th>
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
