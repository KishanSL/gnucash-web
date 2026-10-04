import React, { useState } from 'react';
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
  Menu,
  X,
  ChevronDown,
  LogOut,
  User as UserIcon,
} from 'lucide-react';
import { SUPPORTED_CURRENCIES } from '../api';
import { User } from '../types';

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
  currentUser?: User | null;
  onLogout?: () => void;
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
  currentUser,
  onLogout,
}) => {
  const [isMobileMenuOpen, setIsMobileMenuOpen] = useState(false);

  const navItems: { id: ActiveTab; label: string; icon: React.ReactNode }[] = [
    { id: 'accounts', label: 'Accounts', icon: <FolderTree className="w-4 h-4" /> },
    {
      id: 'register',
      label: selectedAccountName ? `Register (${selectedAccountName.slice(0, 8)}…)` : 'Register',
      icon: <BookOpen className="w-4 h-4" />,
    },
    { id: 'journal', label: 'Journal', icon: <ListFilter className="w-4 h-4" /> },
    { id: 'reconcile', label: 'Reconcile', icon: <CheckCircle2 className="w-4 h-4" /> },
    { id: 'reports', label: 'Reports', icon: <BarChart3 className="w-4 h-4" /> },
  ];

  return (
    <>
      <header className="bg-slate-900 border-b border-slate-800 text-white sticky top-0 z-30 shadow-md">
        <div className="max-w-7xl mx-auto px-3 sm:px-6 lg:px-8">
          <div className="flex items-center justify-between h-14 sm:h-16 gap-2">
            {/* Logo & Brand */}
            <div className="flex items-center space-x-2.5 shrink-0">
              <div className="w-9 h-9 sm:w-10 sm:h-10 rounded-lg bg-gradient-to-tr from-emerald-600 to-teal-500 flex items-center justify-center shadow-inner">
                <Coins className="w-5 h-5 sm:w-6 sm:h-6 text-white" />
              </div>
              <div>
                <div className="flex items-center space-x-1.5">
                  <span className="font-bold text-base sm:text-lg tracking-tight text-white">GnuCash</span>
                  <span className="text-[10px] font-semibold px-1.5 py-0.2 rounded bg-emerald-500/20 text-emerald-400 border border-emerald-500/30">
                    Web
                  </span>
                </div>
                <p className="text-[10px] sm:text-[11px] text-slate-400 hidden xs:block">Double-Entry Accounting</p>
              </div>
            </div>

            {/* Desktop Navigation Tabs */}
            <nav className="hidden md:flex space-x-1 bg-slate-950/60 p-1 rounded-xl border border-slate-800">
              {navItems.map((item) => (
                <button
                  key={item.id}
                  onClick={() => setActiveTab(item.id)}
                  className={`flex items-center space-x-1.5 px-3 py-1.5 rounded-lg text-xs font-medium transition-all ${
                    activeTab === item.id
                      ? 'bg-emerald-600 text-white shadow-sm'
                      : 'text-slate-300 hover:text-white hover:bg-slate-800/60'
                  }`}
                >
                  {item.icon}
                  <span>{item.label}</span>
                </button>
              ))}
            </nav>

            {/* Actions & Currency */}
            <div className="flex items-center space-x-1.5 sm:space-x-2">
              {/* Currency Selector Dropdown */}
              <div className="flex items-center space-x-1 bg-slate-950/80 px-2 py-1 rounded-lg border border-slate-700">
                <span className="text-[10px] sm:text-[11px] text-slate-400 font-semibold uppercase hidden sm:inline">
                  Curr:
                </span>
                <select
                  value={currency}
                  onChange={(e) => onCurrencyChange(e.target.value)}
                  className="bg-transparent text-emerald-400 font-bold text-xs cursor-pointer focus:outline-none border-none py-0.5 font-mono"
                  aria-label="Select Currency"
                >
                  {SUPPORTED_CURRENCIES.map((c) => (
                    <option key={c.code} value={c.code} className="bg-slate-900 text-white">
                      {c.symbol} {c.code}
                    </option>
                  ))}
                </select>
              </div>

              {/* Import .gnucash SQLite Button (Desktop) */}
              <button
                onClick={onOpenImport}
                className="hidden lg:flex items-center space-x-1 px-2.5 py-1.5 text-xs text-slate-200 hover:text-white bg-slate-800 hover:bg-slate-700 rounded-lg border border-slate-700 transition"
                title="Import GnuCash SQLite (.gnucash) file"
              >
                <Upload className="w-3.5 h-3.5 text-sky-400" />
                <span>Import .gnucash</span>
              </button>

              {/* Seed Button (Desktop) */}
              <button
                onClick={onSeedData}
                className="hidden xl:flex items-center space-x-1 px-2.5 py-1.5 text-xs text-slate-300 hover:text-white bg-slate-800 hover:bg-slate-700 rounded-lg border border-slate-700 transition"
                title="Reset / Seed default GnuCash Chart of Accounts"
              >
                <RefreshCw className="w-3.5 h-3.5 text-slate-400" />
                <span>Seed</span>
              </button>

              {/* New Account Button (Desktop) */}
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
                className="flex items-center space-x-1 sm:space-x-1.5 px-2.5 sm:px-3 py-1.5 text-xs font-semibold bg-emerald-600 hover:bg-emerald-500 text-white rounded-lg transition shadow-md shadow-emerald-900/40 active:scale-95"
              >
                <PlusCircle className="w-4 h-4" />
                <span className="hidden xs:inline">New Entry</span>
              </button>

              {/* User profile & logout (Desktop) */}
              {currentUser && (
                <div className="hidden sm:flex items-center space-x-1.5 pl-1.5 border-l border-slate-800">
                  <div
                    className="flex items-center space-x-1.5 bg-slate-950/80 px-2.5 py-1 rounded-lg border border-slate-700/80 text-xs"
                    title={`Logged in as ${currentUser.username}${currentUser.email ? ` (${currentUser.email})` : ''}`}
                  >
                    <div className="w-5 h-5 rounded-full bg-emerald-600/30 border border-emerald-500/40 flex items-center justify-center text-[10px] font-bold text-emerald-300">
                      {currentUser.username.charAt(0).toUpperCase()}
                    </div>
                    <span className="font-medium text-slate-200 max-w-[80px] truncate hidden xl:inline">
                      {currentUser.username}
                    </span>
                  </div>
                  {onLogout && (
                    <button
                      onClick={onLogout}
                      className="p-1.5 text-slate-400 hover:text-rose-400 hover:bg-slate-800 rounded-lg transition"
                      title="Log Out"
                      aria-label="Log Out"
                    >
                      <LogOut className="w-4 h-4" />
                    </button>
                  )}
                </div>
              )}

              {/* Mobile Menu Hamburger Button */}
              <button
                onClick={() => setIsMobileMenuOpen(!isMobileMenuOpen)}
                className="md:hidden p-1.5 rounded-lg text-slate-300 hover:text-white hover:bg-slate-800 border border-slate-800 transition"
                aria-label="Open Mobile Menu"
              >
                {isMobileMenuOpen ? <X className="w-5 h-5" /> : <Menu className="w-5 h-5" />}
              </button>
            </div>
          </div>
        </div>

        {/* Mobile Slide-down Drawer / Menu */}
        {isMobileMenuOpen && (
          <div className="md:hidden border-t border-slate-800 bg-slate-900 px-4 py-3 space-y-3 animate-in slide-in-from-top-2 duration-150">
            <div className="text-[11px] font-semibold text-slate-400 uppercase tracking-wider">
              Quick Actions & Settings
            </div>

            <div className="grid grid-cols-2 gap-2">
              <button
                onClick={() => {
                  onOpenNewAccount();
                  setIsMobileMenuOpen(false);
                }}
                className="flex items-center space-x-2 p-2.5 text-xs font-medium bg-slate-800/80 hover:bg-slate-700 text-slate-200 rounded-lg border border-slate-700 text-left"
              >
                <FolderPlus className="w-4 h-4 text-emerald-400 shrink-0" />
                <span>New Account</span>
              </button>

              <button
                onClick={() => {
                  onOpenImport();
                  setIsMobileMenuOpen(false);
                }}
                className="flex items-center space-x-2 p-2.5 text-xs font-medium bg-slate-800/80 hover:bg-slate-700 text-slate-200 rounded-lg border border-slate-700 text-left"
              >
                <Upload className="w-4 h-4 text-sky-400 shrink-0" />
                <span>Import .gnucash</span>
              </button>
            </div>

            {currentUser && (
              <div className="pt-2 border-t border-slate-800/80 flex items-center justify-between">
                <div className="flex items-center space-x-2">
                  <div className="w-7 h-7 rounded-full bg-emerald-600/30 border border-emerald-500/40 flex items-center justify-center text-xs font-bold text-emerald-300">
                    {currentUser.username.charAt(0).toUpperCase()}
                  </div>
                  <div>
                    <div className="text-xs font-semibold text-slate-200">{currentUser.username}</div>
                    {currentUser.email && (
                      <div className="text-[10px] text-slate-400">{currentUser.email}</div>
                    )}
                  </div>
                </div>
                {onLogout && (
                  <button
                    onClick={() => {
                      onLogout();
                      setIsMobileMenuOpen(false);
                    }}
                    className="flex items-center space-x-1 text-xs text-rose-400 hover:text-rose-300 px-2.5 py-1 bg-rose-500/10 hover:bg-rose-500/20 border border-rose-500/20 rounded-md transition"
                  >
                    <LogOut className="w-3.5 h-3.5" />
                    <span>Log Out</span>
                  </button>
                )}
              </div>
            )}

            <div className="pt-2 border-t border-slate-800/80 flex items-center justify-between">
              <span className="text-xs text-slate-400">Database Status:</span>
              <span
                className={`text-xs font-semibold px-2 py-0.5 rounded-full ${
                  backendOnline
                    ? 'bg-emerald-500/20 text-emerald-400 border border-emerald-500/30'
                    : 'bg-rose-500/20 text-rose-400 border border-rose-500/30'
                }`}
              >
                {backendOnline ? 'Online (8082)' : 'Disconnected'}
              </span>
            </div>

            <button
              onClick={() => {
                onSeedData();
                setIsMobileMenuOpen(false);
              }}
              className="w-full flex items-center justify-center space-x-2 py-2 text-xs font-medium text-slate-300 hover:text-white bg-slate-800/40 hover:bg-slate-800 rounded-lg border border-slate-700/60"
            >
              <RefreshCw className="w-3.5 h-3.5 text-slate-400" />
              <span>Reset & Seed Sample Data</span>
            </button>
          </div>
        )}
      </header>

      {/* Mobile Bottom Navigation Bar (Persistent at bottom on screens < md) */}
      <nav className="md:hidden fixed bottom-0 left-0 right-0 z-40 bg-slate-900/95 backdrop-blur-md border-t border-slate-800 shadow-2xl flex items-center justify-around py-1.5 px-1 safe-area-bottom">
        {navItems.map((item) => {
          const isActive = activeTab === item.id;
          return (
            <button
              key={item.id}
              onClick={() => {
                setActiveTab(item.id);
                setIsMobileMenuOpen(false);
              }}
              className={`flex flex-col items-center justify-center py-1 px-2 rounded-lg text-[10px] font-medium transition-all ${
                isActive
                  ? 'text-emerald-400 font-bold'
                  : 'text-slate-400 hover:text-slate-200'
              }`}
            >
              <div
                className={`p-1 rounded-md mb-0.5 transition ${
                  isActive ? 'bg-emerald-500/20 text-emerald-400' : ''
                }`}
              >
                {item.icon}
              </div>
              <span className="truncate max-w-[64px]">
                {item.id === 'register' ? 'Register' : item.label}
              </span>
            </button>
          );
        })}
      </nav>
    </>
  );
};
