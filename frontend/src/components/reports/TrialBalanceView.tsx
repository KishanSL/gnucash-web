import React, { useState, useEffect } from 'react';
import { BookCheck, CheckCircle2, AlertCircle, Calendar } from 'lucide-react';
import { TrialBalanceReport } from '../../types';
import { fetchTrialBalance, formatCurrency } from '../../api';

interface TrialBalanceViewProps {
  currentCurrency?: string;
}

export const TrialBalanceView: React.FC<TrialBalanceViewProps> = ({ currentCurrency = 'INR' }) => {
  const [date, setDate] = useState(new Date().toISOString().split('T')[0]);
  const [report, setReport] = useState<TrialBalanceReport | null>(null);
  const [loading, setLoading] = useState(false);

  const loadReport = async (asOfDate: string) => {
    try {
      setLoading(true);
      const data = await fetchTrialBalance(asOfDate);
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
      {/* Header Bar */}
      <div className="bg-white rounded-xl border border-slate-200 p-5 shadow-sm flex flex-col sm:flex-row items-center justify-between gap-4">
        <div className="flex items-center space-x-3">
          <div className="w-10 h-10 rounded-lg bg-emerald-50 border border-emerald-200 flex items-center justify-center text-emerald-600">
            <BookCheck className="w-5 h-5" />
          </div>
          <div>
            <h2 className="text-lg font-bold text-slate-900">Trial Balance</h2>
            <p className="text-xs text-slate-500">
              Verifies that total debits equal total credits across all general ledger accounts
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
        <div className="py-16 text-center text-slate-400 text-xs">Loading Trial Balance...</div>
      )}

      {report && (
        <div className="space-y-6">
          {/* Balanced Status Banner */}
          <div
            className={`p-3.5 sm:p-4 rounded-xl border flex flex-col sm:flex-row sm:items-center justify-between gap-3 text-xs font-medium ${
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
                  ? 'Double-entry books are strictly balanced: Total Debits equal Total Credits!'
                  : 'Ledger out of balance! An unbalanced transaction or missing split was recorded.'}
              </span>
            </div>

            <div className="font-mono font-bold flex items-center justify-between sm:justify-end space-x-2 sm:space-x-4 pt-1 sm:pt-0 border-t sm:border-t-0 border-emerald-200/50">
              <span>Debits: {formatCurrency(report.total_debit, currentCurrency)}</span>
              <span>=</span>
              <span>Credits: {formatCurrency(report.total_credit, currentCurrency)}</span>
            </div>
          </div>

          {/* Table */}
          <div className="bg-white rounded-xl border border-slate-200 shadow-sm overflow-hidden">
            <div className="overflow-x-auto">
              <table className="w-full text-left border-collapse text-xs">
                <thead>
                  <tr className="bg-slate-100 text-slate-700 font-bold border-b border-slate-200">
                    <th className="py-3 px-4 w-28">Code</th>
                    <th className="py-3 px-4">Account Name</th>
                    <th className="py-3 px-4 w-32">Type</th>
                    <th className="py-3 px-4 w-36 text-right">Debit Balance</th>
                    <th className="py-3 px-4 w-36 text-right">Credit Balance</th>
                  </tr>
                </thead>
                <tbody className="divide-y divide-slate-100">
                  {report.items.length === 0 ? (
                    <tr>
                      <td colSpan={5} className="py-12 text-center text-slate-400">
                        No active balances recorded.
                      </td>
                    </tr>
                  ) : (
                    report.items.map((item) => (
                      <tr key={item.account_id} className="hover:bg-slate-50 transition">
                        <td className="py-2.5 px-4 font-mono text-slate-500">
                          {item.code || '—'}
                        </td>
                        <td className="py-2.5 px-4 font-medium text-slate-800">{item.name}</td>
                        <td className="py-2.5 px-4 capitalize text-slate-500">
                          {item.account_type}
                        </td>
                        <td className="py-2.5 px-4 text-right font-mono font-semibold text-emerald-700">
                          {parseFloat(item.debit) > 0 ? formatCurrency(item.debit, currentCurrency) : ''}
                        </td>
                        <td className="py-2.5 px-4 text-right font-mono font-semibold text-rose-700">
                          {parseFloat(item.credit) > 0 ? formatCurrency(item.credit, currentCurrency) : ''}
                        </td>
                      </tr>
                    ))
                  )}
                </tbody>
                <tfoot>
                  <tr className="bg-slate-900 text-white font-bold font-mono text-xs border-t-2 border-slate-800">
                    <td colSpan={3} className="py-3 px-4 text-right font-sans font-bold">
                      Grand Totals:
                    </td>
                    <td className="py-3 px-4 text-right text-emerald-300">
                      {formatCurrency(report.total_debit, currentCurrency)}
                    </td>
                    <td className="py-3 px-4 text-right text-rose-300">
                      {formatCurrency(report.total_credit, currentCurrency)}
                    </td>
                  </tr>
                </tfoot>
              </table>
            </div>
          </div>
        </div>
      )}
    </div>
  );
};
