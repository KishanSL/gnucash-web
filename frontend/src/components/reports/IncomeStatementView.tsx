import React, { useState, useEffect } from 'react';
import { TrendingUp, ArrowUpRight, ArrowDownRight } from 'lucide-react';
import { IncomeStatementReport } from '../../types';
import { fetchIncomeStatement, formatCurrency } from '../../api';

interface IncomeStatementViewProps {
  currentCurrency?: string;
}

export const IncomeStatementView: React.FC<IncomeStatementViewProps> = ({ currentCurrency = 'INR' }) => {
  const now = new Date();
  const startOfYear = `${now.getFullYear()}-01-01`;
  const today = now.toISOString().split('T')[0];

  const [fromDate, setFromDate] = useState(startOfYear);
  const [toDate, setToDate] = useState(today);
  const [report, setReport] = useState<IncomeStatementReport | null>(null);
  const [loading, setLoading] = useState(false);

  const loadReport = async (from: string, to: string) => {
    try {
      setLoading(true);
      const data = await fetchIncomeStatement(from, to);
      setReport(data);
    } catch (err) {
      console.error(err);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    loadReport(fromDate, toDate);
  }, [fromDate, toDate]);

  const netIncomeNum = parseFloat(report?.net_income || '0');
  const isProfitable = netIncomeNum >= 0;

  return (
    <div className="space-y-6">
      {/* Header Bar */}
      <div className="bg-white rounded-xl border border-slate-200 p-4 sm:p-5 shadow-sm flex flex-col sm:flex-row items-start sm:items-center justify-between gap-3 sm:gap-4">
        <div className="flex items-center space-x-3">
          <div className="w-10 h-10 rounded-lg bg-emerald-50 border border-emerald-200 flex items-center justify-center text-emerald-600 shrink-0">
            <TrendingUp className="w-5 h-5" />
          </div>
          <div>
            <h2 className="text-base sm:text-lg font-bold text-slate-900">Income Statement (P&L)</h2>
            <p className="text-[11px] sm:text-xs text-slate-500">
              Profit and Loss summary over selected date range (Revenues minus Expenses)
            </p>
          </div>
        </div>

        {/* Date Range Selectors */}
        <div className="flex items-center space-x-2 flex-wrap gap-y-1.5 w-full sm:w-auto">
          <div className="flex items-center space-x-1.5">
            <span className="text-xs text-slate-400 font-semibold">From:</span>
            <input
              type="date"
              value={fromDate}
              onChange={(e) => setFromDate(e.target.value)}
              className="px-2 py-1.5 text-xs bg-slate-50 border border-slate-300 rounded-lg font-mono focus:ring-1 focus:ring-emerald-500"
            />
          </div>
          <div className="flex items-center space-x-1.5">
            <span className="text-xs text-slate-400 font-semibold">To:</span>
            <input
              type="date"
              value={toDate}
              onChange={(e) => setToDate(e.target.value)}
              className="px-2 py-1.5 text-xs bg-slate-50 border border-slate-300 rounded-lg font-mono focus:ring-1 focus:ring-emerald-500"
            />
          </div>
        </div>
      </div>

      {loading && (
        <div className="py-16 text-center text-slate-400 text-xs">Loading Income Statement...</div>
      )}

      {report && (
        <div className="space-y-6">
          {/* Net Income Summary Card */}
          <div className="bg-white p-5 rounded-xl border border-slate-200 shadow-sm flex items-center justify-between">
            <div>
              <p className="text-xs font-semibold text-slate-500 uppercase tracking-wider">
                Net Profit / Net Income
              </p>
              <h3
                className={`text-2xl font-bold font-mono mt-1 ${
                  isProfitable ? 'text-emerald-600' : 'text-rose-600'
                }`}
              >
                {formatCurrency(report.net_income, currentCurrency)}
              </h3>
              <p className="text-xs text-slate-400 mt-0.5">
                {isProfitable
                  ? 'Revenues exceed operating expenses for this period'
                  : 'Net operating loss for this period'}
              </p>
            </div>

            <div
              className={`w-12 h-12 rounded-xl flex items-center justify-center ${
                isProfitable ? 'bg-emerald-50 text-emerald-600' : 'bg-rose-50 text-rose-600'
              }`}
            >
              {isProfitable ? (
                <ArrowUpRight className="w-6 h-6" />
              ) : (
                <ArrowDownRight className="w-6 h-6" />
              )}
            </div>
          </div>

          {/* Revenues & Expenses Tables */}
          <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
            {/* Revenues */}
            <div className="bg-white rounded-xl border border-slate-200 shadow-sm overflow-hidden flex flex-col justify-between">
              <div>
                <div className="p-3.5 bg-emerald-50/70 border-b border-emerald-100 flex items-center justify-between">
                  <h3 className="font-bold text-emerald-900 text-xs uppercase tracking-wider">
                    Revenues & Income
                  </h3>
                  <span className="font-mono font-bold text-emerald-800 text-xs">
                    {formatCurrency(report.revenues.total, currentCurrency)}
                  </span>
                </div>

                <div className="p-4 space-y-1">
                  {report.revenues.items.length === 0 ? (
                    <div className="py-8 text-center text-xs text-slate-400">
                      No revenue transactions in this period.
                    </div>
                  ) : (
                    report.revenues.items.map((item) => (
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
                          {formatCurrency(item.amount, currentCurrency)}
                        </span>
                      </div>
                    ))
                  )}
                </div>
              </div>

              <div className="p-3.5 bg-slate-50 border-t border-slate-200 flex justify-between text-xs font-bold">
                <span className="text-slate-700">Total Revenue</span>
                <span className="font-mono text-emerald-800">
                  {formatCurrency(report.revenues.total, currentCurrency)}
                </span>
              </div>
            </div>

            {/* Expenses */}
            <div className="bg-white rounded-xl border border-slate-200 shadow-sm overflow-hidden flex flex-col justify-between">
              <div>
                <div className="p-3.5 bg-rose-50/70 border-b border-rose-100 flex items-center justify-between">
                  <h3 className="font-bold text-rose-900 text-xs uppercase tracking-wider">
                    Operating Expenses
                  </h3>
                  <span className="font-mono font-bold text-rose-800 text-xs">
                    {formatCurrency(report.expenses.total, currentCurrency)}
                  </span>
                </div>

                <div className="p-4 space-y-1">
                  {report.expenses.items.length === 0 ? (
                    <div className="py-8 text-center text-xs text-slate-400">
                      No expense transactions in this period.
                    </div>
                  ) : (
                    report.expenses.items.map((item) => (
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
                          {formatCurrency(item.amount, currentCurrency)}
                        </span>
                      </div>
                    ))
                  )}
                </div>
              </div>

              <div className="p-3.5 bg-slate-50 border-t border-slate-200 flex justify-between text-xs font-bold">
                <span className="text-slate-700">Total Expenses</span>
                <span className="font-mono text-rose-800">
                  {formatCurrency(report.expenses.total, currentCurrency)}
                </span>
              </div>
            </div>
          </div>
        </div>
      )}
    </div>
  );
};
