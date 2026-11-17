import React from 'react';
import { Plus, Check, X, AlertCircle } from 'lucide-react';
import { Account } from '../../types';
import { AccountAutocomplete } from '../AccountAutocomplete';
import { useAccountRegister } from './useAccountRegister';

export interface RegisterInlineEntryProps {
  account: Account;
  currentCurrency: string;
  registerHook: ReturnType<typeof useAccountRegister>;
  mode: 'desktop' | 'mobile';
}

export const RegisterInlineEntry: React.FC<RegisterInlineEntryProps> = ({
  account,
  currentCurrency,
  registerHook,
  mode,
}) => {
  const {
    debitHeader,
    creditHeader,
    newDate,
    setNewDate,
    newNum,
    setNewNum,
    newDescription,
    setNewDescription,
    newNotes,
    setNewNotes,
    newMemo,
    setNewMemo,
    newTransferAccountId,
    setNewTransferAccountId,
    newDebit,
    setNewDebit,
    newCredit,
    setNewCredit,
    mobileAmount,
    setMobileAmount,
    mobileAmountType,
    setMobileAmountType,
    isSubmittingNew,
    newRowError,
    setNewRowError,
    isPayeeDropdownOpen,
    setIsPayeeDropdownOpen,
    payeeHighlightIndex,
    setPayeeHighlightIndex,
    payeeRef,
    tableBottomRef,
    mobileBottomRef,
    transferAccounts,
    filteredPayees,
    handleSelectPayee,
    handleResetNewRow,
    handleSaveNewRow,
    handleSaveMobileNewCard,
    handleRowKeyDown,
  } = registerHook;

  if (mode === 'mobile') {
    return (
      <div ref={mobileBottomRef} className="p-3.5 bg-emerald-50/30 border-t-2 border-emerald-500/50 space-y-3">
        <div className="flex items-center justify-between">
          <span className="text-xs font-bold text-emerald-800 flex items-center space-x-1.5">
            <Plus className="w-3.5 h-3.5" />
            <span>New Transaction (Blank Entry)</span>
          </span>
          <span className="text-[10px] uppercase font-bold tracking-wider px-1.5 py-0.5 rounded bg-emerald-100 text-emerald-800 border border-emerald-300">
            2 Splits
          </span>
        </div>

        {newRowError && (
          <div className="flex items-center space-x-1.5 text-xs text-rose-700 bg-rose-50 border border-rose-200 p-2 rounded-lg">
            <AlertCircle className="w-3.5 h-3.5 shrink-0" />
            <span>{newRowError}</span>
          </div>
        )}

        <div className="grid grid-cols-3 gap-2">
          <div className="col-span-2">
            <label className="block text-[10px] font-semibold text-slate-500 mb-0.5">Date</label>
            <input
              type="date"
              value={newDate}
              onChange={(e) => setNewDate(e.target.value)}
              className="w-full text-xs font-mono py-1.5 px-2 border border-slate-300 rounded-lg bg-white focus:outline-none focus:ring-1 focus:ring-emerald-500"
            />
          </div>
          <div>
            <label className="block text-[10px] font-semibold text-slate-500 mb-0.5">Num</label>
            <input
              type="text"
              placeholder="# / Check"
              value={newNum}
              onChange={(e) => setNewNum(e.target.value)}
              className="w-full text-xs font-mono py-1.5 px-2 border border-slate-300 rounded-lg bg-white focus:outline-none focus:ring-1 focus:ring-emerald-500"
            />
          </div>
        </div>

        <div className="relative">
          <label className="block text-[10px] font-semibold text-slate-500 mb-0.5">Payee / Description</label>
          <input
            type="text"
            placeholder="Payee or description..."
            value={newDescription}
            onChange={(e) => {
              setNewDescription(e.target.value);
              setIsPayeeDropdownOpen(true);
            }}
            onFocus={() => setIsPayeeDropdownOpen(true)}
            onClick={() => setIsPayeeDropdownOpen(true)}
            className="w-full text-xs py-1.5 px-2.5 border border-slate-300 rounded-lg bg-white focus:outline-none focus:ring-1 focus:ring-emerald-500 font-medium"
          />
          {/* Payee autocomplete suggestions (dropup) */}
          {isPayeeDropdownOpen && filteredPayees.length > 0 && (
            <div data-testid="mobile-payee-autocomplete-dropdown" className="absolute z-50 left-0 right-0 bottom-full mb-1 bg-white border border-slate-200 rounded-xl shadow-xl overflow-hidden max-h-48 overflow-y-auto">
              <div className="px-2.5 py-1 bg-slate-50 border-b border-slate-200 text-[10px] font-semibold text-slate-500 flex items-center justify-between">
                <span>Recent Payees ({filteredPayees.length})</span>
                <span className="text-emerald-700 font-medium">Autofills transfer</span>
              </div>
              {filteredPayees.map((item, idx) => (
                <button
                  key={item.payee}
                  type="button"
                  onClick={() => handleSelectPayee(item)}
                  className={`w-full text-left px-2.5 py-1.5 text-xs flex items-center justify-between hover:bg-emerald-50/80 transition ${
                    idx === payeeHighlightIndex ? 'bg-emerald-50 text-emerald-900 font-medium' : 'text-slate-800'
                  }`}
                >
                  <span className="truncate">{item.payee}</span>
                  <span className="text-[10px] text-slate-400 font-mono ml-2 shrink-0">{item.count}×</span>
                </button>
              ))}
            </div>
          )}
        </div>

        <div>
          <label className="block text-[10px] font-semibold text-slate-500 mb-0.5">Transfer Account (Split 2)</label>
          <AccountAutocomplete
            accounts={transferAccounts}
            value={newTransferAccountId}
            onChange={setNewTransferAccountId}
            placeholder="Select transfer account..."
            dropdownPosition="top"
          />
        </div>

        <div>
          <label className="block text-[10px] font-semibold text-slate-500 mb-0.5">Amount ({currentCurrency || account.currency})</label>
          <div className="flex rounded-lg border border-slate-300 overflow-hidden bg-white focus-within:ring-1 focus-within:ring-emerald-500 focus-within:border-emerald-500">
            <div className="flex bg-slate-100 border-r border-slate-300 p-0.5">
              <button
                type="button"
                onClick={() => setMobileAmountType('debit')}
                className={`px-2 py-1 text-[11px] font-bold rounded transition ${
                  mobileAmountType === 'debit'
                    ? 'bg-emerald-600 text-white shadow-sm'
                    : 'text-slate-600 hover:text-slate-900'
                }`}
              >
                + {debitHeader.split('/')[0].trim()}
              </button>
              <button
                type="button"
                onClick={() => setMobileAmountType('credit')}
                className={`px-2 py-1 text-[11px] font-bold rounded transition ${
                  mobileAmountType === 'credit'
                    ? 'bg-rose-600 text-white shadow-sm'
                    : 'text-slate-600 hover:text-slate-900'
                }`}
              >
                - {creditHeader.split('/')[0].trim()}
              </button>
            </div>
            <input
              type="number"
              step="0.01"
              min="0"
              placeholder="0.00"
              value={mobileAmount}
              onChange={(e) => setMobileAmount(e.target.value)}
              className="flex-1 py-1.5 px-2.5 text-xs font-mono font-bold text-slate-800 focus:outline-none"
            />
          </div>
        </div>

        <div className="grid grid-cols-2 gap-2">
          <div>
            <label className="block text-[10px] font-semibold text-slate-500 mb-0.5">Notes (Optional)</label>
            <input
              type="text"
              placeholder="Notes..."
              value={newNotes}
              onChange={(e) => setNewNotes(e.target.value)}
              className="w-full text-xs py-1.5 px-2 border border-slate-300 rounded-lg bg-white focus:outline-none focus:ring-1 focus:ring-emerald-500"
            />
          </div>
          <div>
            <label className="block text-[10px] font-semibold text-slate-500 mb-0.5">Memo (Optional)</label>
            <input
              type="text"
              placeholder="Memo..."
              value={newMemo}
              onChange={(e) => setNewMemo(e.target.value)}
              className="w-full text-xs py-1.5 px-2 border border-slate-300 rounded-lg bg-white focus:outline-none focus:ring-1 focus:ring-emerald-500"
            />
          </div>
        </div>

        <div className="flex items-center space-x-2 pt-1">
          <button
            type="button"
            onClick={handleSaveMobileNewCard}
            disabled={isSubmittingNew}
            className="flex-1 flex items-center justify-center space-x-1.5 py-2 px-3 text-xs font-bold text-white bg-emerald-600 hover:bg-emerald-500 rounded-lg shadow-sm transition disabled:opacity-50"
          >
            <Check className="w-4 h-4" />
            <span>Record Transaction</span>
          </button>
          <button
            type="button"
            onClick={handleResetNewRow}
            className="py-2 px-3 text-xs font-semibold text-slate-500 hover:text-slate-700 bg-white border border-slate-300 rounded-lg transition"
          >
            Clear
          </button>
        </div>
      </div>
    );
  }

  // Desktop View
  return (
    <>
      <tr
        ref={tableBottomRef}
        data-testid="blank-new-row-main"
        className="bg-emerald-50/40 border-t-2 border-emerald-500 hover:bg-emerald-50/60 transition-colors"
      >
        {/* Date */}
        <td className="py-2 px-2.5">
          <input
            type="date"
            value={newDate}
            onChange={(e) => setNewDate(e.target.value)}
            onKeyDown={handleRowKeyDown}
            className="w-full text-xs font-mono py-1 px-1.5 border border-slate-300 rounded focus:border-emerald-500 focus:outline-none bg-white shadow-inner"
            title="Transaction Date (defaults to today)"
          />
        </td>

        {/* Num */}
        <td className="py-2 px-1.5">
          <input
            type="text"
            placeholder="Num"
            value={newNum}
            onChange={(e) => setNewNum(e.target.value)}
            onKeyDown={handleRowKeyDown}
            className="w-full text-xs font-mono py-1 px-1 border border-slate-300 rounded focus:border-emerald-500 focus:outline-none bg-white text-center shadow-inner"
            title="Transaction Number / Check #"
          />
        </td>

        {/* Description / Payee with dropup autocomplete */}
        <td className="py-2 px-2.5 relative">
          <div className="relative" ref={payeeRef}>
            <input
              type="text"
              placeholder="Description / Payee (autocomplete)..."
              value={newDescription}
              onChange={(e) => {
                setNewDescription(e.target.value);
                setIsPayeeDropdownOpen(true);
              }}
              onFocus={() => setIsPayeeDropdownOpen(true)}
              onClick={() => setIsPayeeDropdownOpen(true)}
              onKeyDown={(e) => {
                if (isPayeeDropdownOpen && filteredPayees.length > 0) {
                  if (e.key === 'ArrowDown') {
                    e.preventDefault();
                    setPayeeHighlightIndex((prev) =>
                      prev < filteredPayees.length - 1 ? prev + 1 : 0
                    );
                    return;
                  } else if (e.key === 'ArrowUp') {
                    e.preventDefault();
                    setPayeeHighlightIndex((prev) =>
                      prev > 0 ? prev - 1 : filteredPayees.length - 1
                    );
                    return;
                  } else if (e.key === 'Enter') {
                    e.preventDefault();
                    if (filteredPayees[payeeHighlightIndex]) {
                      handleSelectPayee(filteredPayees[payeeHighlightIndex]);
                    }
                    return;
                  } else if (e.key === 'Escape') {
                    e.preventDefault();
                    setIsPayeeDropdownOpen(false);
                    return;
                  }
                }
                handleRowKeyDown(e);
              }}
              className="w-full text-xs font-medium py-1 px-2 border border-slate-300 rounded focus:border-emerald-500 focus:outline-none bg-white shadow-inner"
            />

            {/* Payee dropup suggestions */}
            {isPayeeDropdownOpen && filteredPayees.length > 0 && (
              <div data-testid="payee-autocomplete-dropdown" className="absolute z-50 left-0 right-0 bottom-full mb-1 bg-white border border-slate-200 rounded-xl shadow-2xl overflow-hidden max-h-48 overflow-y-auto">
                <div className="px-2.5 py-1 bg-slate-50 border-b border-slate-200 text-[10px] font-semibold text-slate-500 flex items-center justify-between">
                  <span>Payee History ({filteredPayees.length})</span>
                  <span className="text-emerald-700 font-medium">Autofills transfer</span>
                </div>
                {filteredPayees.map((item, idx) => (
                  <button
                    key={item.payee}
                    type="button"
                    onClick={() => handleSelectPayee(item)}
                    className={`w-full text-left px-2.5 py-1.5 text-xs flex items-center justify-between hover:bg-emerald-50/80 transition ${
                      idx === payeeHighlightIndex ? 'bg-emerald-50 text-emerald-900 font-medium' : 'text-slate-800'
                    }`}
                  >
                    <span className="truncate">{item.payee}</span>
                    <span className="text-[10px] text-slate-400 font-mono ml-2 shrink-0">{item.count}×</span>
                  </button>
                ))}
              </div>
            )}
          </div>
        </td>

        {/* Transfer Account with dropup autocomplete */}
        <td className="py-2 px-2.5">
          <AccountAutocomplete
            accounts={transferAccounts}
            value={newTransferAccountId}
            onChange={setNewTransferAccountId}
            placeholder="Select transfer account..."
            dropdownPosition="top"
            className="text-xs"
          />
        </td>

        {/* Reconcile Status (Fixed 'n') */}
        <td className="py-2 px-1 text-center">
          <span className="inline-flex items-center px-1.5 py-0.5 rounded text-[10px] font-medium text-slate-400 select-none" title="Not Reconciled (n)">
            n
          </span>
        </td>

        {/* Debit (Deposit / Increase) */}
        <td className="py-2 px-2.5 text-right">
          <input
            type="number"
            step="0.01"
            min="0"
            placeholder="0.00"
            value={newDebit}
            onChange={(e) => {
              setNewDebit(e.target.value);
              if (e.target.value) setNewCredit('');
              setNewRowError(null);
            }}
            onKeyDown={handleRowKeyDown}
            className="w-full text-right text-xs font-mono font-bold text-emerald-700 py-1 px-2 border border-slate-300 rounded focus:border-emerald-500 focus:outline-none bg-white shadow-inner"
            title={`Enter ${debitHeader}`}
          />
        </td>

        {/* Credit (Withdrawal / Decrease) */}
        <td className="py-2 px-2.5 text-right">
          <input
            type="number"
            step="0.01"
            min="0"
            placeholder="0.00"
            value={newCredit}
            onChange={(e) => {
              setNewCredit(e.target.value);
              if (e.target.value) setNewDebit('');
              setNewRowError(null);
            }}
            onKeyDown={handleRowKeyDown}
            className="w-full text-right text-xs font-mono font-bold text-rose-700 py-1 px-2 border border-slate-300 rounded focus:border-emerald-500 focus:outline-none bg-white shadow-inner"
            title={`Enter ${creditHeader}`}
          />
        </td>

        {/* Balance placeholder */}
        <td className="py-2 px-3 text-right font-mono text-[11px] text-slate-400 bg-slate-100/50 select-none">
          (New)
        </td>

        {/* Actions: Save / Enter & Clear */}
        <td className="py-2 px-2 text-center">
          <div className="flex items-center justify-center space-x-1">
            <button
              type="button"
              onClick={handleSaveNewRow}
              disabled={isSubmittingNew}
              className="p-1 text-white bg-emerald-600 hover:bg-emerald-500 rounded transition shadow-sm disabled:opacity-50"
              title="Record Transaction (Enter)"
            >
              <Check className="w-3.5 h-3.5" />
            </button>
            <button
              type="button"
              onClick={handleResetNewRow}
              className="p-1 text-slate-400 hover:text-slate-600 rounded transition"
              title="Clear Row (Esc)"
            >
              <X className="w-3.5 h-3.5" />
            </button>
          </div>
        </td>
      </tr>

      {/* Desktop View: Blank New Transaction Row 2 (Notes & Memo) */}
      <tr
        data-testid="blank-new-row-notes"
        className="bg-emerald-50/40 border-b-2 border-emerald-500/50"
      >
        <td className="py-1.5 px-3"></td>
        <td className="py-1.5 px-2"></td>
        <td colSpan={6} className="py-1.5 px-2.5">
          <div className="flex items-center space-x-3">
            <div className="flex-1 flex items-center space-x-1.5">
              <span className="text-[10px] font-semibold text-slate-500 uppercase tracking-wider shrink-0">
                Notes:
              </span>
              <input
                type="text"
                placeholder="Transaction notes / details..."
                value={newNotes}
                onChange={(e) => setNewNotes(e.target.value)}
                onKeyDown={handleRowKeyDown}
                className="w-full text-xs py-1 px-2 border border-slate-300 rounded focus:border-emerald-500 focus:outline-none bg-white shadow-inner"
              />
            </div>
            <div className="flex-1 flex items-center space-x-1.5">
              <span className="text-[10px] font-semibold text-slate-500 uppercase tracking-wider shrink-0">
                Memo:
              </span>
              <input
                type="text"
                placeholder="Split memo..."
                value={newMemo}
                onChange={(e) => setNewMemo(e.target.value)}
                onKeyDown={handleRowKeyDown}
                className="w-full text-xs py-1 px-2 border border-slate-300 rounded focus:border-emerald-500 focus:outline-none bg-white shadow-inner"
              />
            </div>
          </div>
        </td>
        <td className="py-1.5 px-2 text-center"></td>
      </tr>

      {/* Error banner below new row if validation fails */}
      {newRowError && (
        <tr className="bg-rose-50 border-b border-rose-200">
          <td colSpan={9} className="py-2 px-4 text-xs text-rose-700 font-medium">
            <div className="flex items-center space-x-1.5">
              <AlertCircle className="w-3.5 h-3.5 shrink-0" />
              <span>{newRowError}</span>
            </div>
          </td>
        </tr>
      )}
    </>
  );
};
