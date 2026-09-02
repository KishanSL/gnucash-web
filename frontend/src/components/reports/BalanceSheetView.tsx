import React, { useState, useEffect } from 'react';
import { Scale, CheckCircle2, AlertCircle, Calendar } from 'lucide-react';
import { BalanceSheetReport } from '../../types';
import { fetchBalanceSheet, formatCurrency } from '../../api';

interface BalanceSheetViewProps {
  currentCurrency: string;
}

export const BalanceSheetView: React.FC<BalanceSheetViewProps> = ({ currentCurrency }) => {
  const [date, setDate] = useState(new Date().toISOString().split('T')[0]);
  const [report, setReport] = useState<BalanceSheetReport | null>(null);
  const [loading, setLoading] = useState(false);

  const loadReport = async (asOfDate: string) => {
    try {
      setLoading(true);
      const data = await fetchBalanceSheet(asOfDate);
      setReport(data);
    } catch (err) {
      console.error(err);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    loadReport(date);
  }, [date]);

  return (
    <div className="space-y-6">
      {/* Report Controls Header */}
      <div className="bg-white rounded-xl border border-slate-200 p-5 shadow-sm flex flex-col sm:flex-row items-center justify-between gap-4">
        <div className="flex items-center space-x-3">
          <div className="w-10 h-10 rounded-lg bg-emerald-50 border border-emerald-200 flex items-center justify-center text-emerald-600">
            <Scale className="w-5 h-5" />
          </div>
          <div>
            <h2 className="text-lg font-bold text-slate-900">Balance Sheet</h2>
            <p className="text-xs text-slate-500">
              Financial position statement as of selected date (Assets = Liabilities + Equity)
            </p>
          </div>
        </div>

        <div className="flex items-center space-x-2">
          <label className="text-xs font-semibold text-slate-500">As of Date:</label>
          <div className="relative">
            <Calendar className="w-3.5 h-3.5 text-slate-400 absolute left-2.5 top-1/2 -translate-y-1/2" />
            <input
              type="date"
              value={date}
              onChange={(e) => setDate(e.target.value)}
              className="pl-8 pr-3 py-1.5 text-xs bg-slate-50 border border-slate-300 rounded-lg font-mono focus:ring-1 focus:ring-emerald-500"
            />
          </div>
        </div>
      </div>

      {loading && (
        <div className="py-16 text-center text-slate-400 text-xs">Loading Balance Sheet...</div>
      )}

      {report && (
        <div className="space-y-6">
          {/* Balanced Status Banner */}
          <div
            className={`p-4 rounded-xl border flex items-center justify-between text-xs font-medium ${
              report.is_balanced
                ? 'bg-emerald-50 border-emerald-200 text-emerald-800'
                : 'bg-rose-50 border-rose-200 text-rose-800'
            }`}
          >
            <div className="flex items-center space-x-2">
              {report.is_balanced ? (
                <CheckCircle2 className="w-5 h-5 text-emerald-600 shrink-0" />
              ) : (
                <AlertCircle className="w-5 h-5 text-rose-600 shrink-0" />
              )}
              <span>
                {report.is_balanced
                  ? 'Double-Entry Equation Balanced: Assets = Liabilities + Equity'
                  : 'Imbalance detected in ledger equation! Check unassigned splits.'}
              </span>
            </div>

            <div className="font-mono font-bold flex items-center space-x-4">
              <span>Assets: {formatCurrency(report.assets.total, currentCurrency)}</span>
              <span>=</span>
              <span>L+E: {formatCurrency(report.total_liabilities_and_equity, currentCurrency)}</span>
            </div>
          </div>

          {/* Balance Sheet Tables Grid */}
          <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
            {/* Left: Assets */}
            <div className="bg-white rounded-xl border border-slate-200 shadow-sm overflow-hidden flex flex-col justify-between">
              <div>
                <div className="p-3.5 bg-sky-50/70 border-b border-sky-100 flex items-center justify-between">
                  <h3 className="font-bold text-sky-900 text-xs uppercase tracking-wider">Assets</h3>
                  <span className="font-mono font-bold text-sky-800 text-xs">
                    {formatCurrency(report.assets.total, currentCurrency)}
                  </span>
                </div>

                <div className="p-4 space-y-1">
                  {report.assets.items.length === 0 ? (
                    <div className="py-8 text-center text-xs text-slate-400">No assets found.</div>
                  ) : (
                    report.assets.items.map((item) => (
                      <div
                        key={item.account_id}
                        className="flex items-center justify-between py-1 text-xs"
                        style={{ paddingLeft: `${item.depth * 16}px` }}
                      >
                        <span className="text-slate-700">
                          {item.code ? `${item.code} ` : ''}
                          {item.name}
                        </span>
                        <span className="font-mono text-slate-900">
                          {formatCurrency(item.balance, currentCurrency)}
                        </span>
                      </div>
                    ))
                  )}
                </div>
              </div>

              <div className="p-3.5 bg-slate-50 border-t border-slate-200 flex justify-between text-xs font-bold">
                <span className="text-slate-700">Total Assets</span>
                <span className="font-mono text-sky-800">{formatCurrency(report.assets.total, currentCurrency)}</span>
              </div>
            </div>

            {/* Right: Liabilities & Equity */}
            <div className="space-y-6">
              {/* Liabilities */}
              <div className="bg-white rounded-xl border border-slate-200 shadow-sm overflow-hidden">
                <div className="p-3.5 bg-amber-50/70 border-b border-amber-100 flex items-center justify-between">
                  <h3 className="font-bold text-amber-900 text-xs uppercase tracking-wider">
                    Liabilities
                  </h3>
                  <span className="font-mono font-bold text-amber-800 text-xs">
                    {formatCurrency(report.liabilities.total, currentCurrency)}
                  </span>
                </div>

                <div className="p-4 space-y-1">
                  {report.liabilities.items.length === 0 ? (
                    <div className="py-6 text-center text-xs text-slate-400">No liabilities found.</div>
                  ) : (
                    report.liabilities.items.map((item) => (
                      <div
                        key={item.account_id}
                        className="flex items-center justify-between py-1 text-xs"
                        style={{ paddingLeft: `${item.depth * 16}px` }}
                      >
                        <span className="text-slate-700">
                          {item.code ? `${item.code} ` : ''}
                          {item.name}
                        </span>
                        <span className="font-mono text-slate-900">
                          {formatCurrency(item.balance, currentCurrency)}
                        </span>
                      </div>
                    ))
                  )}
                </div>

                <div className="p-3.5 bg-slate-50 border-t border-slate-200 flex justify-between text-xs font-bold">
                  <span className="text-slate-700">Total Liabilities</span>
                  <span className="font-mono text-amber-800">
                    {formatCurrency(report.liabilities.total, currentCurrency)}
                  </span>
                </div>
              </div>

              {/* Equity */}
              <div className="bg-white rounded-xl border border-slate-200 shadow-sm overflow-hidden">
                <div className="p-3.5 bg-purple-50/70 border-b border-purple-100 flex items-center justify-between">
                  <h3 className="font-bold text-purple-900 text-xs uppercase tracking-wider">
                    Equity
                  </h3>
                  <span className="font-mono font-bold text-purple-800 text-xs">
                    {formatCurrency(report.equity.total, currentCurrency)}
                  </span>
                </div>

                <div className="p-4 space-y-1">
                  {report.equity.items.map((item) => (
                    <div
                      key={item.account_id}
                      className="flex items-center justify-between py-1 text-xs"
                      style={{ paddingLeft: `${item.depth * 16}px` }}
                    >
                      <span className="text-slate-700">{item.name}</span>
                      <span className="font-mono text-slate-900">
                        {formatCurrency(item.balance, currentCurrency)}
                      </span>
                    </div>
                  ))}

                  {/* Net Income line from P&L */}
                  <div className="flex items-center justify-between py-1 text-xs font-semibold text-emerald-700 border-t border-slate-100 pt-2">
                    <span>Retained Earnings / Net Income</span>
                    <span className="font-mono">{formatCurrency(report.net_income, currentCurrency)}</span>
                  </div>
                </div>

                <div className="p-3.5 bg-slate-50 border-t border-slate-200 flex justify-between text-xs font-bold">
                  <span className="text-slate-700">Total Equity</span>
                  <span className="font-mono text-purple-800">
                    {formatCurrency(report.equity.total, currentCurrency)}
                  </span>
                </div>
              </div>

              {/* Combined Total */}
              <div className="p-4 bg-slate-900 text-white rounded-xl flex justify-between items-center text-xs font-bold shadow-md">
                <span>Total Liabilities & Equity</span>
                <span className="font-mono text-sm">
                  {formatCurrency(report.total_liabilities_and_equity, currentCurrency)}
                </span>
              </div>
            </div>
          </div>
        </div>
      )}
    </div>
  );
};
