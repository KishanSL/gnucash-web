import React from 'react';
import { ChevronRight, ChevronDown, BookOpen, CheckCircle2, Trash2, Plus, Eye, EyeOff, Edit3 } from 'lucide-react';
import { Account, AccountTreeNode } from '../../types';
import { formatCurrency } from '../../api';
import { getAccountTypeIcon, getBadgeStyle } from './accountStyles';

function hasNonHiddenDescendant(childrenList: AccountTreeNode[]): boolean {
  return childrenList.some(
    (c) => !c.account.hidden || hasNonHiddenDescendant(c.children)
  );
}

export interface AccountTreeRowProps {
  node: AccountTreeNode;
  depth: number;
  collapsed: Record<string, boolean>;
  showHidden: boolean;
  searchTerm: string;
  currentCurrency: string;
  toggleCollapse: (id: string) => void;
  onSelectAccountForRegister: (account: Account) => void;
  onSelectAccountForReconcile: (account: Account) => void;
  onDeleteAccount: (id: string) => void;
  onOpenNewAccount: (parentId?: string) => void;
  onEditAccount: (account: Account) => void;
  onToggleHideAccount: (account: Account) => void;
}

/** One account row plus, unless collapsed, its children (recursively). Applies the hidden-account and search filters. */
export const AccountTreeRow: React.FC<AccountTreeRowProps> = (props) => {
  const {
    node,
    depth,
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
  } = props;
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
        onDoubleClick={() => {
          if (!account.placeholder) {
            onSelectAccountForRegister(account);
          } else {
            toggleCollapse(account.id);
          }
        }}
        title={
          account.placeholder
            ? 'Double-click to expand / collapse folder'
            : 'Double-click to open ledger register'
        }
        className={`border-b border-slate-100 transition-colors select-none ${
          isHiddenAccount
            ? 'bg-amber-50/50 hover:bg-amber-100/40 text-slate-600 italic cursor-pointer'
            : account.placeholder
            ? 'bg-slate-50/80 font-semibold text-slate-800 cursor-pointer'
            : 'hover:bg-emerald-50/40 text-slate-700 cursor-pointer'
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
        children.map((child) => (
          <AccountTreeRow key={child.account.id} {...props} node={child} depth={depth + 1} />
        ))}
    </React.Fragment>
  );
};
