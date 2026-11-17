import React from 'react';
import { FolderPlus, RefreshCw, Coins, Upload, Download, LogOut, KeyRound, RotateCcw } from 'lucide-react';
import { SUPPORTED_CURRENCIES } from '../../api';
import { User } from '../../types';
import { Setter } from './types';

interface MobileDrawerProps {
  onOpenNewAccount: () => void;
  onOpenImport: () => void;
  onExport: () => void;
  onSeedData: () => void;
  onResetDataset?: () => void;
  backendOnline: boolean;
  currency: string;
  onCurrencyChange: (curr: string) => void;
  currentUser?: User | null;
  onLogout?: () => void;
  setIsMobileMenuOpen: Setter<boolean>;
  setIsChangePasswordOpen: Setter<boolean>;
}

/** Mobile slide-down drawer; mirrors every action in the desktop user menu. */
export const MobileDrawer: React.FC<MobileDrawerProps> = ({
  onOpenNewAccount,
  onOpenImport,
  onExport,
  onSeedData,
  onResetDataset,
  backendOnline,
  currency,
  onCurrencyChange,
  currentUser,
  onLogout,
  setIsMobileMenuOpen,
  setIsChangePasswordOpen,
}) => (
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

      <button
        onClick={() => {
          onExport();
          setIsMobileMenuOpen(false);
        }}
        className="col-span-2 flex items-center justify-center space-x-2 p-2.5 text-xs font-medium bg-slate-800/80 hover:bg-slate-700 text-slate-200 rounded-lg border border-slate-700 text-center"
      >
        <Download className="w-4 h-4 text-emerald-400 shrink-0" />
        <span>Export .gnucash (SQLite)</span>
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
        className="bg-slate-800 !text-white text-white text-xs font-bold rounded-md px-2.5 py-1 border border-slate-600 focus:outline-none focus:ring-1 focus:ring-emerald-500 font-mono cursor-pointer"
        style={{ color: '#ffffff' }}
        aria-label="Display Currency"
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

);
