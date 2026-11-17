import React from 'react';
import { Scale, TrendingUp, BookCheck, LucideIcon } from 'lucide-react';
import { BalanceSheetView } from './BalanceSheetView';
import { IncomeStatementView } from './IncomeStatementView';
import { TrialBalanceView } from './TrialBalanceView';

export type ReportTab = 'balance-sheet' | 'income-statement' | 'trial-balance';

const REPORT_TABS: { id: ReportTab; label: string; icon: LucideIcon }[] = [
  { id: 'balance-sheet', label: 'Balance Sheet', icon: Scale },
  { id: 'income-statement', label: 'Income Statement (P&L)', icon: TrendingUp },
  { id: 'trial-balance', label: 'Trial Balance', icon: BookCheck },
];

interface ReportsViewProps {
  reportTab: ReportTab;
  onReportTabChange: (tab: ReportTab) => void;
  currentCurrency: string;
}

export const ReportsView: React.FC<ReportsViewProps> = ({ reportTab, onReportTabChange, currentCurrency }) => (
  <div className="space-y-6">
    {/* Reports Sub-nav buttons */}
    <div className="flex space-x-2 border-b border-slate-200 pb-3 overflow-x-auto no-scrollbar">
      {REPORT_TABS.map(({ id, label, icon: Icon }) => (
        <button
          key={id}
          onClick={() => onReportTabChange(id)}
          className={`flex items-center space-x-2 px-3.5 py-2 rounded-lg text-xs font-semibold transition ${
            reportTab === id
              ? 'bg-emerald-600 text-white shadow-sm'
              : 'bg-white text-slate-600 hover:bg-slate-100 border border-slate-200'
          }`}
        >
          <Icon className="w-4 h-4" />
          <span>{label}</span>
        </button>
      ))}
    </div>

    {reportTab === 'balance-sheet' && <BalanceSheetView currentCurrency={currentCurrency} />}
    {reportTab === 'income-statement' && <IncomeStatementView currentCurrency={currentCurrency} />}
    {reportTab === 'trial-balance' && <TrialBalanceView currentCurrency={currentCurrency} />}
  </div>
);
