import React from 'react';
import { Landmark, CreditCard, Scale } from 'lucide-react';
import { formatCurrency } from '../../api';

interface AccountSummaryCardsProps {
  netWorth: number;
  assetsTotal: number;
  liabTotal: number;
  equityTotal: number;
  currentCurrency: string;
}

/** Net worth, assets, liabilities and equity totals above the accounts tree. */
export const AccountSummaryCards: React.FC<AccountSummaryCardsProps> = ({
  netWorth,
  assetsTotal,
  liabTotal,
  equityTotal,
  currentCurrency,
}) => (
  <div className="grid grid-cols-2 lg:grid-cols-4 gap-2.5 sm:gap-4">
          {/* Net Worth */}
          <div className="bg-white p-3 sm:p-4 rounded-xl border border-slate-200 shadow-sm flex items-center justify-between">
            <div>
              <p className="text-[10px] sm:text-xs font-semibold text-slate-500 uppercase tracking-wider">Net Worth</p>
              <h3 className="text-base sm:text-xl font-bold font-mono text-slate-900 mt-0.5 sm:mt-1 truncate">
                {formatCurrency(netWorth, currentCurrency)}
              </h3>
              <p className="text-[10px] sm:text-[11px] text-slate-400 mt-0.5 hidden xs:block">Assets minus Liabilities</p>
            </div>
            <div className="w-8 h-8 sm:w-10 sm:h-10 rounded-lg bg-emerald-50 border border-emerald-100 flex items-center justify-center text-emerald-600 shrink-0">
              <Scale className="w-4 h-4 sm:w-5 sm:h-5" />
            </div>
          </div>

          {/* Total Assets */}
          <div className="bg-white p-3 sm:p-4 rounded-xl border border-slate-200 shadow-sm flex items-center justify-between">
            <div>
              <p className="text-[10px] sm:text-xs font-semibold text-slate-500 uppercase tracking-wider">Total Assets</p>
              <h3 className="text-base sm:text-xl font-bold font-mono text-sky-700 mt-0.5 sm:mt-1 truncate">
                {formatCurrency(assetsTotal, currentCurrency)}
              </h3>
              <p className="text-[10px] sm:text-[11px] text-slate-400 mt-0.5 hidden xs:block">Cash, Bank, Property</p>
            </div>
            <div className="w-8 h-8 sm:w-10 sm:h-10 rounded-lg bg-sky-50 border border-sky-100 flex items-center justify-center text-sky-600 shrink-0">
              <Landmark className="w-4 h-4 sm:w-5 sm:h-5" />
            </div>
          </div>

          {/* Total Liabilities */}
          <div className="bg-white p-3 sm:p-4 rounded-xl border border-slate-200 shadow-sm flex items-center justify-between">
            <div>
              <p className="text-[10px] sm:text-xs font-semibold text-slate-500 uppercase tracking-wider">Liabilities</p>
              <h3 className="text-base sm:text-xl font-bold font-mono text-amber-700 mt-0.5 sm:mt-1 truncate">
                {formatCurrency(liabTotal, currentCurrency)}
              </h3>
              <p className="text-[10px] sm:text-[11px] text-slate-400 mt-0.5 hidden xs:block">Credit Cards, Payables</p>
            </div>
            <div className="w-8 h-8 sm:w-10 sm:h-10 rounded-lg bg-amber-50 border border-amber-100 flex items-center justify-center text-amber-600 shrink-0">
              <CreditCard className="w-4 h-4 sm:w-5 sm:h-5" />
            </div>
          </div>

          {/* Total Equity */}
          <div className="bg-white p-3 sm:p-4 rounded-xl border border-slate-200 shadow-sm flex items-center justify-between">
            <div>
              <p className="text-[10px] sm:text-xs font-semibold text-slate-500 uppercase tracking-wider">Equity</p>
              <h3 className="text-base sm:text-xl font-bold font-mono text-purple-700 mt-0.5 sm:mt-1 truncate">
                {formatCurrency(equityTotal, currentCurrency)}
              </h3>
              <p className="text-[10px] sm:text-[11px] text-slate-400 mt-0.5 hidden xs:block">Opening Balances & Earnings</p>
            </div>
            <div className="w-8 h-8 sm:w-10 sm:h-10 rounded-lg bg-purple-50 border border-purple-100 flex items-center justify-center text-purple-600 shrink-0">
              <Scale className="w-4 h-4 sm:w-5 sm:h-5" />
            </div>
          </div>
        </div>
);
