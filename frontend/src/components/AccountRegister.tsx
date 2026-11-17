import React from 'react';
import {
  BookOpen,
  PlusCircle,
  CheckCircle2,
  Search,
  Eye,
  EyeOff,
} from 'lucide-react';
import { Account, CreateTransactionPayload, RegisterEntry } from '../types';
import { formatCurrency } from '../api';
import { useAccountRegister } from './register/useAccountRegister';
import { RegisterRow } from './register/RegisterRow';
import { RegisterInlineEntry } from './register/RegisterInlineEntry';

interface AccountRegisterProps {
  account: Account;
  registerEntries: RegisterEntry[];
  allAccounts: Account[];
  currentCurrency: string;
  onSelectAccount: (acc: Account) => void;
  onOpenNewTx: (defaultAccountId?: string) => void;
  onOpenReconcile: (acc: Account) => void;
  onDeleteTransaction: (txId: string) => void;
  onEditTransaction?: (txId: string) => void;
  onToggleHideAccount?: (acc: Account) => void;
  onCreateTransaction?: (payload: CreateTransactionPayload) => Promise<void>;
}

export const AccountRegister: React.FC<AccountRegisterProps> = ({
  account,
  registerEntries,
  allAccounts,
  currentCurrency,
  onSelectAccount,
  onOpenNewTx,
  onOpenReconcile,
  onDeleteTransaction,
  onEditTransaction,
  onToggleHideAccount,
  onCreateTransaction,
}) => {
  const registerHook = useAccountRegister({
    account,
    registerEntries,
    allAccounts,
    onCreateTransaction,
  });

  const {
    expandedTxId,
    toggleExpand,
    searchTerm,
    setSearchTerm,
    filteredEntries,
    debitHeader,
    creditHeader,
  } = registerHook;

  const currencyToUse = currentCurrency || account.currency;

  return (
    <div className="space-y-4">
      {/* Account Header Banner */}
      <div className="bg-white rounded-xl border border-slate-200 p-4 sm:p-5 shadow-sm">
        <div className="flex flex-col md:flex-row md:items-center justify-between gap-4">
          <div className="flex items-center space-x-3">
            <div className="w-10 h-10 sm:w-11 sm:h-11 rounded-lg bg-emerald-50 border border-emerald-200 flex items-center justify-center text-emerald-600 shrink-0">
              <BookOpen className="w-5 h-5 sm:w-6 sm:h-6" />
            </div>
            <div className="min-w-0">
              <div className="flex items-center space-x-2 flex-wrap gap-y-1">
                {/* Account Switcher dropdown */}
                <select
                  value={account.id}
                  onChange={(e) => {
                    const selected = allAccounts.find((a) => a.id === e.target.value);
                    if (selected) onSelectAccount(selected);
                  }}
                  className="font-bold text-base sm:text-lg text-slate-900 bg-transparent hover:bg-slate-100 border border-transparent hover:border-slate-300 rounded px-1.5 py-0.5 cursor-pointer focus:outline-none max-w-full"
                >
                  {allAccounts
                    .filter((a) => !a.placeholder)
                    .map((a) => (
                      <option key={a.id} value={a.id}>
                        {a.code ? `${a.code} - ` : ''}
                        {a.name} ({a.account_type}){a.hidden ? ' [Hidden]' : ''}
                      </option>
                    ))}
                </select>
                <span className="text-[11px] sm:text-xs px-2 py-0.5 rounded-md capitalize font-medium bg-slate-100 text-slate-600 border border-slate-200">
                  {account.account_type}
                </span>
                {account.hidden && (
                  <span className="inline-flex items-center space-x-1 text-[11px] sm:text-xs px-2 py-0.5 rounded-md font-medium bg-amber-100 text-amber-800 border border-amber-300">
                    <EyeOff className="w-3 h-3 text-amber-700" />
                    <span>Hidden Account</span>
                  </span>
                )}
              </div>
              <p className="text-[11px] sm:text-xs text-slate-400 mt-0.5 truncate">
                {account.description || `Ledger register for ${account.name}`}
              </p>
            </div>
          </div>

          {/* Current Running Balance Display & Actions */}
          <div className="flex items-center justify-between md:justify-end space-x-3 sm:space-x-4 pt-2 md:pt-0 border-t md:border-t-0 border-slate-100">
            <div className="text-left md:text-right">
              <span className="text-[10px] sm:text-[11px] font-semibold text-slate-400 uppercase tracking-wider block">
                Ending Balance
              </span>
              <span
                className={`text-xl sm:text-2xl font-bold font-mono ${
                  parseFloat(account.display_balance) < 0 ? 'text-rose-600' : 'text-slate-900'
                }`}
              >
                {formatCurrency(account.display_balance, currencyToUse)}
              </span>
            </div>

            <div className="flex items-center space-x-1.5 sm:space-x-2">
              {onToggleHideAccount && (
                <button
                  onClick={() => onToggleHideAccount(account)}
                  className={`flex items-center space-x-1.5 px-2.5 sm:px-3 py-1.5 sm:py-2 text-xs font-semibold rounded-lg border transition ${
                    account.hidden
                      ? 'text-amber-800 bg-amber-100 hover:bg-amber-200 border-amber-300'
                      : 'text-slate-600 bg-slate-50 hover:bg-slate-100 border-slate-200'
                  }`}
                  title={account.hidden ? 'Unhide Account (Make visible in tree)' : 'Hide Account (Omit from tree)'}
                >
                  {account.hidden ? <EyeOff className="w-4 h-4 text-amber-700" /> : <Eye className="w-4 h-4 text-slate-500" />}
                  <span className="hidden sm:inline">{account.hidden ? 'Unhide' : 'Hide'}</span>
                </button>
              )}

              {['bank', 'credit', 'cash'].includes(account.account_type.toLowerCase()) && (
                <button
                  onClick={() => onOpenReconcile(account)}
                  className="flex items-center space-x-1.5 px-2.5 sm:px-3 py-1.5 sm:py-2 text-xs font-semibold text-emerald-700 bg-emerald-50 hover:bg-emerald-100 border border-emerald-200 rounded-lg transition"
                >
                  <CheckCircle2 className="w-4 h-4" />
                  <span className="hidden xs:inline">Reconcile</span>
                </button>
              )}

              <button
                onClick={() => onOpenNewTx(account.id)}
                className="flex items-center space-x-1.5 px-3 sm:px-3.5 py-1.5 sm:py-2 text-xs font-semibold text-white bg-emerald-600 hover:bg-emerald-500 rounded-lg transition shadow-sm shrink-0"
              >
                <PlusCircle className="w-4 h-4" />
                <span>Add Entry</span>
              </button>
            </div>
          </div>
        </div>
      </div>

      {/* Register Table Container */}
      <div className="bg-white rounded-xl border border-slate-200 shadow-sm overflow-hidden">
        {/* Table Filter Bar */}
        <div className="p-3 sm:p-3.5 border-b border-slate-200 bg-slate-50/70 flex flex-col sm:flex-row items-stretch sm:items-center justify-between gap-2.5 sm:gap-3">
          <div className="flex items-center space-x-2">
            <span className="text-xs font-semibold text-slate-600 uppercase tracking-wider">
              Ledger Register
            </span>
            <span className="text-xs text-slate-500 font-mono">
              ({filteredEntries.length} {filteredEntries.length === 1 ? 'transaction' : 'transactions'})
            </span>
          </div>

          <div className="relative w-full sm:w-72">
            <Search className="w-3.5 h-3.5 text-slate-400 absolute left-3 top-1/2 -translate-y-1/2" />
            <input
              type="text"
              placeholder="Search in register..."
              value={searchTerm}
              onChange={(e) => setSearchTerm(e.target.value)}
              className="w-full pl-8 pr-3 py-1.5 sm:py-1 text-xs bg-white border border-slate-300 rounded-md focus:outline-none focus:ring-1 focus:ring-emerald-500"
            />
          </div>
        </div>

        {/* Mobile View: Clean Card Stream (Visible on < md) */}
        <div className="md:hidden divide-y divide-slate-100">
          {filteredEntries.length === 0 ? (
            <div className="py-12 text-center text-xs text-slate-400">
              No transactions recorded for this account. Tap "Add Entry" to create one.
            </div>
          ) : (
            filteredEntries.map((entry) => (
              <RegisterRow
                key={entry.split_id}
                entry={entry}
                account={account}
                currentCurrency={currentCurrency}
                isExpanded={expandedTxId === entry.transaction_id}
                onToggleExpand={toggleExpand}
                onEditTransaction={onEditTransaction}
                onDeleteTransaction={onDeleteTransaction}
                mode="mobile"
              />
            ))
          )}

          <RegisterInlineEntry
            account={account}
            currentCurrency={currentCurrency}
            registerHook={registerHook}
            mode="mobile"
          />
        </div>

        {/* Desktop View: Full Classic GnuCash Table (Visible on >= md) */}
        <div className="hidden md:block overflow-x-auto">
          <table className="w-full text-left border-collapse">
            <thead>
              <tr className="bg-slate-100 text-slate-700 text-xs font-bold border-b border-slate-300 select-none">
                <th className="py-2.5 px-3 w-28">Date</th>
                <th className="py-2.5 px-2 w-20">Num</th>
                <th className="py-2.5 px-3">Description / Payee</th>
                <th className="py-2.5 px-3 w-48">Transfer Account</th>
                <th className="py-2.5 px-2 w-10 text-center" title="Reconciliation Status">R</th>
                <th className="py-2.5 px-3 w-32 text-right">{debitHeader}</th>
                <th className="py-2.5 px-3 w-32 text-right">{creditHeader}</th>
                <th className="py-2.5 px-3 w-36 text-right font-bold bg-slate-200/50">Balance</th>
                <th className="py-2.5 px-2 w-14 text-center">Actions</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-slate-200 font-sans text-xs">
              {filteredEntries.length === 0 ? (
                <tr>
                  <td colSpan={9} className="py-12 text-center text-slate-400">
                    No transactions recorded for this account. Click "Add Entry" to create one.
                  </td>
                </tr>
              ) : (
                filteredEntries.map((entry) => (
                  <RegisterRow
                    key={entry.split_id}
                    entry={entry}
                    account={account}
                    currentCurrency={currentCurrency}
                    isExpanded={expandedTxId === entry.transaction_id}
                    onToggleExpand={toggleExpand}
                    onEditTransaction={onEditTransaction}
                    onDeleteTransaction={onDeleteTransaction}
                    mode="desktop"
                  />
                ))
              )}

              <RegisterInlineEntry
                account={account}
                currentCurrency={currentCurrency}
                registerHook={registerHook}
                mode="desktop"
              />
            </tbody>
          </table>
        </div>
      </div>
    </div>
  );
};
