import React from 'react';
import {
  X,
  Plus,
  CheckCircle2,
  AlertCircle,
  Wand2,
  Split,
  Sparkles,
  History,
} from 'lucide-react';
import { Account, CreateTransactionPayload, Transaction, UpdateTransactionPayload } from '../types';
import { formatCurrency } from '../api';
import { useTransactionModal } from './transaction_modal/useTransactionModal';
import { TransactionSplitRow } from './transaction_modal/TransactionSplitRow';

interface TransactionModalProps {
  isOpen: boolean;
  onClose: () => void;
  onSubmit: (payload: CreateTransactionPayload) => Promise<void>;
  onUpdate?: (id: string, payload: UpdateTransactionPayload) => Promise<void>;
  editingTransaction?: Transaction | null;
  accounts: Account[];
  defaultAccountId?: string;
  currentCurrency: string;
  transactions?: Transaction[];
}

export const TransactionModal: React.FC<TransactionModalProps> = ({
  isOpen,
  onClose,
  onSubmit,
  onUpdate,
  editingTransaction,
  accounts,
  defaultAccountId,
  currentCurrency,
  transactions = [],
}) => {
  const modal = useTransactionModal({
    isOpen,
    onClose,
    onSubmit,
    onUpdate,
    editingTransaction,
    accounts,
    defaultAccountId,
    currentCurrency,
    transactions,
  });

  if (!isOpen) return null;

  const {
    postDate,
    setPostDate,
    num,
    setNum,
    description,
    setDescription,
    notes,
    setNotes,
    submitting,
    error,
    autofillBanner,
    setAutofillBanner,
    isPayeeDropdownOpen,
    setIsPayeeDropdownOpen,
    payeeHighlightIndex,
    setPayeeHighlightIndex,
    payeeRef,
    payeeListRef,
    validAccounts,
    payeeHistory,
    filteredPayees,
    handleSelectPayee,
    handlePayeeKeyDown,
    splits,
    handleSplitChange,
    addSplitRow,
    removeSplitRow,
    totalDebits,
    totalCredits,
    imbalance,
    isBalanced,
    handleAutoBalance,
    handleSubmit,
  } = modal;

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
              <h3 className="font-bold text-slate-800 text-sm sm:text-base">
                {editingTransaction ? 'Edit Transaction' : 'New Double-Entry Transaction'}
              </h3>
              <p className="text-[11px] sm:text-xs text-slate-400">
                {editingTransaction
                  ? 'Modify balanced splits and transaction details'
                  : 'GnuCash multi-split transaction with balanced debits and credits'}
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

            <div className="sm:col-span-6 relative" ref={payeeRef}>
              <div className="flex items-center justify-between mb-1">
                <label className="block text-xs font-semibold text-slate-600">
                  Description / Payee
                </label>
                {payeeHistory.length > 0 && (
                  <span className="text-[10px] text-emerald-600 font-medium flex items-center space-x-1">
                    <Sparkles className="w-3 h-3" />
                    <span>Autofill on select</span>
                  </span>
                )}
              </div>
              <div className="relative">
                <input
                  type="text"
                  required
                  placeholder="e.g. Grocery Store, Client Invoice, Payroll"
                  value={description}
                  onFocus={() => {
                    if (payeeHistory.length > 0) {
                      setIsPayeeDropdownOpen(true);
                    }
                  }}
                  onClick={() => {
                    if (payeeHistory.length > 0) {
                      setIsPayeeDropdownOpen(true);
                    }
                  }}
                  onChange={(e) => {
                    setDescription(e.target.value);
                    if (!isPayeeDropdownOpen && payeeHistory.length > 0) {
                      setIsPayeeDropdownOpen(true);
                    }
                  }}
                  onKeyDown={handlePayeeKeyDown}
                  className="w-full px-3 py-1.5 text-xs bg-slate-50 border border-slate-300 rounded-lg focus:ring-2 focus:ring-emerald-500/20 focus:border-emerald-500 text-slate-900"
                  aria-expanded={isPayeeDropdownOpen}
                  aria-label="Description or Payee"
                />
                {description && (
                  <button
                    type="button"
                    onClick={() => setDescription('')}
                    className="absolute right-2 top-1/2 -translate-y-1/2 text-slate-400 hover:text-slate-600 p-0.5"
                    tabIndex={-1}
                  >
                    <X className="w-3 h-3" />
                  </button>
                )}
              </div>

              {/* Payee Autocomplete Dropdown */}
              {isPayeeDropdownOpen && filteredPayees.length > 0 && (
                <div className="absolute z-50 left-0 right-0 mt-1 bg-white border border-slate-200 rounded-xl shadow-2xl overflow-hidden animate-in fade-in slide-in-from-top-1 duration-150">
                  <div className="px-3 py-1.5 bg-slate-50 border-b border-slate-200 flex items-center justify-between text-[11px] text-slate-500 font-semibold">
                    <span className="flex items-center space-x-1">
                      <History className="w-3.5 h-3.5 text-emerald-600" />
                      <span>Previous Payees</span>
                    </span>
                    <span className="text-[10px] text-slate-400">↵ to autofill splits</span>
                  </div>
                  <ul ref={payeeListRef} className="max-h-52 overflow-y-auto divide-y divide-slate-100 py-1" role="listbox">
                    {filteredPayees.map((item, idx) => {
                      const isHighlighted = idx === payeeHighlightIndex;
                      return (
                        <li
                          key={item.payee}
                          role="option"
                          aria-selected={isHighlighted}
                          onMouseEnter={() => setPayeeHighlightIndex(idx)}
                          onClick={() => handleSelectPayee(item)}
                          className={`px-3 py-2 text-xs cursor-pointer flex items-center justify-between transition-colors ${
                            isHighlighted ? 'bg-emerald-50 text-emerald-900 font-medium' : 'hover:bg-slate-50 text-slate-800'
                          }`}
                        >
                          <div className="flex items-center space-x-2 truncate pr-2">
                            <div className="w-6 h-6 rounded-full bg-emerald-100 text-emerald-700 flex items-center justify-center text-[10px] font-bold shrink-0">
                              {item.payee.charAt(0).toUpperCase()}
                            </div>
                            <div className="truncate">
                              <div className="font-semibold text-slate-800 truncate">{item.payee}</div>
                              <div className="text-[10px] text-slate-400">
                                Last: {item.latestTx.post_date} • {item.latestTx.splits.length} splits
                              </div>
                            </div>
                          </div>
                          <span className="text-[10px] text-emerald-700 bg-emerald-50 border border-emerald-200 px-1.5 py-0.5 rounded font-medium shrink-0">
                            Autofill
                          </span>
                        </li>
                      );
                    })}
                  </ul>
                </div>
              )}
            </div>
          </div>

          {/* Autofill Notification Banner */}
          {autofillBanner && (
            <div className="p-2.5 bg-emerald-50 border border-emerald-200 text-emerald-800 rounded-lg text-xs flex items-center justify-between animate-in fade-in duration-200">
              <div className="flex items-center space-x-2">
                <Sparkles className="w-4 h-4 text-emerald-600 shrink-0" />
                <span className="font-medium">{autofillBanner}</span>
              </div>
              <button
                type="button"
                onClick={() => setAutofillBanner(null)}
                className="text-emerald-700 hover:text-emerald-900 text-xs font-bold"
              >
                Dismiss
              </button>
            </div>
          )}

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
                <TransactionSplitRow
                  key={s.id}
                  split={s}
                  index={idx}
                  validAccounts={validAccounts}
                  onSplitChange={handleSplitChange}
                  onRemoveSplit={removeSplitRow}
                  mode="mobile"
                />
              ))}
            </div>

            {/* Desktop View: Split Table (>= sm) */}
            <div className="hidden sm:block border border-slate-200 rounded-xl shadow-sm min-h-[160px]">
              <table className="w-full text-xs">
                <thead>
                  <tr className="bg-slate-100 text-slate-600 font-semibold border-b border-slate-200">
                    <th className="py-2 px-3 text-left min-w-[240px]">Account</th>
                    <th className="py-2 px-2 text-left">Memo</th>
                    <th className="py-2 px-2 text-right w-28">Debit</th>
                    <th className="py-2 px-2 text-right w-28">Credit</th>
                    <th className="py-2 px-2 text-center w-10"></th>
                  </tr>
                </thead>
                <tbody className="divide-y divide-slate-100">
                  {splits.map((s, idx) => (
                    <TransactionSplitRow
                      key={s.id}
                      split={s}
                      index={idx}
                      validAccounts={validAccounts}
                      onSplitChange={handleSplitChange}
                      onRemoveSplit={removeSplitRow}
                      mode="desktop"
                    />
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
              {submitting
                ? 'Saving...'
                : editingTransaction
                ? 'Update Transaction'
                : 'Record Transaction'}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
};
