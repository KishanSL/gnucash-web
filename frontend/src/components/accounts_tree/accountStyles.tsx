import React from 'react';
import { Folder, FolderOpen, Landmark, Wallet, CreditCard, TrendingUp, Receipt, Scale } from 'lucide-react';

// Icon and badge colours per account type.

export function getAccountTypeIcon(type: string, isPlaceholder: boolean): React.ReactNode {
  switch (type.toLowerCase()) {
    case 'bank':
      return <Landmark className="w-4 h-4 text-sky-600" />;
    case 'cash':
      return <Wallet className="w-4 h-4 text-emerald-600" />;
    case 'credit':
      return <CreditCard className="w-4 h-4 text-amber-600" />;
    case 'income':
      return <TrendingUp className="w-4 h-4 text-emerald-600" />;
    case 'expense':
      return <Receipt className="w-4 h-4 text-rose-500" />;
    case 'equity':
      return <Scale className="w-4 h-4 text-purple-600" />;
    case 'asset':
    case 'liability':
    default:
      return isPlaceholder ? (
        <Folder className="w-4 h-4 text-amber-500" />
      ) : (
        <FolderOpen className="w-4 h-4 text-slate-500" />
      );
  }
}

export function getBadgeStyle(type: string): string {
  switch (type.toLowerCase()) {
    case 'bank':
    case 'asset':
    case 'cash':
    case 'receivable':
      return 'bg-sky-50 text-sky-700 border-sky-200';
    case 'liability':
    case 'credit':
    case 'payable':
      return 'bg-amber-50 text-amber-700 border-amber-200';
    case 'equity':
      return 'bg-purple-50 text-purple-700 border-purple-200';
    case 'income':
      return 'bg-emerald-50 text-emerald-700 border-emerald-200';
    case 'expense':
      return 'bg-rose-50 text-rose-700 border-rose-200';
    default:
      return 'bg-slate-100 text-slate-700 border-slate-200';
  }
}
