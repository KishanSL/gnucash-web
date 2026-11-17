import React from 'react';
import { Trash2 } from 'lucide-react';
import { Account } from '../../types';
import { AccountAutocomplete } from '../AccountAutocomplete';
import { SplitRowState } from './useTransactionModal';

export interface TransactionSplitRowProps {
  split: SplitRowState;
  index: number;
  validAccounts: Account[];
  onSplitChange: (index: number, field: keyof SplitRowState, value: string) => void;
  onRemoveSplit: (index: number) => void;
  mode: 'desktop' | 'mobile';
}

export const TransactionSplitRow: React.FC<TransactionSplitRowProps> = ({
  split,
  index,
  validAccounts,
  onSplitChange,
  onRemoveSplit,
  mode,
}) => {
  if (mode === 'mobile') {
    return (
      <div className="bg-slate-50 p-3 rounded-xl border border-slate-200 space-y-2">
        <div className="flex items-center justify-between">
          <span className="text-[11px] font-bold text-slate-600">Split #{index + 1}</span>
          <button
            type="button"
            onClick={() => onRemoveSplit(index)}
            className="text-slate-400 hover:text-rose-500 p-1"
            title="Remove split"
          >
            <Trash2 className="w-3.5 h-3.5" />
          </button>
        </div>

        <div>
          <label className="block text-[10px] uppercase font-semibold text-slate-400 mb-0.5">Account</label>
          <AccountAutocomplete
            accounts={validAccounts}
            value={split.account_id}
            onChange={(accountId) => onSplitChange(index, 'account_id', accountId)}
          />
        </div>

        <div>
          <label className="block text-[10px] uppercase font-semibold text-slate-400 mb-0.5">Memo</label>
          <input
            type="text"
            placeholder="Optional memo"
            value={split.memo}
            onChange={(e) => onSplitChange(index, 'memo', e.target.value)}
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
              value={split.debit}
              onChange={(e) => onSplitChange(index, 'debit', e.target.value)}
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
              value={split.credit}
              onChange={(e) => onSplitChange(index, 'credit', e.target.value)}
              className="w-full py-1.5 px-2 text-xs text-right font-mono bg-white border border-slate-300 rounded-lg focus:outline-none focus:ring-1 focus:ring-emerald-500 text-rose-700 font-bold"
            />
          </div>
        </div>
      </div>
    );
  }

  // Desktop view
  return (
    <tr className="hover:bg-slate-50">
      {/* Account Selector */}
      <td className="py-1.5 px-3 min-w-[240px]">
        <AccountAutocomplete
          accounts={validAccounts}
          value={split.account_id}
          onChange={(accountId) => onSplitChange(index, 'account_id', accountId)}
        />
      </td>

      {/* Memo */}
      <td className="py-1.5 px-2">
        <input
          type="text"
          placeholder="Optional memo"
          value={split.memo}
          onChange={(e) => onSplitChange(index, 'memo', e.target.value)}
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
          value={split.debit}
          onChange={(e) => onSplitChange(index, 'debit', e.target.value)}
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
          value={split.credit}
          onChange={(e) => onSplitChange(index, 'credit', e.target.value)}
          className="w-full py-1 px-2 text-xs text-right font-mono bg-white border border-slate-300 rounded focus:outline-none focus:ring-1 focus:ring-emerald-500 text-rose-700 font-semibold"
        />
      </td>

      {/* Remove split */}
      <td className="py-1.5 px-2 text-center">
        <button
          type="button"
          onClick={() => onRemoveSplit(index)}
          className="text-slate-300 hover:text-rose-500 transition p-1"
          title="Remove split"
        >
          <Trash2 className="w-3.5 h-3.5" />
        </button>
      </td>
    </tr>
  );
};
