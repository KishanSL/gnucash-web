import React from 'react';
import { RefreshCw, Coins, Upload, Download, LogOut, KeyRound, RotateCcw, ShieldCheck } from 'lucide-react';
import { SUPPORTED_CURRENCIES } from '../../api';
import { User } from '../../types';
import { Setter } from './types';

interface UserMenuDropdownProps {
  onOpenImport: () => void;
  onExport: () => void;
  onSeedData: () => void;
  onResetDataset?: () => void;
  currency: string;
  onCurrencyChange: (curr: string) => void;
  currentUser: User;
  onLogout?: () => void;
  setIsUserMenuOpen: Setter<boolean>;
  setIsChangePasswordOpen: Setter<boolean>;
  setIsApiTokensOpen: Setter<boolean>;
}

/** Desktop user menu: profile, currency, import/export, seed, password, reset and logout (AGENTS.md section 3). */
export const UserMenuDropdown: React.FC<UserMenuDropdownProps> = ({
  onOpenImport,
  onExport,
  onSeedData,
  onResetDataset,
  currency,
  onCurrencyChange,
  currentUser,
  onLogout,
  setIsUserMenuOpen,
  setIsChangePasswordOpen,
  setIsApiTokensOpen,
}) => (
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
        <span className="font-medium text-slate-200">Currency</span>
      </div>
      <select
        value={currency}
        onChange={(e) => onCurrencyChange(e.target.value)}
        className="bg-slate-800 hover:bg-slate-700 !text-white text-white text-xs font-bold rounded-md px-2.5 py-1 border border-slate-600 focus:outline-none focus:ring-1 focus:ring-emerald-500 font-mono cursor-pointer transition shadow-sm"
        style={{ color: '#ffffff' }}
        aria-label="Select Currency"
      >
        {SUPPORTED_CURRENCIES.map((c) => (
          <option
            key={c.code}
            value={c.code}
            className="bg-slate-900 text-white font-normal"
            style={{ color: '#ffffff', backgroundColor: '#0f172a' }}
          >
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
          onExport();
        }}
        className="w-full flex items-center space-x-2 px-3.5 py-2 text-xs text-slate-300 hover:text-white hover:bg-slate-800 transition text-left"
        title="Export database as standard GnuCash SQLite file (.gnucash)"
      >
        <Download className="w-4 h-4 text-emerald-400 shrink-0" />
        <span>Export .gnucash</span>
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

      <button
        type="button"
        onClick={() => {
          setIsUserMenuOpen(false);
          setIsApiTokensOpen(true);
        }}
        className="w-full flex items-center space-x-2 px-3.5 py-2 text-xs text-slate-300 hover:text-white hover:bg-slate-800 transition text-left"
        title="Create and revoke API tokens for service clients"
      >
        <ShieldCheck className="w-4 h-4 text-emerald-400 shrink-0" />
        <span>API Tokens</span>
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

);
