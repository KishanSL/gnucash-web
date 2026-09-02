import React from 'react';
import {
  BookOpen,
  FolderTree,
  ListFilter,
  CheckCircle2,
  BarChart3,
  PlusCircle,
  FolderPlus,
  RefreshCw,
  Coins,
  Upload,
} from 'lucide-react';
import { SUPPORTED_CURRENCIES } from '../api';

export type ActiveTab = 'accounts' | 'register' | 'journal' | 'reconcile' | 'reports';

interface NavbarProps {
  activeTab: ActiveTab;
  setActiveTab: (tab: ActiveTab) => void;
  onOpenNewTx: () => void;
  onOpenNewAccount: () => void;
  onOpenImport: () => void;
  onSeedData: () => void;
  backendOnline: boolean;
  selectedAccountName?: string;
  currency: string;
  onCurrencyChange: (curr: string) => void;
}

export const Navbar: React.FC<NavbarProps> = ({
  activeTab,
  setActiveTab,
  onOpenNewTx,
  onOpenNewAccount,
  onOpenImport,
  onSeedData,
  backendOnline,
  selectedAccountName,
  currency,
  onCurrencyChange,
}) => {
  return (
    <header className="bg-slate-900 border-b border-slate-800 text-white sticky top-0 z-30 shadow-md">
      <div className="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8">
        <div className="flex items-center justify-between h-16 gap-2">
          {/* Logo & Brand */}
          <div className="flex items-center space-x-3 shrink-0">
            <div className="w-10 h-10 rounded-lg bg-gradient-to-tr from-emerald-600 to-teal-500 flex items-center justify-center shadow-inner">
              <Coins className="w-6 h-6 text-white" />
            </div>
            <div>
              <div className="flex items-center space-x-2">
                <span className="font-bold text-lg tracking-tight text-white">GnuCash</span>
                <span className="text-[10px] font-semibold px-1.5 py-0.5 rounded bg-emerald-500/20 text-emerald-400 border border-emerald-500/30">
                  Web
                </span>
              </div>
              <p className="text-[11px] text-slate-400">Double-Entry Accounting</p>
            </div>
          </div>

          {/* Navigation Tabs */}
          <nav className="hidden md:flex space-x-1 bg-slate-950/60 p-1 rounded-xl border border-slate-800">
            <button
              onClick={() => setActiveTab('accounts')}
              className={`flex items-center space-x-1.5 px-3 py-1.5 rounded-lg text-xs font-medium transition-all ${
                activeTab === 'accounts'
                  ? 'bg-emerald-600 text-white shadow-sm'
                  : 'text-slate-300 hover:text-white hover:bg-slate-800/60'
              }`}
            >
              <FolderTree className="w-4 h-4" />
              <span>Accounts</span>
            </button>

            <button
              onClick={() => setActiveTab('register')}
              className={`flex items-center space-x-1.5 px-3 py-1.5 rounded-lg text-xs font-medium transition-all ${
                activeTab === 'register'
                  ? 'bg-emerald-600 text-white shadow-sm'
                  : 'text-slate-300 hover:text-white hover:bg-slate-800/60'
              }`}
            >
              <BookOpen className="w-4 h-4" />
              <span>
                Register {selectedAccountName ? `(${selectedAccountName.slice(0, 10)}...)` : ''}
              </span>
            </button>

            <button
              onClick={() => setActiveTab('journal')}
              className={`flex items-center space-x-1.5 px-3 py-1.5 rounded-lg text-xs font-medium transition-all ${
                activeTab === 'journal'
                  ? 'bg-emerald-600 text-white shadow-sm'
                  : 'text-slate-300 hover:text-white hover:bg-slate-800/60'
              }`}
            >
              <ListFilter className="w-4 h-4" />
              <span>Journal</span>
            </button>

            <button
              onClick={() => setActiveTab('reconcile')}
              className={`flex items-center space-x-1.5 px-3 py-1.5 rounded-lg text-xs font-medium transition-all ${
                activeTab === 'reconcile'
                  ? 'bg-emerald-600 text-white shadow-sm'
                  : 'text-slate-300 hover:text-white hover:bg-slate-800/60'
              }`}
            >
              <CheckCircle2 className="w-4 h-4" />
              <span>Reconcile</span>
            </button>

            <button
              onClick={() => setActiveTab('reports')}
              className={`flex items-center space-x-1.5 px-3 py-1.5 rounded-lg text-xs font-medium transition-all ${
                activeTab === 'reports'
                  ? 'bg-emerald-600 text-white shadow-sm'
                  : 'text-slate-300 hover:text-white hover:bg-slate-800/60'
              }`}
            >
              <BarChart3 className="w-4 h-4" />
              <span>Reports</span>
            </button>
          </nav>

          {/* Currency Selector & Actions */}
          <div className="flex items-center space-x-2">
            {/* Currency Selector Dropdown (Defaults to INR ₹) */}
            <div className="flex items-center space-x-1 bg-slate-950/80 px-2 py-1 rounded-lg border border-slate-700">
              <span className="text-[11px] text-slate-400 font-semibold uppercase">Currency:</span>
              <select
                value={currency}
                onChange={(e) => onCurrencyChange(e.target.value)}
                className="bg-transparent text-emerald-400 font-bold text-xs cursor-pointer focus:outline-none border-none py-0.5 font-mono"
              >
                {SUPPORTED_CURRENCIES.map((c) => (
                  <option key={c.code} value={c.code} className="bg-slate-900 text-white">
                    {c.symbol} {c.code}
                  </option>
                ))}
              </select>
            </div>

            {/* Import .gnucash SQLite Button */}
            <button
              onClick={onOpenImport}
              className="flex items-center space-x-1 px-2.5 py-1.5 text-xs text-slate-200 hover:text-white bg-slate-800 hover:bg-slate-700 rounded-lg border border-slate-700 transition"
              title="Import GnuCash SQLite (.gnucash) file"
            >
              <Upload className="w-3.5 h-3.5 text-sky-400" />
              <span className="hidden sm:inline">Import .gnucash</span>
            </button>

            {/* Seed Button */}
            <button
              onClick={onSeedData}
              className="hidden lg:flex items-center space-x-1 px-2.5 py-1.5 text-xs text-slate-300 hover:text-white bg-slate-800 hover:bg-slate-700 rounded-lg border border-slate-700 transition"
              title="Reset / Seed default GnuCash Chart of Accounts"
            >
              <RefreshCw className="w-3.5 h-3.5 text-slate-400" />
              <span>Seed</span>
            </button>

            {/* New Account Button */}
            <button
              onClick={onOpenNewAccount}
              className="hidden sm:flex items-center space-x-1 px-2.5 py-1.5 text-xs font-medium bg-slate-800 hover:bg-slate-700 text-slate-100 rounded-lg border border-slate-700 transition shadow-sm"
            >
              <FolderPlus className="w-3.5 h-3.5 text-emerald-400" />
              <span>New Account</span>
            </button>

            {/* New Transaction Button */}
            <button
              onClick={onOpenNewTx}
              className="flex items-center space-x-1.5 px-3 py-1.5 text-xs font-semibold bg-emerald-600 hover:bg-emerald-500 text-white rounded-lg transition shadow-md shadow-emerald-900/40 active:scale-95"
            >
              <PlusCircle className="w-4 h-4" />
              <span>New Entry</span>
            </button>
          </div>
        </div>
      </div>
    </header>
  );
};
