import React, { useState, useEffect } from 'react';
import {
  CheckCircle2,
  ArrowDownLeft,
  ArrowUpRight,
  Sparkles,
  Calendar,
} from 'lucide-react';
import { Account, ReconcileData } from '../types';
import { fetchReconcileInfo, finishReconciliation, formatCurrency } from '../api';

interface ReconcileModalProps {
  initialAccount?: Account;
  allAccounts: Account[];
  currentCurrency: string;
  onReconciledSuccess: () => void;
}

export const ReconcileModal: React.FC<ReconcileModalProps> = ({
  initialAccount,
  allAccounts,
  currentCurrency,
  onReconciledSuccess,
}) => {
  const reconcilableAccounts = allAccounts.filter(
    (a) => !a.placeholder && ['bank', 'credit', 'cash', 'liability'].includes(a.account_type.toLowerCase())
  );

  const [selectedAccountId, setSelectedAccountId] = useState<string>(
    initialAccount?.id || reconcilableAccounts[0]?.id || ''
  );

  const [statementDate, setStatementDate] = useState(
    new Date().toISOString().split('T')[0]
  );
  const [statementEndingBalance, setStatementEndingBalance] = useState('');
  const [reconcileData, setReconcileData] = useState<ReconcileData | null>(null);
  const [selectedSplitIds, setSelectedSplitIds] = useState<Set<string>>(new Set());
  const [mobileTab, setMobileTab] = useState<'deposits' | 'withdrawals'>('deposits');
  const [submitting, setSubmitting] = useState(false);
  const [message, setMessage] = useState<string | null>(null);

  // Load reconcile info
  const loadData = async (accountId: string, date: string) => {
    if (!accountId) return;
    try {
      setMessage(null);
      const data = await fetchReconcileInfo(accountId, date);
      setReconcileData(data);
      // Pre-set statement ending balance to current ending balance if empty
      setStatementEndingBalance(data.ending_balance);

      // Pre-select splits that were already cleared ('c')
      const preSelected = new Set<string>();
      data.deposits.forEach((d) => {
        if (d.reconciled_state === 'c') preSelected.add(d.split_id);
      });
      data.withdrawals.forEach((w) => {
        if (w.reconciled_state === 'c') preSelected.add(w.split_id);
      });
      setSelectedSplitIds(preSelected);
    } catch (err: any) {
      console.error(err);
    }
  };

  useEffect(() => {
    if (selectedAccountId) {
      loadData(selectedAccountId, statementDate);
    }
  }, [selectedAccountId, statementDate]);

  const toggleSplit = (splitId: string) => {
    setSelectedSplitIds((prev) => {
      const next = new Set(prev);
      if (next.has(splitId)) {
        next.delete(splitId);
      } else {
        next.add(splitId);
      }
      return next;
    });
  };

  const toggleAllDeposits = () => {
    if (!reconcileData) return;
    const allDepositIds = reconcileData.deposits.map((d) => d.split_id);
    const allSelected = allDepositIds.every((id) => selectedSplitIds.has(id));

    setSelectedSplitIds((prev) => {
      const next = new Set(prev);
      if (allSelected) {
        allDepositIds.forEach((id) => next.delete(id));
      } else {
        allDepositIds.forEach((id) => next.add(id));
      }
      return next;
    });
  };

  const toggleAllWithdrawals = () => {
    if (!reconcileData) return;
    const allWithdrawalIds = reconcileData.withdrawals.map((w) => w.split_id);
    const allSelected = allWithdrawalIds.every((id) => selectedSplitIds.has(id));

    setSelectedSplitIds((prev) => {
      const next = new Set(prev);
      if (allSelected) {
        allWithdrawalIds.forEach((id) => next.delete(id));
      } else {
        allWithdrawalIds.forEach((id) => next.add(id));
      }
      return next;
    });
  };

  // Calculations
  const startingBalanceNum = parseFloat(reconcileData?.starting_balance || '0');
  const targetEndingNum = parseFloat(statementEndingBalance || '0');

  let clearedDepositsTotal = 0;
  let clearedWithdrawalsTotal = 0;

  if (reconcileData) {
    for (const d of reconcileData.deposits) {
      if (selectedSplitIds.has(d.split_id)) {
        clearedDepositsTotal += parseFloat(d.display_amount) || 0;
      }
    }
    for (const w of reconcileData.withdrawals) {
      if (selectedSplitIds.has(w.split_id)) {
        clearedWithdrawalsTotal += parseFloat(w.display_amount) || 0;
      }
    }
  }

  // Account normal balance check
  const isAsset = reconcileData?.account
    ? ['bank', 'cash', 'asset', 'receivable'].includes(reconcileData.account.account_type.toLowerCase())
    : true;

  const clearedBalance = isAsset
    ? startingBalanceNum + clearedDepositsTotal - clearedWithdrawalsTotal
    : startingBalanceNum + clearedWithdrawalsTotal - clearedDepositsTotal;

  const difference = Math.round((targetEndingNum - clearedBalance) * 100) / 100;
  const isReconciled = Math.abs(difference) < 0.001;

  const handleFinish = async () => {
    if (!selectedAccountId || !isReconciled) return;

    try {
      setSubmitting(true);
      const res = await finishReconciliation(selectedAccountId, {
        statement_date: statementDate,
        reconciled_split_ids: Array.from(selectedSplitIds),
      });
      setMessage(res.message);
      onReconciledSuccess();
      await loadData(selectedAccountId, statementDate);
    } catch (err: any) {
      alert(err.message || 'Failed to finish reconciliation');
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <div className="space-y-6">
      {/* Header Banner */}
      <div className="bg-white rounded-xl border border-slate-200 p-5 shadow-sm">
        <div className="flex flex-col lg:flex-row lg:items-center justify-between gap-4">
          <div className="flex items-center space-x-3">
            <div className="w-10 h-10 rounded-lg bg-emerald-50 border border-emerald-200 flex items-center justify-center text-emerald-600">
              <CheckCircle2 className="w-5 h-5" />
            </div>
            <div>
              <h2 className="text-lg font-bold text-slate-900">Account Reconciliation</h2>
              <p className="text-xs text-slate-500">
                Match transactions against your official bank or credit card statement
              </p>
            </div>
          </div>

          {/* Statement Parameters */}
          <div className="grid grid-cols-1 sm:grid-cols-3 gap-3">
            {/* Account Selector */}
            <div>
              <label className="block text-[11px] font-semibold text-slate-500 uppercase tracking-wider mb-1">
                Account to Reconcile
              </label>
              <select
                value={selectedAccountId}
                onChange={(e) => setSelectedAccountId(e.target.value)}
                className="w-full py-1.5 px-2.5 text-xs bg-slate-50 border border-slate-300 rounded-lg font-semibold focus:outline-none focus:ring-2 focus:ring-emerald-500/20 focus:border-emerald-500"
              >
                {reconcilableAccounts.map((a) => (
                  <option key={a.id} value={a.id}>
                    {a.name} ({a.account_type})
                  </option>
                ))}
              </select>
            </div>

            {/* Statement Date */}
            <div>
              <label className="block text-[11px] font-semibold text-slate-500 uppercase tracking-wider mb-1">
                Statement Date
              </label>
              <div className="relative">
                <Calendar className="w-3.5 h-3.5 text-slate-400 absolute left-2.5 top-1/2 -translate-y-1/2" />
                <input
                  type="date"
                  value={statementDate}
                  onChange={(e) => setStatementDate(e.target.value)}
                  className="w-full pl-8 pr-2.5 py-1.5 text-xs bg-slate-50 border border-slate-300 rounded-lg font-mono focus:outline-none focus:ring-2 focus:ring-emerald-500/20 focus:border-emerald-500"
                />
              </div>
            </div>

            {/* Statement Ending Balance */}
            <div>
              <label className="block text-[11px] font-semibold text-slate-500 uppercase tracking-wider mb-1">
                Statement Ending Balance ($)
              </label>
              <input
                type="number"
                step="0.01"
                placeholder="0.00"
                value={statementEndingBalance}
                onChange={(e) => setStatementEndingBalance(e.target.value)}
                className="w-full px-2.5 py-1.5 text-xs bg-slate-50 border border-slate-300 rounded-lg font-mono font-bold text-slate-800 focus:outline-none focus:ring-2 focus:ring-emerald-500/20 focus:border-emerald-500"
              />
            </div>
          </div>
        </div>
      </div>

      {message && (
        <div className="p-3 bg-emerald-50 border border-emerald-200 text-emerald-800 rounded-xl text-xs flex items-center space-x-2 font-medium">
          <Sparkles className="w-4 h-4 text-emerald-600" />
          <span>{message}</span>
        </div>
      )}

      {/* Mobile Tab Switcher for Funds In vs Funds Out */}
      <div className="lg:hidden flex space-x-2 border-b border-slate-200 pb-2">
        <button
          onClick={() => setMobileTab('deposits')}
          className={`flex-1 flex items-center justify-center space-x-1.5 py-2 px-3 rounded-lg text-xs font-bold transition ${
            mobileTab === 'deposits'
              ? 'bg-emerald-600 text-white shadow-sm'
              : 'bg-slate-100 text-slate-600 hover:bg-slate-200'
          }`}
        >
          <ArrowDownLeft className="w-3.5 h-3.5" />
          <span>Funds In ({reconcileData?.deposits.length || 0})</span>
        </button>

        <button
          onClick={() => setMobileTab('withdrawals')}
          className={`flex-1 flex items-center justify-center space-x-1.5 py-2 px-3 rounded-lg text-xs font-bold transition ${
            mobileTab === 'withdrawals'
              ? 'bg-rose-600 text-white shadow-sm'
              : 'bg-slate-100 text-slate-600 hover:bg-slate-200'
          }`}
        >
          <ArrowUpRight className="w-3.5 h-3.5" />
          <span>Funds Out ({reconcileData?.withdrawals.length || 0})</span>
        </button>
      </div>

      {/* Two-Column Reconcile Checkoff Grid (GnuCash Style) */}
      <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
        {/* Column 1: Funds In / Deposits */}
        <div className={`${mobileTab === 'deposits' ? 'flex' : 'hidden lg:flex'} bg-white rounded-xl border border-slate-200 shadow-sm overflow-hidden flex-col`}>
          <div className="p-3.5 border-b border-slate-200 bg-emerald-50/50 flex items-center justify-between">
            <div className="flex items-center space-x-2">
              <ArrowDownLeft className="w-4 h-4 text-emerald-600" />
              <h3 className="font-bold text-slate-800 text-xs uppercase tracking-wider">
                Funds In (Deposits / Credits)
              </h3>
            </div>
            <button
              onClick={toggleAllDeposits}
              className="text-[11px] font-semibold text-emerald-700 hover:underline"
            >
              Select / Clear All
            </button>
          </div>

          <div className="overflow-y-auto max-h-96 divide-y divide-slate-100 flex-1">
            {!reconcileData || reconcileData.deposits.length === 0 ? (
              <div className="py-12 text-center text-xs text-slate-400">
                No uncleared deposits for this period.
              </div>
            ) : (
              reconcileData.deposits.map((item) => {
                const checked = selectedSplitIds.has(item.split_id);
                return (
                  <div
                    key={item.split_id}
                    onClick={() => toggleSplit(item.split_id)}
                    className={`flex items-center justify-between p-3 text-xs cursor-pointer transition ${
                      checked ? 'bg-emerald-50/60' : 'hover:bg-slate-50'
                    }`}
                  >
                    <div className="flex items-center space-x-3">
                      <input
                        type="checkbox"
                        checked={checked}
                        onChange={() => {}} // handled by parent onClick
                        className="rounded border-slate-300 text-emerald-600 focus:ring-emerald-500 w-4 h-4"
                      />
                      <div>
                        <span className="font-medium text-slate-800 block">
                          {item.description}
                        </span>
                        <span className="text-[11px] text-slate-400 font-mono">
                          {item.post_date} {item.num ? `• #${item.num}` : ''}
                        </span>
                      </div>
                    </div>
                    <span className="font-mono font-bold text-emerald-700 text-right">
                      +{formatCurrency(item.display_amount, currentCurrency)}
                    </span>
                  </div>
                );
              })
            )}
          </div>

          <div className="p-3 bg-slate-50 border-t border-slate-200 flex justify-between text-xs font-semibold">
            <span className="text-slate-500">Cleared Deposits:</span>
            <span className="font-mono text-emerald-700">{formatCurrency(clearedDepositsTotal, currentCurrency)}</span>
          </div>
        </div>

        {/* Column 2: Funds Out / Payments */}
        <div className={`${mobileTab === 'withdrawals' ? 'flex' : 'hidden lg:flex'} bg-white rounded-xl border border-slate-200 shadow-sm overflow-hidden flex-col`}>
          <div className="p-3.5 border-b border-slate-200 bg-rose-50/50 flex items-center justify-between">
            <div className="flex items-center space-x-2">
              <ArrowUpRight className="w-4 h-4 text-rose-600" />
              <h3 className="font-bold text-slate-800 text-xs uppercase tracking-wider">
                Funds Out (Payments / Charges)
              </h3>
            </div>
            <button
              onClick={toggleAllWithdrawals}
              className="text-[11px] font-semibold text-rose-700 hover:underline"
            >
              Select / Clear All
            </button>
          </div>

          <div className="overflow-y-auto max-h-96 divide-y divide-slate-100 flex-1">
            {!reconcileData || reconcileData.withdrawals.length === 0 ? (
              <div className="py-12 text-center text-xs text-slate-400">
                No uncleared payments for this period.
              </div>
            ) : (
              reconcileData.withdrawals.map((item) => {
                const checked = selectedSplitIds.has(item.split_id);
                return (
                  <div
                    key={item.split_id}
                    onClick={() => toggleSplit(item.split_id)}
                    className={`flex items-center justify-between p-3 text-xs cursor-pointer transition ${
                      checked ? 'bg-rose-50/60' : 'hover:bg-slate-50'
                    }`}
                  >
                    <div className="flex items-center space-x-3">
                      <input
                        type="checkbox"
                        checked={checked}
                        onChange={() => {}}
                        className="rounded border-slate-300 text-emerald-600 focus:ring-emerald-500 w-4 h-4"
                      />
                      <div>
                        <span className="font-medium text-slate-800 block">
                          {item.description}
                        </span>
                        <span className="text-[11px] text-slate-400 font-mono">
                          {item.post_date} {item.num ? `• #${item.num}` : ''}
                        </span>
                      </div>
                    </div>
                    <span className="font-mono font-bold text-rose-700 text-right">
                      -{formatCurrency(item.display_amount, currentCurrency)}
                    </span>
                  </div>
                );
              })
            )}
          </div>

          <div className="p-3 bg-slate-50 border-t border-slate-200 flex justify-between text-xs font-semibold">
            <span className="text-slate-500">Cleared Payments:</span>
            <span className="font-mono text-rose-700">{formatCurrency(clearedWithdrawalsTotal, currentCurrency)}</span>
          </div>
        </div>
      </div>

      {/* Reconciliation Balance Summary & Finish Bar */}
      <div className="bg-white rounded-xl border border-slate-200 p-4 sm:p-5 shadow-sm">
        <div className="grid grid-cols-2 sm:grid-cols-5 gap-3 sm:gap-4 items-center">
          <div>
            <span className="text-[10px] font-bold text-slate-400 uppercase tracking-wider block">
              Starting Balance
            </span>
            <span className="text-base font-bold font-mono text-slate-800">
              {formatCurrency(startingBalanceNum, currentCurrency)}
            </span>
          </div>

          <div>
            <span className="text-[10px] font-bold text-slate-400 uppercase tracking-wider block">
              Cleared Balance
            </span>
            <span className="text-base font-bold font-mono text-slate-800">
              {formatCurrency(clearedBalance, currentCurrency)}
            </span>
          </div>

          <div>
            <span className="text-[10px] font-bold text-slate-400 uppercase tracking-wider block">
              Target Ending Balance
            </span>
            <span className="text-base font-bold font-mono text-slate-800">
              {formatCurrency(targetEndingNum, currentCurrency)}
            </span>
          </div>

          <div>
            <span className="text-[10px] font-bold text-slate-400 uppercase tracking-wider block">
              Difference
            </span>
            <span
              className={`text-xl font-bold font-mono ${
                isReconciled ? 'text-emerald-600' : 'text-rose-600'
              }`}
            >
              {formatCurrency(difference, currentCurrency)}
            </span>
          </div>

          <div className="col-span-2 sm:col-span-1 flex justify-end">
            <button
              onClick={handleFinish}
              disabled={!isReconciled || submitting}
              className={`w-full sm:w-auto px-5 py-2.5 rounded-lg font-bold text-xs shadow-md transition active:scale-95 flex items-center justify-center space-x-2 ${
                isReconciled
                  ? 'bg-emerald-600 hover:bg-emerald-500 text-white cursor-pointer shadow-emerald-700/30'
                  : 'bg-slate-200 text-slate-400 cursor-not-allowed'
              }`}
            >
              <CheckCircle2 className="w-4 h-4" />
              <span>{submitting ? 'Reconciling...' : 'Finish Reconcile'}</span>
            </button>
          </div>
        </div>
      </div>
    </div>
  );
};
