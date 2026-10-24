import React, { useState, useRef, useEffect } from 'react';
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
  KeyRound,
  RotateCcw,
} from 'lucide-react';
import { SUPPORTED_CURRENCIES } from '../api';
import { User } from '../types';
import { ChangePasswordModal } from './ChangePasswordModal';

export type ActiveTab = 'accounts' | 'register' | 'journal' | 'reconcile' | 'reports';

interface NavbarProps {
  activeTab: ActiveTab;
  setActiveTab: (tab: ActiveTab) => void;
  onOpenNewTx: () => void;
  onOpenNewAccount: () => void;
  onOpenImport: () => void;
  onSeedData: () => void;
  onResetDataset?: () => void;
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
  onResetDataset,
  backendOnline,
  selectedAccountName,
  currency,
  onCurrencyChange,
  currentUser,
  onLogout,
}) => {
  const [isMobileMenuOpen, setIsMobileMenuOpen] = useState(false);
  const [isUserMenuOpen, setIsUserMenuOpen] = useState(false);
  const [isChangePasswordOpen, setIsChangePasswordOpen] = useState(false);

  const userMenuRef = useRef<HTMLDivElement>(null);

  // Close user dropdown when clicking outside
  useEffect(() => {
    const handleClickOutside = (event: MouseEvent) => {
      if (userMenuRef.current && !userMenuRef.current.contains(event.target as Node)) {
        setIsUserMenuOpen(false);
      }
    };
    document.addEventListener('mousedown', handleClickOutside);
    return () => {
      document.removeEventListener('mousedown', handleClickOutside);
    };
  }, []);

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

            {/* Actions */}
            <div className="flex items-center space-x-1.5 sm:space-x-2">
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

              {/* User Dropdown (Desktop) */}
              {currentUser && (
                <div className="relative hidden sm:block" ref={userMenuRef}>
                  <button
                    type="button"
                    onClick={() => setIsUserMenuOpen(!isUserMenuOpen)}
                    className="flex items-center space-x-1.5 bg-slate-950/80 hover:bg-slate-800 px-2.5 py-1.5 rounded-lg border border-slate-700/80 text-xs text-slate-200 transition focus:outline-none focus:ring-2 focus:ring-emerald-500/20 active:scale-[0.98]"
                    aria-expanded={isUserMenuOpen}
                    aria-label="User Account Menu"
                  >
                    <div className="w-5 h-5 rounded-full bg-emerald-600/30 border border-emerald-500/40 flex items-center justify-center text-[10px] font-bold text-emerald-300">
                      {currentUser.username.charAt(0).toUpperCase()}
                    </div>
                    <span className="font-semibold text-slate-200 max-w-[110px] truncate">
                      {currentUser.username}
                    </span>
                    <ChevronDown
                      className={`w-3.5 h-3.5 text-slate-400 transition-transform duration-150 ${
                        isUserMenuOpen ? 'rotate-180' : ''
                      }`}
                    />
                  </button>

                  {/* Dropdown Card */}
                  {isUserMenuOpen && (
                    <div className="absolute right-0 mt-2 w-56 bg-slate-900 border border-slate-700/90 rounded-xl shadow-2xl py-1.5 z-50 animate-in fade-in slide-in-from-top-1 duration-150">
                      {/* User Info Header */}
                      <div className="px-3.5 py-2.5 border-b border-slate-800">
                        <div className="flex items-center space-x-2.5">
                          <div className="w-8 h-8 rounded-full bg-emerald-600/30 border border-emerald-500/40 flex items-center justify-center text-xs font-bold text-emerald-300">
                            {currentUser.username.charAt(0).toUpperCase()}
                          </div>
                          <div className="overflow-hidden">
                            <p className="text-xs font-bold text-slate-200 truncate">
                              {currentUser.username}
                            </p>
                            {currentUser.email && (
                              <p className="text-[10px] text-slate-400 truncate">
                                {currentUser.email}
                              </p>
                            )}
                          </div>
                        </div>
                      </div>

                      {/* Currency Preference Selector */}
                      <div className="px-3.5 py-2 border-b border-slate-800 flex items-center justify-between bg-slate-950/40">
                        <div className="flex items-center space-x-2 text-xs text-slate-300">
                          <Coins className="w-4 h-4 text-emerald-400 shrink-0" />
                          <span className="font-medium">Currency</span>
                        </div>
                        <select
                          value={currency}
                          onChange={(e) => onCurrencyChange(e.target.value)}
                          className="bg-slate-800 hover:bg-slate-750 text-emerald-400 text-xs font-semibold rounded-md px-2 py-1 border border-slate-700 focus:outline-none focus:ring-1 focus:ring-emerald-500 font-mono cursor-pointer transition"
                          aria-label="Select Currency"
                        >
                          {SUPPORTED_CURRENCIES.map((c) => (
                            <option key={c.code} value={c.code} className="bg-slate-900 text-white font-normal">
                              {c.symbol} {c.code}
                            </option>
                          ))}
                        </select>
                      </div>

                      {/* Dropdown Menu Items */}
                      <div className="py-1">
                        <button
                          type="button"
                          onClick={() => {
                            setIsUserMenuOpen(false);
                            onOpenImport();
                          }}
                          className="w-full flex items-center space-x-2 px-3.5 py-2 text-xs text-slate-300 hover:text-white hover:bg-slate-800 transition text-left"
                          title="Import GnuCash SQLite / XML file (.gnucash)"
                        >
                          <Upload className="w-4 h-4 text-sky-400 shrink-0" />
                          <span>Import .gnucash</span>
                        </button>

                        <button
                          type="button"
                          onClick={() => {
                            setIsUserMenuOpen(false);
                            onSeedData();
                          }}
                          className="w-full flex items-center space-x-2 px-3.5 py-2 text-xs text-slate-300 hover:text-white hover:bg-slate-800 transition text-left"
                          title="Seed standard Chart of Accounts & realistic sample transactions"
                        >
                          <RefreshCw className="w-4 h-4 text-emerald-400 shrink-0" />
                          <span>Seed Sample Data</span>
                        </button>

                        <button
                          type="button"
                          onClick={() => {
                            setIsUserMenuOpen(false);
                            setIsChangePasswordOpen(true);
                          }}
                          className="w-full flex items-center space-x-2 px-3.5 py-2 text-xs text-slate-300 hover:text-white hover:bg-slate-800 transition text-left"
                        >
                          <KeyRound className="w-4 h-4 text-emerald-400 shrink-0" />
                          <span>Change Password</span>
                        </button>

                        {onResetDataset && (
                          <button
                            type="button"
                            onClick={() => {
                              setIsUserMenuOpen(false);
                              onResetDataset();
                            }}
                            className="w-full flex items-center space-x-2 px-3.5 py-2 text-xs text-amber-400 hover:text-amber-300 hover:bg-amber-500/10 transition text-left"
                            title="Delete all accounts, transactions, and reset database to empty"
                          >
                            <RotateCcw className="w-4 h-4 text-amber-400 shrink-0" />
                            <span>Reset Entire Database</span>
                          </button>
                        )}

                        <div className="my-1 border-t border-slate-800" />

                        {onLogout && (
                          <button
                            type="button"
                            onClick={() => {
                              setIsUserMenuOpen(false);
                              onLogout();
                            }}
                            className="w-full flex items-center space-x-2 px-3.5 py-2 text-xs text-rose-400 hover:text-rose-300 hover:bg-rose-500/10 transition text-left"
                          >
                            <LogOut className="w-4 h-4 text-rose-400 shrink-0" />
                            <span>Log Out</span>
                          </button>
                        )}
                      </div>
                    </div>
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

            {/* Mobile Currency Preference */}
            <div className="flex items-center justify-between p-2.5 bg-slate-800/80 rounded-lg border border-slate-700">
              <div className="flex items-center space-x-2 text-xs text-slate-200">
                <Coins className="w-4 h-4 text-emerald-400 shrink-0" />
                <span className="font-medium">Currency</span>
              </div>
              <select
                value={currency}
                onChange={(e) => onCurrencyChange(e.target.value)}
                className="bg-slate-900 text-emerald-400 text-xs font-semibold rounded-md px-2.5 py-1 border border-slate-700 focus:outline-none focus:ring-1 focus:ring-emerald-500 font-mono cursor-pointer"
                aria-label="Display Currency"
              >
                {SUPPORTED_CURRENCIES.map((c) => (
                  <option key={c.code} value={c.code} className="bg-slate-900 text-white font-normal">
                    {c.symbol} {c.code}
                  </option>
                ))}
              </select>
            </div>

            {/* Mobile User Profile & Password/Logout Actions */}
            {currentUser && (
              <div className="pt-2 border-t border-slate-800/80 space-y-2">
                <div className="flex items-center space-x-2.5">
                  <div className="w-8 h-8 rounded-full bg-emerald-600/30 border border-emerald-500/40 flex items-center justify-center text-xs font-bold text-emerald-300">
                    {currentUser.username.charAt(0).toUpperCase()}
                  </div>
                  <div>
                    <div className="text-xs font-semibold text-slate-200">{currentUser.username}</div>
                    {currentUser.email && (
                      <div className="text-[10px] text-slate-400">{currentUser.email}</div>
                    )}
                  </div>
                </div>

                <div className="grid grid-cols-2 gap-2 pt-1">
                  <button
                    onClick={() => {
                      setIsMobileMenuOpen(false);
                      setIsChangePasswordOpen(true);
                    }}
                    className="flex items-center justify-center space-x-1.5 text-xs text-slate-200 hover:text-white py-2 px-2.5 bg-slate-800 hover:bg-slate-700 border border-slate-700 rounded-lg transition"
                  >
                    <KeyRound className="w-3.5 h-3.5 text-emerald-400" />
                    <span>Change Password</span>
                  </button>

                  <button
                    onClick={() => {
                      setIsMobileMenuOpen(false);
                      onSeedData();
                    }}
                    className="flex items-center justify-center space-x-1.5 text-xs text-slate-200 hover:text-white py-2 px-2.5 bg-slate-800 hover:bg-slate-700 border border-slate-700 rounded-lg transition"
                  >
                    <RefreshCw className="w-3.5 h-3.5 text-emerald-400" />
                    <span>Seed Data</span>
                  </button>
                </div>

                {onResetDataset && (
                  <button
                    onClick={() => {
                      setIsMobileMenuOpen(false);
                      onResetDataset();
                    }}
                    className="w-full flex items-center justify-center space-x-1.5 text-xs text-amber-400 hover:text-amber-300 py-2 px-2.5 bg-amber-500/10 hover:bg-amber-500/20 border border-amber-500/20 rounded-lg transition mt-1"
                  >
                    <RotateCcw className="w-3.5 h-3.5" />
                    <span>Reset Entire Database</span>
                  </button>
                )}

                {onLogout && (
                  <button
                    onClick={() => {
                      setIsMobileMenuOpen(false);
                      onLogout();
                    }}
                    className="w-full flex items-center justify-center space-x-1.5 text-xs text-rose-400 hover:text-rose-300 py-2 px-2.5 bg-rose-500/10 hover:bg-rose-500/20 border border-rose-500/20 rounded-lg transition mt-1"
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
          </div>
        )}
      </header>

      {/* Change Password Modal */}
      {currentUser && (
        <ChangePasswordModal
          isOpen={isChangePasswordOpen}
          onClose={() => setIsChangePasswordOpen(false)}
          username={currentUser.username}
        />
      )}

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
