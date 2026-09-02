import React, { useState } from 'react';
import {
  ListFilter,
  Search,
  Calendar,
  ChevronDown,
  ChevronUp,
  Trash2,
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
}

export const TransactionsJournal: React.FC<TransactionsJournalProps> = ({
  transactions,
  currentCurrency,
  onOpenNewTx,
  onDeleteTransaction,
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
      <div className="bg-white rounded-xl border border-slate-200 p-5 shadow-sm">
        <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-4">
          <div className="flex items-center space-x-3">
            <div className="w-10 h-10 rounded-lg bg-emerald-50 border border-emerald-200 flex items-center justify-center text-emerald-600">
              <ListFilter className="w-5 h-5" />
            </div>
            <div>
              <h2 className="text-lg font-bold text-slate-900">General Journal</h2>
              <p className="text-xs text-slate-500">
                Complete chronological log of all double-entry financial transactions
              </p>
            </div>
          </div>

          <div className="flex items-center space-x-3">
            <div className="relative w-64">
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
              className="flex items-center space-x-1.5 px-3 py-1.5 text-xs font-semibold bg-emerald-600 hover:bg-emerald-500 text-white rounded-lg transition shadow-sm"
            >
              <PlusCircle className="w-4 h-4" />
              <span>New Transaction</span>
            </button>
          </div>
        </div>
      </div>

      {/* Journal Table */}
      <div className="bg-white rounded-xl border border-slate-200 shadow-sm overflow-hidden">
        <div className="overflow-x-auto">
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
                          <button
                            onClick={() => {
                              if (confirm('Delete this transaction?')) {
                                onDeleteTransaction(tx.id);
                              }
                            }}
                            className="p-1 text-slate-300 hover:text-rose-600 rounded transition"
                            title="Delete"
                          >
                            <Trash2 className="w-3.5 h-3.5" />
                          </button>
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
