import React from 'react';
import {
  ChevronDown,
  ChevronUp,
  Trash2,
  CornerDownRight,
  Split,
  Pencil,
} from 'lucide-react';
import { Account, RegisterEntry } from '../../types';
import { formatCurrency } from '../../api';

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

export interface RegisterRowProps {
  entry: RegisterEntry;
  account: Account;
  currentCurrency: string;
  isExpanded: boolean;
  onToggleExpand: (txId: string) => void;
  onEditTransaction?: (txId: string) => void;
  onDeleteTransaction: (txId: string) => void;
  mode: 'desktop' | 'mobile';
}

export const RegisterRow: React.FC<RegisterRowProps> = ({
  entry,
  account,
  currentCurrency,
  isExpanded,
  onToggleExpand,
  onEditTransaction,
  onDeleteTransaction,
  mode,
}) => {
  const isMultiSplit = entry.all_splits.length > 2;
  const currencyToUse = currentCurrency || account.currency;

  if (mode === 'mobile') {
    const hasDeposit = !!entry.debit && parseFloat(entry.debit) > 0;

    return (
      <div className="p-3 space-y-2 hover:bg-slate-50 transition">
        <div className="flex items-center justify-between text-xs">
          <div className="flex items-center space-x-1.5">
            <span className="font-mono text-slate-500 font-semibold">{entry.post_date}</span>
            {entry.num && (
              <span className="font-mono text-[10px] bg-slate-100 text-slate-500 px-1 rounded">
                #{entry.num}
              </span>
            )}
            {getReconciledBadge(entry.reconciled_state)}
          </div>

          {/* Amount */}
          <div className="text-right">
            {hasDeposit ? (
              <span className="font-mono font-bold text-emerald-700">
                +{formatCurrency(entry.debit, currencyToUse)}
              </span>
            ) : (
              <span className="font-mono font-bold text-rose-700">
                -{formatCurrency(entry.credit, currencyToUse)}
              </span>
            )}
          </div>
        </div>

        <div className="flex items-center justify-between text-xs">
          <div className="min-w-0 pr-2">
            <div className="font-bold text-slate-900 truncate">{entry.description}</div>
            <div className="text-[11px] text-slate-500 truncate mt-0.5">
              {isMultiSplit ? (
                <span className="text-emerald-700 font-medium">-- Split ({entry.all_splits.length}) --</span>
              ) : (
                entry.transfer_account_name
              )}
            </div>
          </div>

          <div className="text-right shrink-0">
            <span className="text-[10px] text-slate-400 block uppercase">Balance</span>
            <span className="font-mono font-semibold text-slate-700 text-xs">
              {formatCurrency(entry.running_balance, currencyToUse)}
            </span>
          </div>
        </div>

        {/* Second Row with Details / Memo / Notes */}
        <div className="bg-slate-50 rounded-lg p-2 border border-slate-100 text-[11px] space-y-1">
          <div className="flex items-baseline space-x-1.5">
            <span className="text-[10px] font-semibold text-slate-400 uppercase tracking-wider shrink-0">
              Notes:
            </span>
            <span className={entry.notes ? 'text-slate-700 font-medium' : 'text-slate-400 italic'}>
              {entry.notes || '—'}
            </span>
          </div>
          <div className="flex items-baseline space-x-1.5">
            <span className="text-[10px] font-semibold text-slate-400 uppercase tracking-wider shrink-0">
              Memo:
            </span>
            <span className={entry.memo || (entry.all_splits.find(s => s.memo)?.memo) ? 'text-slate-700 font-medium' : 'text-slate-400 italic'}>
              {entry.memo || (entry.all_splits.find(s => s.memo)?.memo) || '—'}
            </span>
          </div>
        </div>

        {/* Mobile Actions: Expand splits, Edit, or Delete */}
        <div className="flex items-center justify-between pt-1 border-t border-slate-100 text-xs">
          {isMultiSplit ? (
            <button
              onClick={() => onToggleExpand(entry.transaction_id)}
              className="inline-flex items-center space-x-1 text-emerald-700 font-medium text-[11px]"
            >
              <Split className="w-3 h-3" />
              <span>{isExpanded ? 'Hide Splits' : 'View Splits'}</span>
            </button>
          ) : (
            <span />
          )}

          <div className="flex items-center space-x-1">
            {onEditTransaction && (
              <button
                onClick={() => onEditTransaction(entry.transaction_id)}
                className="p-1.5 text-slate-400 hover:text-emerald-700 hover:bg-emerald-50 rounded transition"
                title="Edit Transaction"
              >
                <Pencil className="w-3.5 h-3.5" />
              </button>
            )}
            <button
              onClick={() => {
                if (confirm('Delete this transaction and all its splits?')) {
                  onDeleteTransaction(entry.transaction_id);
                }
              }}
              className="p-1.5 text-slate-400 hover:text-rose-600 hover:bg-rose-50 rounded transition"
              title="Delete"
            >
              <Trash2 className="w-3.5 h-3.5" />
            </button>
          </div>
        </div>

        {/* Expanded Splits in Mobile Card */}
        {isExpanded && (
          <div className="mt-2 bg-slate-50 rounded-lg p-2.5 border border-slate-200 text-xs space-y-1">
            <div className="font-semibold text-slate-700 text-[11px] mb-1">
              Double-Entry Splits:
            </div>
            {entry.all_splits.map((s) => {
              const amt = parseFloat(s.amount);
              return (
                <div key={s.id} className="flex justify-between py-0.5 text-[11px] border-b border-slate-100 last:border-none">
                  <span className={s.account_id === account.id ? 'font-bold text-slate-900' : 'text-slate-600'}>
                    {s.account_name}
                  </span>
                  <span className={`font-mono ${amt > 0 ? 'text-emerald-700 font-semibold' : 'text-rose-700'}`}>
                    {formatCurrency(Math.abs(amt), currencyToUse)}
                  </span>
                </div>
              );
            })}
          </div>
        )}
      </div>
    );
  }

  // Desktop View
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
                onClick={() => onToggleExpand(entry.transaction_id)}
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
              onClick={() => onToggleExpand(entry.transaction_id)}
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
          {entry.debit ? formatCurrency(entry.debit, currencyToUse) : ''}
        </td>

        {/* Credit */}
        <td className="py-2 px-3 text-right font-mono text-slate-800">
          {entry.credit ? formatCurrency(entry.credit, currencyToUse) : ''}
        </td>

        {/* Running Balance */}
        <td className="py-2 px-3 text-right font-mono font-semibold bg-slate-50/60 text-slate-900">
          {formatCurrency(entry.running_balance, currencyToUse)}
        </td>

        {/* Actions: Edit, Delete */}
        <td className="py-2 px-2 text-center">
          <div className="flex items-center justify-center space-x-1">
            {onEditTransaction && (
              <button
                onClick={() => onEditTransaction(entry.transaction_id)}
                className="p-1 text-slate-400 hover:text-emerald-700 hover:bg-emerald-50 rounded transition"
                title="Edit Transaction"
              >
                <Pencil className="w-3.5 h-3.5" />
              </button>
            )}
            <button
              onClick={() => {
                if (confirm('Delete this transaction and all its splits?')) {
                  onDeleteTransaction(entry.transaction_id);
                }
              }}
              className="p-1 text-slate-300 hover:text-rose-600 hover:bg-rose-50 rounded transition"
              title="Delete Transaction"
            >
              <Trash2 className="w-3.5 h-3.5" />
            </button>
          </div>
        </td>
      </tr>

      {/* Second Row with Details / Memo / Notes text visible */}
      <tr
        className={`border-b border-slate-200 text-slate-500 text-[11px] transition-colors ${
          isExpanded ? 'bg-emerald-50/20' : 'bg-slate-50/50 hover:bg-amber-50/20'
        }`}
      >
        <td className="py-1 px-3 text-slate-400 font-mono text-[10px]"></td>
        <td className="py-1 px-2 text-slate-400 text-[10px]"></td>
        <td colSpan={6} className="py-1 px-3">
          <div className="flex flex-wrap items-center gap-x-4 gap-y-1 text-slate-600">
            <div className="flex items-center space-x-1.5 min-w-0">
              <span className="font-semibold text-slate-400 uppercase text-[9px] tracking-wider shrink-0">
                Notes:
              </span>
              <span className={entry.notes ? 'text-slate-700 font-medium truncate' : 'text-slate-400 italic'}>
                {entry.notes || '—'}
              </span>
            </div>
            <div className="flex items-center space-x-1.5 min-w-0">
              <span className="font-semibold text-slate-400 uppercase text-[9px] tracking-wider shrink-0">
                Memo:
              </span>
              <span className={entry.memo || (entry.all_splits.find(s => s.memo)?.memo) ? 'text-slate-700 font-medium truncate' : 'text-slate-400 italic'}>
                {entry.memo || (entry.all_splits.find(s => s.memo)?.memo) || '—'}
              </span>
            </div>
          </div>
        </td>
        <td className="py-1 px-2 text-center"></td>
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
                          {debitVal !== null ? formatCurrency(debitVal, currencyToUse) : ''}
                        </td>
                        <td className="py-1 text-right font-mono">
                          {creditVal !== null ? formatCurrency(creditVal, currencyToUse) : ''}
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
};
