import React, { useState } from 'react';
import {
  BookOpen,
  PlusCircle,
  CheckCircle2,
  ChevronDown,
  ChevronUp,
  Trash2,
  CornerDownRight,
  Split,
  Search,
} from 'lucide-react';
import { Account, RegisterEntry } from '../types';
import { formatCurrency } from '../api';

interface AccountRegisterProps {
  account: Account;
  registerEntries: RegisterEntry[];
  allAccounts: Account[];
  currentCurrency: string;
  onSelectAccount: (acc: Account) => void;
  onOpenNewTx: (defaultAccountId?: string) => void;
  onOpenReconcile: (acc: Account) => void;
  onDeleteTransaction: (txId: string) => void;
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
}) => {
  const [expandedTxId, setExpandedTxId] = useState<string | null>(null);
  const [searchTerm, setSearchTerm] = useState('');

  const toggleExpand = (txId: string) => {
    setExpandedTxId(expandedTxId === txId ? null : txId);
  };

  const filteredEntries = registerEntries.filter((entry) => {
    if (!searchTerm) return true;
    const term = searchTerm.toLowerCase();
    return (
      entry.description.toLowerCase().includes(term) ||
      (entry.num && entry.num.toLowerCase().includes(term)) ||
      (entry.transfer_account_name && entry.transfer_account_name.toLowerCase().includes(term)) ||
      (entry.memo && entry.memo.toLowerCase().includes(term))
    );
  });

  const getReconciledBadge = (state: string) => {
    switch (state) {
      case 'y':
        return (
          <span
            className="inline-flex items-center px-1.5 py-0.5 rounded text-[10px] font-bold bg-emerald-100 text-emerald-800 border border-emerald-300"
            title="Reconciled (y)"
          >
            y
          </span>
        );
      case 'c':
        return (
          <span
            className="inline-flex items-center px-1.5 py-0.5 rounded text-[10px] font-bold bg-amber-100 text-amber-800 border border-amber-300"
            title="Cleared (c)"
          >
            c
          </span>
        );
      default:
        return (
          <span
            className="inline-flex items-center px-1.5 py-0.5 rounded text-[10px] font-medium text-slate-400"
            title="Not Reconciled (n)"
          >
            n
          </span>
        );
    }
  };

  // Debit/Credit column header labels based on account type
  const isAsset = ['asset', 'bank', 'cash', 'receivable'].includes(account.account_type.toLowerCase());
  const debitHeader = isAsset ? 'Deposit / Increase' : 'Payment / Decrease';
  const creditHeader = isAsset ? 'Withdrawal / Decrease' : 'Charge / Increase';

  return (
    <div className="space-y-4">
      {/* Account Header Banner */}
      <div className="bg-white rounded-xl border border-slate-200 p-5 shadow-sm">
        <div className="flex flex-col md:flex-row md:items-center justify-between gap-4">
          <div className="flex items-center space-x-3">
            <div className="w-11 h-11 rounded-lg bg-emerald-50 border border-emerald-200 flex items-center justify-center text-emerald-600">
              <BookOpen className="w-6 h-6" />
            </div>
            <div>
              <div className="flex items-center space-x-2">
                {/* Account Switcher dropdown */}
                <select
                  value={account.id}
                  onChange={(e) => {
                    const selected = allAccounts.find((a) => a.id === e.target.value);
                    if (selected) onSelectAccount(selected);
                  }}
                  className="font-bold text-lg text-slate-900 bg-transparent hover:bg-slate-100 border border-transparent hover:border-slate-300 rounded px-1.5 py-0.5 cursor-pointer focus:outline-none"
                >
                  {allAccounts
                    .filter((a) => !a.placeholder)
                    .map((a) => (
                      <option key={a.id} value={a.id}>
                        {a.code ? `${a.code} - ` : ''}
                        {a.name} ({a.account_type})
                      </option>
                    ))}
                </select>
                <span className="text-xs px-2 py-0.5 rounded-md capitalize font-medium bg-slate-100 text-slate-600 border border-slate-200">
                  {account.account_type}
                </span>
              </div>
              <p className="text-xs text-slate-400 mt-0.5">
                {account.description || `Ledger register for ${account.name}`}
              </p>
            </div>
          </div>

          {/* Current Running Balance Display & Actions */}
          <div className="flex items-center space-x-4">
            <div className="text-right">
              <span className="text-[11px] font-semibold text-slate-400 uppercase tracking-wider block">
                Ending Balance
              </span>
              <span
                className={`text-2xl font-bold font-mono ${
                  parseFloat(account.display_balance) < 0 ? 'text-rose-600' : 'text-slate-900'
                }`}
              >
                {formatCurrency(account.display_balance, currentCurrency || account.currency)}
              </span>
            </div>

            <div className="flex items-center space-x-2">
              {['bank', 'credit', 'cash'].includes(account.account_type.toLowerCase()) && (
                <button
                  onClick={() => onOpenReconcile(account)}
                  className="flex items-center space-x-1.5 px-3 py-2 text-xs font-semibold text-emerald-700 bg-emerald-50 hover:bg-emerald-100 border border-emerald-200 rounded-lg transition"
                >
                  <CheckCircle2 className="w-4 h-4" />
                  <span>Reconcile</span>
                </button>
              )}

              <button
                onClick={() => onOpenNewTx(account.id)}
                className="flex items-center space-x-1.5 px-3.5 py-2 text-xs font-semibold text-white bg-emerald-600 hover:bg-emerald-500 rounded-lg transition shadow-sm"
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
        <div className="p-3.5 border-b border-slate-200 bg-slate-50/70 flex items-center justify-between gap-3">
          <div className="flex items-center space-x-2">
            <span className="text-xs font-semibold text-slate-600 uppercase tracking-wider">
              Ledger Register
            </span>
            <span className="text-xs text-slate-500 font-mono">
              ({filteredEntries.length} {filteredEntries.length === 1 ? 'transaction' : 'transactions'})
            </span>
          </div>

          <div className="relative w-72">
            <Search className="w-3.5 h-3.5 text-slate-400 absolute left-3 top-1/2 -translate-y-1/2" />
            <input
              type="text"
              placeholder="Search in register..."
              value={searchTerm}
              onChange={(e) => setSearchTerm(e.target.value)}
              className="w-full pl-8 pr-3 py-1 text-xs bg-white border border-slate-300 rounded-md focus:outline-none focus:ring-1 focus:ring-emerald-500"
            />
          </div>
        </div>

        {/* Ledger Table (GnuCash Style) */}
        <div className="overflow-x-auto">
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
                filteredEntries.map((entry) => {
                  const isExpanded = expandedTxId === entry.transaction_id;
                  const isMultiSplit = entry.all_splits.length > 2;

                  return (
                    <React.Fragment key={entry.split_id}>
                      <tr
                        className={`hover:bg-amber-50/40 transition-colors ${
                          isExpanded ? 'bg-emerald-50/30' : ''
                        }`}
                      >
                        {/* Date */}
                        <td className="py-2 px-3 font-mono text-slate-600 whitespace-nowrap">
                          {entry.post_date}
                        </td>

                        {/* Num */}
                        <td className="py-2 px-2 font-mono text-slate-500 text-[11px]">
                          {entry.num || ''}
                        </td>

                        {/* Description */}
                        <td className="py-2 px-3 font-medium text-slate-900">
                          <div className="flex items-center space-x-1.5">
                            {isMultiSplit && (
                              <button
                                onClick={() => toggleExpand(entry.transaction_id)}
                                className="p-0.5 text-slate-400 hover:text-emerald-600 rounded"
                                title="Toggle Split breakdown"
                              >
                                {isExpanded ? (
                                  <ChevronUp className="w-3.5 h-3.5" />
                                ) : (
                                  <ChevronDown className="w-3.5 h-3.5" />
                                )}
                              </button>
                            )}
                            <span>{entry.description}</span>
                            {entry.memo && (
                              <span className="text-[11px] text-slate-400 font-normal">
                                ({entry.memo})
                              </span>
                            )}
                          </div>
                        </td>

                        {/* Transfer Account */}
                        <td className="py-2 px-3 text-slate-600 truncate max-w-[200px]">
                          {isMultiSplit ? (
                            <button
                              onClick={() => toggleExpand(entry.transaction_id)}
                              className="inline-flex items-center space-x-1 text-emerald-700 font-semibold hover:underline"
                            >
                              <Split className="w-3 h-3" />
                              <span>-- Split ({entry.all_splits.length}) --</span>
                            </button>
                          ) : (
                            <span className="font-medium">{entry.transfer_account_name}</span>
                          )}
                        </td>

                        {/* Reconciled status */}
                        <td className="py-2 px-2 text-center">
                          {getReconciledBadge(entry.reconciled_state)}
                        </td>

                        {/* Debit */}
                        <td className="py-2 px-3 text-right font-mono text-slate-800">
                          {entry.debit ? formatCurrency(entry.debit, currentCurrency || account.currency) : ''}
                        </td>

                        {/* Credit */}
                        <td className="py-2 px-3 text-right font-mono text-slate-800">
                          {entry.credit ? formatCurrency(entry.credit, currentCurrency || account.currency) : ''}
                        </td>

                        {/* Running Balance */}
                        <td className="py-2 px-3 text-right font-mono font-semibold bg-slate-50/60 text-slate-900">
                          {formatCurrency(entry.running_balance, currentCurrency || account.currency)}
                        </td>

                        {/* Delete action */}
                        <td className="py-2 px-2 text-center">
                          <button
                            onClick={() => {
                              if (confirm('Delete this transaction and all its splits?')) {
                                onDeleteTransaction(entry.transaction_id);
                              }
                            }}
                            className="p-1 text-slate-300 hover:text-rose-600 rounded transition"
                            title="Delete Transaction"
                          >
                            <Trash2 className="w-3.5 h-3.5" />
                          </button>
                        </td>
                      </tr>

                      {/* Expanded Splits Row (Journal view inside register) */}
                      {isExpanded && (
                        <tr className="bg-emerald-50/20 border-b border-emerald-100">
                          <td colSpan={9} className="py-3 px-6">
                            <div className="bg-white rounded-lg border border-slate-200 p-3 shadow-inner">
                              <div className="flex items-center space-x-1.5 text-xs font-bold text-slate-700 mb-2">
                                <CornerDownRight className="w-3.5 h-3.5 text-emerald-600" />
                                <span>Full Double-Entry Split Breakdown:</span>
                              </div>

                              <table className="w-full text-xs">
                                <thead>
                                  <tr className="text-slate-500 border-b border-slate-100 font-semibold">
                                    <th className="py-1 text-left">Account</th>
                                    <th className="py-1 text-left">Type</th>
                                    <th className="py-1 text-left">Memo</th>
                                    <th className="py-1 text-center w-12">Rec</th>
                                    <th className="py-1 text-right w-28">Debit</th>
                                    <th className="py-1 text-right w-28">Credit</th>
                                  </tr>
                                </thead>
                                <tbody>
                                  {entry.all_splits.map((split) => {
                                    const amountNum = parseFloat(split.amount);
                                    const debitVal = amountNum > 0 ? amountNum : null;
                                    const creditVal = amountNum < 0 ? -amountNum : null;

                                    return (
                                      <tr
                                        key={split.id}
                                        className={
                                          split.account_id === account.id
                                            ? 'font-bold bg-slate-50'
                                            : 'text-slate-600'
                                        }
                                      >
                                        <td className="py-1">
                                          {split.account_name}
                                          {split.account_id === account.id && (
                                            <span className="ml-1 text-[10px] text-emerald-600 uppercase font-bold">
                                              (Current)
                                            </span>
                                          )}
                                        </td>
                                        <td className="py-1 capitalize text-slate-400">
                                          {split.account_type}
                                        </td>
                                        <td className="py-1 text-slate-400 italic">
                                          {split.memo || '—'}
                                        </td>
                                        <td className="py-1 text-center">
                                          {getReconciledBadge(split.reconciled_state)}
                                        </td>
                                        <td className="py-1 text-right font-mono">
                                          {debitVal !== null ? formatCurrency(debitVal, currentCurrency) : ''}
                                        </td>
                                        <td className="py-1 text-right font-mono">
                                          {creditVal !== null ? formatCurrency(creditVal, currentCurrency) : ''}
                                        </td>
                                      </tr>
                                    );
                                  })}
                                </tbody>
                              </table>
                            </div>
                          </td>
                        </tr>
                      )}
                    </React.Fragment>
                  );
                })
              )}
            </tbody>
          </table>
        </div>
      </div>
    </div>
  );
};
