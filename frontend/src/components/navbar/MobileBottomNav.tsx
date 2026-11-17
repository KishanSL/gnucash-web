import React from 'react';
import { ActiveTab, NavItem, Setter } from './types';

interface MobileBottomNavProps {
  activeTab: ActiveTab;
  setActiveTab: (tab: ActiveTab) => void;
  setIsMobileMenuOpen: Setter<boolean>;
  navItems: NavItem[];
}

/** Persistent bottom tab bar on screens narrower than md. */
export const MobileBottomNav: React.FC<MobileBottomNavProps> = ({
  activeTab,
  setActiveTab,
  setIsMobileMenuOpen,
  navItems,
}) => (
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
);
