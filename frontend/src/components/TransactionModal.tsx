import React, { useState, useEffect } from 'react';
import { X, Plus, Trash2, CheckCircle2, AlertCircle, Wand2, Split } from 'lucide-react';
import { Account, CreateTransactionPayload } from '../types';
import { formatCurrency } from '../api';

interface TransactionModalProps {
  isOpen: boolean;
  onClose: () => void;
  onSubmit: (payload: CreateTransactionPayload) => Promise<void>;
  accounts: Account[];
  defaultAccountId?: string;
  currentCurrency: string;
}

interface SplitRowState {
  id: string;
  account_id: string;
  memo: string;
  debit: string;
  credit: string;
}

export const TransactionModal: React.FC<TransactionModalProps> = ({
  isOpen,
  onClose,
  onSubmit,
  accounts,
  defaultAccountId,
  currentCurrency,
}) => {
  const [postDate, setPostDate] = useState(new Date().toISOString().split('T')[0]);
  const [num, setNum] = useState('');
  const [description, setDescription] = useState('');
  const [notes, setNotes] = useState('');
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Non-placeholder accounts only
  const validAccounts = accounts.filter((a) => !a.placeholder);

  // Initialize splits
  const [splits, setSplits] = useState<SplitRowState[]>([
    {
      id: '1',
      account_id: defaultAccountId || (validAccounts[0]?.id || ''),
      memo: '',
      debit: '',
      credit: '',
    },
    {
      id: '2',
      account_id: validAccounts[1]?.id || '',
      memo: '',
      debit: '',
      credit: '',
    },
  ]);

  useEffect(() => {
    if (isOpen) {
      setPostDate(new Date().toISOString().split('T')[0]);
      setNum('');
      setDescription('');
      setNotes('');
      setError(null);
      setSplits([
        {
          id: '1',
          account_id: defaultAccountId || (validAccounts[0]?.id || ''),
          memo: '',
          debit: '',
          credit: '',
        },
        {
          id: '2',
          account_id: validAccounts[1]?.id || '',
          memo: '',
          debit: '',
          credit: '',
        },
      ]);
    }
  }, [isOpen, defaultAccountId]);

  if (!isOpen) return null;

  const handleSplitChange = (index: number, field: keyof SplitRowState, value: string) => {
    setSplits((prev) => {
      const next = [...prev];
      if (field === 'debit' && value) {
        next[index] = { ...next[index], debit: value, credit: '' };
      } else if (field === 'credit' && value) {
        next[index] = { ...next[index], credit: value, debit: '' };
      } else {
        next[index] = { ...next[index], [field]: value };
      }
      return next;
    });
  };

  const addSplitRow = () => {
    setSplits((prev) => [
      ...prev,
      {
        id: Math.random().toString(),
        account_id: validAccounts[0]?.id || '',
        memo: '',
        debit: '',
        credit: '',
      },
    ]);
  };

  const removeSplitRow = (index: number) => {
    if (splits.length <= 2) {
      alert('A double-entry transaction must contain at least 2 splits.');
      return;
    }
    setSplits((prev) => prev.filter((_, i) => i !== index));
  };

  // Calculate totals
  let totalDebits = 0;
  let totalCredits = 0;

  for (const s of splits) {
    if (s.debit) totalDebits += parseFloat(s.debit) || 0;
    if (s.credit) totalCredits += parseFloat(s.credit) || 0;
  }

  const imbalance = Math.round((totalDebits - totalCredits) * 100) / 100;
  const isBalanced = Math.abs(imbalance) < 0.001 && totalDebits > 0;

  // Auto-balance button: adjusts or fills the remaining difference
  const handleAutoBalance = () => {
    if (Math.abs(imbalance) < 0.001) return;

    setSplits((prev) => {
      const next = [...prev];
      const lastIndex = next.length - 1;

      if (imbalance > 0) {
        // Debits exceed credits: add/increase credit on last row
        const currentCredit = parseFloat(next[lastIndex].credit) || 0;
        next[lastIndex].credit = (currentCredit + imbalance).toFixed(2);
        next[lastIndex].debit = '';
      } else {
        // Credits exceed debits: add/increase debit on last row
        const currentDebit = parseFloat(next[lastIndex].debit) || 0;
        next[lastIndex].debit = (currentDebit + Math.abs(imbalance)).toFixed(2);
        next[lastIndex].credit = '';
      }
      return next;
    });
  };

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setError(null);

    if (!description.trim()) {
      setError('Please provide a description or payee for the transaction.');
      return;
    }

    if (splits.length < 2) {
      setError('A transaction must have at least 2 splits.');
      return;
    }

    if (!isBalanced) {
      setError(
        `Transaction is not balanced! Total Debits (${formatCurrency(
          totalDebits,
          currentCurrency
        )}) must equal Total Credits (${formatCurrency(totalCredits, currentCurrency)}). Imbalance: ${formatCurrency(
          imbalance,
          currentCurrency
        )}`
      );
      return;
    }

    // Build payload
    const payloadSplits = splits.map((s) => {
      const debitVal = parseFloat(s.debit) || 0;
      const creditVal = parseFloat(s.credit) || 0;
      const netAmount = debitVal > 0 ? debitVal : -creditVal;

      return {
        account_id: s.account_id,
        amount: netAmount.toFixed(2),
        memo: s.memo.trim() ? s.memo.trim() : null,
      };
    });

    try {
      setSubmitting(true);
      await onSubmit({
        post_date: postDate,
        num: num.trim() ? num.trim() : null,
        description: description.trim(),
        notes: notes.trim() ? notes.trim() : null,
        splits: payloadSplits,
      });
      onClose();
    } catch (err: any) {
      setError(err.message || 'Failed to create transaction');
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <div className="fixed inset-0 z-50 overflow-y-auto bg-slate-900/60 backdrop-blur-sm flex items-center justify-center p-2 sm:p-4">
      <div className="bg-white rounded-2xl border border-slate-200 shadow-2xl max-w-3xl w-full overflow-hidden max-h-[94vh] flex flex-col animate-in fade-in zoom-in-95 duration-150">
        {/* Header */}
        <div className="px-4 sm:px-6 py-3 sm:py-4 border-b border-slate-200 bg-slate-50 flex items-center justify-between shrink-0">
          <div className="flex items-center space-x-2">
            <div className="w-8 h-8 rounded-lg bg-emerald-100 text-emerald-700 flex items-center justify-center shrink-0">
              <Split className="w-4 h-4" />
            </div>
            <div>
              <h3 className="font-bold text-slate-800 text-sm sm:text-base">New Double-Entry Transaction</h3>
              <p className="text-[11px] sm:text-xs text-slate-400">
                GnuCash multi-split transaction with balanced debits and credits
              </p>
            </div>
          </div>
          <button
            onClick={onClose}
            className="p-1 rounded-lg text-slate-400 hover:text-slate-600 hover:bg-slate-200 transition"
          >
            <X className="w-5 h-5" />
          </button>
        </div>

        {/* Form Body */}
        <form onSubmit={handleSubmit} className="p-4 sm:p-6 space-y-4 sm:space-y-5 overflow-y-auto flex-1">
          {error && (
            <div className="p-3 bg-rose-50 border border-rose-200 text-rose-700 rounded-lg text-xs flex items-center space-x-2">
              <AlertCircle className="w-4 h-4 shrink-0 text-rose-500" />
              <span>{error}</span>
            </div>
          )}

          {/* Top Fields: Date, Num, Description */}
          <div className="grid grid-cols-1 sm:grid-cols-12 gap-3">
            <div className="sm:col-span-3">
              <label className="block text-xs font-semibold text-slate-600 mb-1">Date</label>
              <input
                type="date"
                required
                value={postDate}
                onChange={(e) => setPostDate(e.target.value)}
                className="w-full px-3 py-1.5 text-xs bg-slate-50 border border-slate-300 rounded-lg focus:ring-2 focus:ring-emerald-500/20 focus:border-emerald-500 font-mono"
              />
            </div>

            <div className="sm:col-span-3">
              <label className="block text-xs font-semibold text-slate-600 mb-1">
                Num / Check #
              </label>
              <input
                type="text"
                placeholder="e.g. 102, DIR-DEP"
                value={num}
                onChange={(e) => setNum(e.target.value)}
                className="w-full px-3 py-1.5 text-xs bg-slate-50 border border-slate-300 rounded-lg focus:ring-2 focus:ring-emerald-500/20 focus:border-emerald-500 font-mono"
              />
            </div>

            <div className="sm:col-span-6">
              <label className="block text-xs font-semibold text-slate-600 mb-1">
                Description / Payee
              </label>
              <input
                type="text"
                required
                placeholder="e.g. Grocery Store, Client Invoice, Payroll"
                value={description}
                onChange={(e) => setDescription(e.target.value)}
                className="w-full px-3 py-1.5 text-xs bg-slate-50 border border-slate-300 rounded-lg focus:ring-2 focus:ring-emerald-500/20 focus:border-emerald-500"
              />
            </div>
          </div>

          {/* Splits Section */}
          <div className="space-y-2">
            <div className="flex items-center justify-between">
              <label className="text-xs font-bold text-slate-700 uppercase tracking-wider">
                Transaction Splits
              </label>
              <div className="flex items-center space-x-2">
                <button
                  type="button"
                  onClick={handleAutoBalance}
                  disabled={isBalanced || totalDebits + totalCredits === 0}
                  className="flex items-center space-x-1 px-2.5 py-1 text-[11px] font-medium text-emerald-700 bg-emerald-50 hover:bg-emerald-100 disabled:opacity-40 disabled:cursor-not-allowed border border-emerald-200 rounded-md transition"
                  title="Auto-calculate the balancing split"
                >
                  <Wand2 className="w-3.5 h-3.5" />
                  <span className="hidden xs:inline">Auto-Balance</span>
                </button>
                <button
                  type="button"
                  onClick={addSplitRow}
                  className="flex items-center space-x-1 px-2.5 py-1 text-[11px] font-medium text-slate-700 bg-slate-100 hover:bg-slate-200 border border-slate-200 rounded-md transition"
                >
                  <Plus className="w-3.5 h-3.5 text-emerald-600" />
                  <span>Add Split</span>
                </button>
              </div>
            </div>

            {/* Mobile View: Split Cards (< sm) */}
            <div className="sm:hidden space-y-2.5">
              {splits.map((s, idx) => (
                <div key={s.id} className="bg-slate-50 p-3 rounded-xl border border-slate-200 space-y-2">
                  <div className="flex items-center justify-between">
                    <span className="text-[11px] font-bold text-slate-600">Split #{idx + 1}</span>
                    <button
                      type="button"
                      onClick={() => removeSplitRow(idx)}
                      className="text-slate-400 hover:text-rose-500 p-1"
                      title="Remove split"
                    >
                      <Trash2 className="w-3.5 h-3.5" />
                    </button>
                  </div>

                  <div>
                    <label className="block text-[10px] uppercase font-semibold text-slate-400 mb-0.5">Account</label>
                    <select
                      value={s.account_id}
                      onChange={(e) => handleSplitChange(idx, 'account_id', e.target.value)}
                      className="w-full py-1.5 px-2 text-xs bg-white border border-slate-300 rounded-lg focus:outline-none focus:ring-1 focus:ring-emerald-500 font-medium"
                    >
                      {validAccounts.map((a) => (
                        <option key={a.id} value={a.id}>
                          {a.code ? `${a.code} - ` : ''}
                          {a.name} ({a.account_type})
                        </option>
                      ))}
                    </select>
                  </div>

                  <div>
                    <label className="block text-[10px] uppercase font-semibold text-slate-400 mb-0.5">Memo</label>
                    <input
                      type="text"
                      placeholder="Optional memo"
                      value={s.memo}
                      onChange={(e) => handleSplitChange(idx, 'memo', e.target.value)}
                      className="w-full py-1.5 px-2 text-xs bg-white border border-slate-300 rounded-lg focus:outline-none focus:ring-1 focus:ring-emerald-500"
                    />
                  </div>

                  <div className="grid grid-cols-2 gap-2">
                    <div>
                      <label className="block text-[10px] uppercase font-semibold text-emerald-700 mb-0.5">Debit (+)</label>
                      <input
                        type="number"
                        step="0.01"
                        min="0"
                        placeholder="0.00"
                        value={s.debit}
                        onChange={(e) => handleSplitChange(idx, 'debit', e.target.value)}
                        className="w-full py-1.5 px-2 text-xs text-right font-mono bg-white border border-slate-300 rounded-lg focus:outline-none focus:ring-1 focus:ring-emerald-500 text-emerald-700 font-bold"
                      />
                    </div>
                    <div>
                      <label className="block text-[10px] uppercase font-semibold text-rose-700 mb-0.5">Credit (-)</label>
                      <input
                        type="number"
                        step="0.01"
                        min="0"
                        placeholder="0.00"
                        value={s.credit}
                        onChange={(e) => handleSplitChange(idx, 'credit', e.target.value)}
                        className="w-full py-1.5 px-2 text-xs text-right font-mono bg-white border border-slate-300 rounded-lg focus:outline-none focus:ring-1 focus:ring-emerald-500 text-rose-700 font-bold"
                      />
                    </div>
                  </div>
                </div>
              ))}
            </div>

            {/* Desktop View: Split Table (>= sm) */}
            <div className="hidden sm:block border border-slate-200 rounded-xl overflow-x-auto shadow-sm">
              <table className="w-full text-xs">
                <thead>
                  <tr className="bg-slate-100 text-slate-600 font-semibold border-b border-slate-200">
                    <th className="py-2 px-3 text-left w-56">Account</th>
                    <th className="py-2 px-2 text-left">Memo</th>
                    <th className="py-2 px-2 text-right w-28">Debit</th>
                    <th className="py-2 px-2 text-right w-28">Credit</th>
                    <th className="py-2 px-2 text-center w-10"></th>
                  </tr>
                </thead>
                <tbody className="divide-y divide-slate-100">
                  {splits.map((s, idx) => (
                    <tr key={s.id} className="hover:bg-slate-50">
                      {/* Account Selector */}
                      <td className="py-1.5 px-3">
                        <select
                          value={s.account_id}
                          onChange={(e) => handleSplitChange(idx, 'account_id', e.target.value)}
                          className="w-full py-1 px-2 text-xs bg-white border border-slate-300 rounded focus:outline-none focus:ring-1 focus:ring-emerald-500 font-medium"
                        >
                          {validAccounts.map((a) => (
                            <option key={a.id} value={a.id}>
                              {a.code ? `${a.code} - ` : ''}
                              {a.name} ({a.account_type})
                            </option>
                          ))}
                        </select>
                      </td>

                      {/* Memo */}
                      <td className="py-1.5 px-2">
                        <input
                          type="text"
                          placeholder="Optional memo"
                          value={s.memo}
                          onChange={(e) => handleSplitChange(idx, 'memo', e.target.value)}
                          className="w-full py-1 px-2 text-xs bg-white border border-slate-300 rounded focus:outline-none focus:ring-1 focus:ring-emerald-500"
                        />
                      </td>

                      {/* Debit */}
                      <td className="py-1.5 px-2">
                        <input
                          type="number"
                          step="0.01"
                          min="0"
                          placeholder="0.00"
                          value={s.debit}
                          onChange={(e) => handleSplitChange(idx, 'debit', e.target.value)}
                          className="w-full py-1 px-2 text-xs text-right font-mono bg-white border border-slate-300 rounded focus:outline-none focus:ring-1 focus:ring-emerald-500 text-emerald-700 font-semibold"
                        />
                      </td>

                      {/* Credit */}
                      <td className="py-1.5 px-2">
                        <input
                          type="number"
                          step="0.01"
                          min="0"
                          placeholder="0.00"
                          value={s.credit}
                          onChange={(e) => handleSplitChange(idx, 'credit', e.target.value)}
                          className="w-full py-1 px-2 text-xs text-right font-mono bg-white border border-slate-300 rounded focus:outline-none focus:ring-1 focus:ring-emerald-500 text-rose-700 font-semibold"
                        />
                      </td>

                      {/* Remove split */}
                      <td className="py-1.5 px-2 text-center">
                        <button
                          type="button"
                          onClick={() => removeSplitRow(idx)}
                          className="text-slate-300 hover:text-rose-500 transition p-1"
                          title="Remove split"
                        >
                          <Trash2 className="w-3.5 h-3.5" />
                        </button>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>

            {/* Totals & Balance Status Bar (Responsive) */}
            <div className="bg-slate-50 border border-slate-200 rounded-xl p-3 flex flex-col sm:flex-row items-stretch sm:items-center justify-between gap-2.5 text-xs">
              <div className="flex items-center space-x-2">
                {isBalanced ? (
                  <div className="flex items-center space-x-1.5 text-emerald-700 font-semibold bg-emerald-100 px-2.5 py-1 rounded-full border border-emerald-300">
                    <CheckCircle2 className="w-4 h-4 text-emerald-600 shrink-0" />
                    <span>Balanced! (Diff: 0.00)</span>
                  </div>
                ) : (
                  <div className="flex items-center space-x-1.5 text-rose-700 font-semibold bg-rose-100 px-2.5 py-1 rounded-full border border-rose-300">
                    <AlertCircle className="w-4 h-4 text-rose-600 shrink-0" />
                    <span>
                      Diff: {formatCurrency(Math.abs(imbalance), currentCurrency)} (
                      {imbalance > 0 ? 'Need Credits' : 'Need Debits'})
                    </span>
                  </div>
                )}
              </div>

              <div className="flex items-center justify-between sm:justify-end space-x-4 font-mono text-xs">
                <div>
                  <span className="text-slate-400 mr-1.5 font-sans font-medium text-[11px]">Debits:</span>
                  <span className="font-bold text-emerald-700">{formatCurrency(totalDebits, currentCurrency)}</span>
                </div>
                <div>
                  <span className="text-slate-400 mr-1.5 font-sans font-medium text-[11px]">Credits:</span>
                  <span className="font-bold text-rose-700">{formatCurrency(totalCredits, currentCurrency)}</span>
                </div>
              </div>
            </div>
          </div>

          {/* Optional Notes */}
          <div>
            <label className="block text-xs font-semibold text-slate-600 mb-1">
              Internal Notes (Optional)
            </label>
            <textarea
              rows={2}
              placeholder="Add details, invoice numbers, receipt info..."
              value={notes}
              onChange={(e) => setNotes(e.target.value)}
              className="w-full px-3 py-1.5 text-xs bg-slate-50 border border-slate-300 rounded-lg focus:ring-2 focus:ring-emerald-500/20 focus:border-emerald-500"
            />
          </div>

          {/* Action Buttons */}
          <div className="flex items-center justify-end space-x-3 pt-3 border-t border-slate-200">
            <button
              type="button"
              onClick={onClose}
              className="px-4 py-2 text-xs font-semibold text-slate-600 hover:text-slate-800 bg-slate-100 hover:bg-slate-200 rounded-lg transition"
            >
              Cancel
            </button>

            <button
              type="submit"
              disabled={submitting || !isBalanced}
              className="px-5 py-2 text-xs font-semibold text-white bg-emerald-600 hover:bg-emerald-500 disabled:opacity-50 disabled:cursor-not-allowed rounded-lg shadow-sm transition active:scale-95 flex items-center space-x-1.5"
            >
              {submitting ? 'Saving...' : 'Record Transaction'}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
};
