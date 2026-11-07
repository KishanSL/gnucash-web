import React, { useState } from 'react';
import {
  ListFilter,
  Search,
  Calendar,
  ChevronDown,
  ChevronUp,
  Trash2,
  Pencil,
  PlusCircle,
  Split,
  CornerDownRight,
} from 'lucide-react';
import { Transaction } from '../types';
import { formatCurrency } from '../api';

interface TransactionsJournalProps {
  transactions: Transaction[];
  currentCurrency: string;
  onOpenNewTx: () => void;
  onDeleteTransaction: (id: string) => void;
  onEditTransaction?: (id: string) => void;
}

export const TransactionsJournal: React.FC<TransactionsJournalProps> = ({
  transactions,
  currentCurrency,
  onOpenNewTx,
  onDeleteTransaction,
  onEditTransaction,
}) => {
  const [searchTerm, setSearchTerm] = useState('');
  const [expandedId, setExpandedId] = useState<string | null>(null);

  const toggleExpand = (id: string) => {
    setExpandedId(expandedId === id ? null : id);
  };

  const filtered = transactions.filter((tx) => {
    if (!searchTerm) return true;
    const term = searchTerm.toLowerCase();
    return (
      tx.description.toLowerCase().includes(term) ||
      (tx.num && tx.num.toLowerCase().includes(term)) ||
      (tx.notes && tx.notes.toLowerCase().includes(term)) ||
      tx.splits.some((s) => s.account_name.toLowerCase().includes(term))
    );
  });

  return (
    <div className="space-y-4">
      {/* Journal Header Bar */}
      <div className="bg-white rounded-xl border border-slate-200 p-4 sm:p-5 shadow-sm">
        <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-3 sm:gap-4">
          <div className="flex items-center space-x-3">
            <div className="w-10 h-10 rounded-lg bg-emerald-50 border border-emerald-200 flex items-center justify-center text-emerald-600 shrink-0">
              <ListFilter className="w-5 h-5" />
            </div>
            <div>
              <h2 className="text-base sm:text-lg font-bold text-slate-900">General Journal</h2>
              <p className="text-[11px] sm:text-xs text-slate-500">
                Complete chronological log of all double-entry financial transactions
              </p>
            </div>
          </div>

          <div className="flex items-center space-x-2 w-full sm:w-auto">
            <div className="relative flex-1 sm:w-64">
              <Search className="w-3.5 h-3.5 text-slate-400 absolute left-3 top-1/2 -translate-y-1/2" />
              <input
                type="text"
                placeholder="Search transactions..."
                value={searchTerm}
                onChange={(e) => setSearchTerm(e.target.value)}
                className="w-full pl-8 pr-3 py-1.5 text-xs bg-slate-50 border border-slate-300 rounded-lg focus:outline-none focus:ring-1 focus:ring-emerald-500"
              />
            </div>

            <button
              onClick={onOpenNewTx}
              className="flex items-center space-x-1 sm:space-x-1.5 px-3 py-1.5 text-xs font-semibold bg-emerald-600 hover:bg-emerald-500 text-white rounded-lg transition shadow-sm shrink-0"
            >
              <PlusCircle className="w-4 h-4" />
              <span>New Entry</span>
            </button>
          </div>
        </div>
      </div>

      {/* Journal Container */}
      <div className="bg-white rounded-xl border border-slate-200 shadow-sm overflow-hidden">
        {/* Mobile View: Cards Stream (Visible on < md) */}
        <div className="md:hidden divide-y divide-slate-100">
          {filtered.length === 0 ? (
            <div className="py-12 text-center text-xs text-slate-400">
              No transactions found. Tap "New Entry" to record one.
            </div>
          ) : (
            filtered.map((tx) => {
              const isExpanded = expandedId === tx.id;
              const totalPositive = tx.splits
                .map((s) => parseFloat(s.amount))
                .filter((a) => a > 0)
                .reduce((a, b) => a + b, 0);

              return (
                <div key={tx.id} className="p-3.5 space-y-2 hover:bg-slate-50 transition">
                  <div className="flex items-center justify-between text-xs">
                    <div className="flex items-center space-x-1.5 font-mono text-slate-500 font-semibold">
                      <Calendar className="w-3.5 h-3.5 text-slate-400" />
                      <span>{tx.post_date}</span>
                      {tx.num && (
                        <span className="text-[10px] bg-slate-100 text-slate-600 px-1.5 py-0.2 rounded">
                          #{tx.num}
                        </span>
                      )}
                    </div>
                    <span className="font-mono font-bold text-slate-900 text-sm">
                      {formatCurrency(totalPositive, currentCurrency)}
                    </span>
                  </div>

                  <div>
                    <h4 className="font-bold text-slate-900 text-xs sm:text-sm">{tx.description}</h4>
                    {tx.notes && <p className="text-[11px] text-slate-400 mt-0.5">{tx.notes}</p>}
                  </div>

                  {/* Accounts Involved */}
                  <div className="flex flex-wrap gap-1 pt-1">
                    {tx.splits.map((s) => (
                      <span
                        key={s.id}
                        className="px-1.5 py-0.5 rounded bg-slate-100 text-slate-600 text-[10px] border border-slate-200"
                      >
                        {s.account_name}
                      </span>
                    ))}
                  </div>

                  {/* Mobile Actions: View splits or edit/delete */}
                  <div className="flex items-center justify-between pt-1 border-t border-slate-100 text-xs">
                    <button
                      onClick={() => toggleExpand(tx.id)}
                      className="inline-flex items-center space-x-1 text-emerald-700 font-medium text-[11px]"
                    >
                      <Split className="w-3 h-3" />
                      <span>{isExpanded ? 'Hide Splits' : `View Splits (${tx.splits.length})`}</span>
                    </button>

                    <div className="flex items-center space-x-1">
                      {onEditTransaction && (
                        <button
                          onClick={() => onEditTransaction(tx.id)}
                          className="p-1 text-slate-400 hover:text-emerald-700 hover:bg-emerald-50 rounded transition"
                          title="Edit Transaction"
                        >
                          <Pencil className="w-3.5 h-3.5" />
                        </button>
                      )}
                      <button
                        onClick={() => {
                          if (confirm('Delete this transaction and its splits?')) {
                            onDeleteTransaction(tx.id);
                          }
                        }}
                        className="p-1 text-slate-400 hover:text-rose-600 hover:bg-rose-50 rounded transition"
                        title="Delete"
                      >
                        <Trash2 className="w-3.5 h-3.5" />
                      </button>
                    </div>
                  </div>

                  {/* Expanded Splits in Mobile Card */}
                  {isExpanded && (
                    <div className="mt-2 bg-slate-50 rounded-lg p-2.5 border border-slate-200 text-xs space-y-1.5">
                      <div className="font-semibold text-slate-700 text-[11px]">Double-Entry Splits:</div>
                      {tx.splits.map((s) => {
                        const val = parseFloat(s.amount);
                        return (
                          <div key={s.id} className="flex justify-between items-center text-[11px] py-1 border-b border-slate-100 last:border-none">
                            <div>
                              <span className="font-medium text-slate-800">{s.account_name}</span>
                              {s.memo && <span className="text-slate-400 ml-1 italic">({s.memo})</span>}
                            </div>
                            <span className={`font-mono font-bold ${val > 0 ? 'text-emerald-700' : 'text-rose-700'}`}>
                              {val > 0 ? `+${formatCurrency(val, currentCurrency)}` : `-${formatCurrency(Math.abs(val), currentCurrency)}`}
                            </span>
                          </div>
                        );
                      })}
                    </div>
                  )}
                </div>
              );
            })
          )}
        </div>

        {/* Desktop View: Wide Table (Visible on >= md) */}
        <div className="hidden md:block overflow-x-auto">
          <table className="w-full text-left border-collapse">
            <thead>
              <tr className="bg-slate-100 text-slate-700 text-xs font-bold border-b border-slate-200">
                <th className="py-3 px-4 w-32">Date</th>
                <th className="py-3 px-3 w-24">Num</th>
                <th className="py-3 px-4">Description</th>
                <th className="py-3 px-4 w-64">Accounts Involved</th>
                <th className="py-3 px-4 w-32 text-right">Total Amount</th>
                <th className="py-3 px-3 w-16 text-center">Actions</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-slate-100 text-xs">
              {filtered.length === 0 ? (
                <tr>
                  <td colSpan={6} className="py-12 text-center text-slate-400">
                    No transactions found.
                  </td>
                </tr>
              ) : (
                filtered.map((tx) => {
                  const isExpanded = expandedId === tx.id;
                  const totalPositive = tx.splits
                    .map((s) => parseFloat(s.amount))
                    .filter((a) => a > 0)
                    .reduce((a, b) => a + b, 0);

                  return (
                    <React.Fragment key={tx.id}>
                      <tr
                        className={`hover:bg-slate-50 transition-colors ${
                          isExpanded ? 'bg-emerald-50/20' : ''
                        }`}
                      >
                        <td className="py-2.5 px-4 font-mono text-slate-600 flex items-center space-x-1.5">
                          <Calendar className="w-3.5 h-3.5 text-slate-400" />
                          <span>{tx.post_date}</span>
                        </td>

                        <td className="py-2.5 px-3 font-mono text-slate-500 text-[11px]">
                          {tx.num || '—'}
                        </td>

                        <td className="py-2.5 px-4 font-medium text-slate-900">
                          <div className="flex items-center space-x-2">
                            <button
                              onClick={() => toggleExpand(tx.id)}
                              className="p-0.5 text-slate-400 hover:text-emerald-600 rounded"
                            >
                              {isExpanded ? (
                                <ChevronUp className="w-3.5 h-3.5" />
                              ) : (
                                <ChevronDown className="w-3.5 h-3.5" />
                              )}
                            </button>
                            <span>{tx.description}</span>
                          </div>
                          {tx.notes && (
                            <p className="text-[11px] text-slate-400 pl-5">{tx.notes}</p>
                          )}
                        </td>

                        <td className="py-2.5 px-4 text-slate-600">
                          <div className="flex flex-wrap gap-1">
                            {tx.splits.slice(0, 3).map((s) => (
                              <span
                                key={s.id}
                                className="px-1.5 py-0.5 rounded bg-slate-100 text-slate-700 text-[11px] border border-slate-200"
                              >
                                {s.account_name}
                              </span>
                            ))}
                            {tx.splits.length > 3 && (
                              <span className="text-[10px] text-slate-400 flex items-center">
                                +{tx.splits.length - 3} more
                              </span>
                            )}
                          </div>
                        </td>

                        <td className="py-2.5 px-4 text-right font-mono font-bold text-slate-900">
                          {formatCurrency(totalPositive, currentCurrency)}
                        </td>

                        <td className="py-2.5 px-3 text-center">
                          <div className="flex items-center justify-center space-x-1">
                            {onEditTransaction && (
                              <button
                                onClick={() => onEditTransaction(tx.id)}
                                className="p-1 text-slate-400 hover:text-emerald-700 hover:bg-emerald-50 rounded transition"
                                title="Edit Transaction"
                              >
                                <Pencil className="w-3.5 h-3.5" />
                              </button>
                            )}
                            <button
                              onClick={() => {
                                if (confirm('Delete this transaction?')) {
                                  onDeleteTransaction(tx.id);
                                }
                              }}
                              className="p-1 text-slate-300 hover:text-rose-600 hover:bg-rose-50 rounded transition"
                              title="Delete"
                            >
                              <Trash2 className="w-3.5 h-3.5" />
                            </button>
                          </div>
                        </td>
                      </tr>

                      {/* Splits Details */}
                      {isExpanded && (
                        <tr className="bg-slate-50/70">
                          <td colSpan={6} className="py-3 px-8">
                            <div className="bg-white rounded-lg border border-slate-200 p-3 shadow-inner">
                              <div className="flex items-center space-x-1.5 text-xs font-bold text-slate-700 mb-2">
                                <CornerDownRight className="w-3.5 h-3.5 text-emerald-600" />
                                <span>Double-Entry Splits ({tx.splits.length}):</span>
                              </div>

                              <table className="w-full text-xs">
                                <thead>
                                  <tr className="text-slate-400 border-b border-slate-100 font-semibold">
                                    <th className="py-1 text-left">Account</th>
                                    <th className="py-1 text-left">Memo</th>
                                    <th className="py-1 text-center w-12">Rec</th>
                                    <th className="py-1 text-right w-28">Debit</th>
                                    <th className="py-1 text-right w-28">Credit</th>
                                  </tr>
                                </thead>
                                <tbody>
                                  {tx.splits.map((s) => {
                                    const val = parseFloat(s.amount);
                                    const debit = val > 0 ? val : null;
                                    const credit = val < 0 ? -val : null;

                                    return (
                                      <tr key={s.id} className="text-slate-700">
                                        <td className="py-1 font-medium">{s.account_name}</td>
                                        <td className="py-1 text-slate-400 italic">
                                          {s.memo || '—'}
                                        </td>
                                        <td className="py-1 text-center font-mono font-bold text-slate-500">
                                          {s.reconciled_state}
                                        </td>
                                        <td className="py-1 text-right font-mono text-emerald-700">
                                          {debit !== null ? formatCurrency(debit, currentCurrency) : ''}
                                        </td>
                                        <td className="py-1 text-right font-mono text-rose-700">
                                          {credit !== null ? formatCurrency(credit, currentCurrency) : ''}
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
