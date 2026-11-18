import React, { useState, useRef, useEffect } from 'react';
import {
  BookOpen,
  FolderTree,
  ListFilter,
  CheckCircle2,
  BarChart3,
  PlusCircle,
  FolderPlus,
  Coins,
  Menu,
  X,
  ChevronDown,
} from 'lucide-react';
import { User } from '../types';
import { ChangePasswordModal } from './ChangePasswordModal';
import { ApiTokensModal } from './ApiTokensModal';
import { UserMenuDropdown } from './navbar/UserMenuDropdown';
import { MobileDrawer } from './navbar/MobileDrawer';
import { MobileBottomNav } from './navbar/MobileBottomNav';
import { ActiveTab, NavItem } from './navbar/types';

export type { ActiveTab } from './navbar/types';

interface NavbarProps {
  activeTab: ActiveTab;
  setActiveTab: (tab: ActiveTab) => void;
  onOpenNewTx: () => void;
  onOpenNewAccount: () => void;
  onOpenImport: () => void;
  onExport: () => void;
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
  onExport,
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
  const [isApiTokensOpen, setIsApiTokensOpen] = useState(false);

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

  const navItems: NavItem[] = [
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
                type="button"
                aria-label="New Entry"
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
                    <UserMenuDropdown
                      onOpenImport={onOpenImport}
                      onExport={onExport}
                      onSeedData={onSeedData}
                      onResetDataset={onResetDataset}
                      currency={currency}
                      onCurrencyChange={onCurrencyChange}
                      currentUser={currentUser}
                      onLogout={onLogout}
                      setIsUserMenuOpen={setIsUserMenuOpen}
                      setIsChangePasswordOpen={setIsChangePasswordOpen}
                      setIsApiTokensOpen={setIsApiTokensOpen}
                    />
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
          <MobileDrawer
            onOpenNewAccount={onOpenNewAccount}
            onOpenImport={onOpenImport}
            onExport={onExport}
            onSeedData={onSeedData}
            onResetDataset={onResetDataset}
            backendOnline={backendOnline}
            currency={currency}
            onCurrencyChange={onCurrencyChange}
            currentUser={currentUser}
            onLogout={onLogout}
            setIsMobileMenuOpen={setIsMobileMenuOpen}
            setIsChangePasswordOpen={setIsChangePasswordOpen}
            setIsApiTokensOpen={setIsApiTokensOpen}
          />
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

      {/* API Tokens Modal */}
      {currentUser && (
        <ApiTokensModal
          isOpen={isApiTokensOpen}
          onClose={() => setIsApiTokensOpen(false)}
          username={currentUser.username}
        />
      )}

      {/* Mobile Bottom Navigation Bar (Persistent at bottom on screens < md) */}
      <MobileBottomNav
        activeTab={activeTab}
        setActiveTab={setActiveTab}
        setIsMobileMenuOpen={setIsMobileMenuOpen}
        navItems={navItems}
      />
    </>
  );
};
