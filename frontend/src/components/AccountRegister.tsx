import React, { useState, useRef, useEffect, useMemo } from 'react';
import {
  BookOpen,
  PlusCircle,
  CheckCircle2,
  ChevronDown,
  ChevronUp,
  Trash2,
  CornerDownRight,
  Split,
  Search,
  Eye,
  EyeOff,
  Check,
  X,
  AlertCircle,
  Plus,
  Pencil,
} from 'lucide-react';
import { Account, CreateTransactionPayload, RegisterEntry } from '../types';
import { formatCurrency } from '../api';
import { AccountAutocomplete } from './AccountAutocomplete';

interface AccountRegisterProps {
  account: Account;
  registerEntries: RegisterEntry[];
  allAccounts: Account[];
  currentCurrency: string;
  onSelectAccount: (acc: Account) => void;
  onOpenNewTx: (defaultAccountId?: string) => void;
  onOpenReconcile: (acc: Account) => void;
  onDeleteTransaction: (txId: string) => void;
  onEditTransaction?: (txId: string) => void;
  onToggleHideAccount?: (acc: Account) => void;
  onCreateTransaction?: (payload: CreateTransactionPayload) => Promise<void>;
}

export const AccountRegister: React.FC<AccountRegisterProps> = ({
  account,
  registerEntries,
  allAccounts,
  currentCurrency,
  onSelectAccount,
  onOpenNewTx,
  onOpenReconcile,
  onDeleteTransaction,
  onEditTransaction,
  onToggleHideAccount,
  onCreateTransaction,
}) => {
  const [expandedTxId, setExpandedTxId] = useState<string | null>(null);
  const [searchTerm, setSearchTerm] = useState('');

  const toggleExpand = (txId: string) => {
    setExpandedTxId(expandedTxId === txId ? null : txId);
  };

  const filteredEntries = registerEntries.filter((entry) => {
    if (!searchTerm) return true;
    const term = searchTerm.toLowerCase();
    return (
      entry.description.toLowerCase().includes(term) ||
      (entry.num && entry.num.toLowerCase().includes(term)) ||
      (entry.transfer_account_name && entry.transfer_account_name.toLowerCase().includes(term)) ||
      (entry.memo && entry.memo.toLowerCase().includes(term))
    );
  });

  const getReconciledBadge = (state: string) => {
    switch (state) {
      case 'y':
        return (
          <span
            className="inline-flex items-center px-1.5 py-0.5 rounded text-[10px] font-bold bg-emerald-100 text-emerald-800 border border-emerald-300"
            title="Reconciled (y)"
          >
            y
          </span>
        );
      case 'c':
        return (
          <span
            className="inline-flex items-center px-1.5 py-0.5 rounded text-[10px] font-bold bg-amber-100 text-amber-800 border border-amber-300"
            title="Cleared (c)"
          >
            c
          </span>
        );
      default:
        return (
          <span
            className="inline-flex items-center px-1.5 py-0.5 rounded text-[10px] font-medium text-slate-400"
            title="Not Reconciled (n)"
          >
            n
          </span>
        );
    }
  };

  // Debit/Credit column header labels based on account type
  const isAsset = ['asset', 'bank', 'cash', 'receivable'].includes(account.account_type.toLowerCase());
  const debitHeader = isAsset ? 'Deposit / Increase' : 'Payment / Decrease';
  const creditHeader = isAsset ? 'Withdrawal / Decrease' : 'Charge / Increase';

  // Inline blank new transaction row state
  const [newDate, setNewDate] = useState(() => new Date().toISOString().split('T')[0]);
  const [newNum, setNewNum] = useState('');
  const [newDescription, setNewDescription] = useState('');
  const [newNotes, setNewNotes] = useState('');
  const [newMemo, setNewMemo] = useState('');
  const [newTransferAccountId, setNewTransferAccountId] = useState('');
  const [newDebit, setNewDebit] = useState('');
  const [newCredit, setNewCredit] = useState('');
  const [mobileAmount, setMobileAmount] = useState('');
  const [mobileAmountType, setMobileAmountType] = useState<'debit' | 'credit'>('debit');
  const [isSubmittingNew, setIsSubmittingNew] = useState(false);
  const [newRowError, setNewRowError] = useState<string | null>(null);

  // Payee autocomplete dropdown states for new row
  const [isPayeeDropdownOpen, setIsPayeeDropdownOpen] = useState(false);
  const [payeeHighlightIndex, setPayeeHighlightIndex] = useState(0);
  const payeeRef = useRef<HTMLDivElement>(null);

  // Bottom scroll refs for auto-scrolling to the end
  const tableBottomRef = useRef<HTMLTableRowElement | null>(null);
  const mobileBottomRef = useRef<HTMLDivElement | null>(null);

  // Transfer accounts (all non-placeholder accounts except current)
  const transferAccounts = useMemo(() => {
    return allAccounts.filter((a) => !a.placeholder && a.id !== account.id);
  }, [allAccounts, account.id]);

  // Group distinct previous payees from THIS CURRENT ACCOUNT's complete history (registerEntries) mapped to their latest entry
  const payeeHistory = useMemo(() => {
    if (!registerEntries || registerEntries.length === 0) return [];
    // Sort copy descending so the latest transaction for each payee is seen first
    const sorted = [...registerEntries].sort((a, b) => {
      const timeA = new Date(a.post_date).getTime();
      const timeB = new Date(b.post_date).getTime();
      if (timeA !== timeB) return timeB - timeA;
      return (b.split_id || '').localeCompare(a.split_id || '');
    });

    const map = new Map<string, { payee: string; latestEntry: RegisterEntry; count: number }>();
    for (const entry of sorted) {
      const p = (entry.description || '').trim();
      if (!p) continue;
      const key = p.toLowerCase();
      if (!map.has(key)) {
        map.set(key, { payee: p, latestEntry: entry, count: 1 });
      } else {
        map.get(key)!.count += 1;
      }
    }
    return Array.from(map.values());
  }, [registerEntries]);

  // Filtered payees matching current description input
  const filteredPayees = useMemo(() => {
    const q = newDescription.trim().toLowerCase();
    if (!q) return payeeHistory.slice(0, 8);
    return payeeHistory
      .filter((item) => item.payee.toLowerCase().includes(q))
      .slice(0, 8);
  }, [payeeHistory, newDescription]);

  useEffect(() => {
    setPayeeHighlightIndex(0);
  }, [filteredPayees]);

  // Close payee dropdown on outside click
  useEffect(() => {
    const handleClickOutside = (e: MouseEvent) => {
      if (payeeRef.current && !payeeRef.current.contains(e.target as Node)) {
        setIsPayeeDropdownOpen(false);
      }
    };
    document.addEventListener('mousedown', handleClickOutside);
    return () => {
      document.removeEventListener('mousedown', handleClickOutside);
    };
  }, []);

  // Handle payee selection: sets description and autofills transfer account, memo, notes, and amount
  const handleSelectPayee = (item: { payee: string; latestEntry: RegisterEntry; count?: number }) => {
    setNewDescription(item.payee);
    setIsPayeeDropdownOpen(false);

    const entry = item.latestEntry;
    if (entry.notes) {
      setNewNotes(entry.notes);
    }
    if (entry.memo) {
      setNewMemo(entry.memo);
    }

    // Find the other split account in the previous transaction
    if (entry.all_splits && entry.all_splits.length >= 2) {
      const otherSplit = entry.all_splits.find((s) => s.account_id !== account.id);
      if (otherSplit) {
        setNewTransferAccountId(otherSplit.account_id);
        if (otherSplit.memo && !entry.memo) {
          setNewMemo(otherSplit.memo);
        }
      }
    }

    // Autofill debit/credit amount and mobile amount based on previous entry
    if (entry.debit && Math.abs(parseFloat(entry.debit)) > 0.0001) {
      const val = Math.abs(parseFloat(entry.debit)).toFixed(2);
      setNewDebit(val);
      setNewCredit('');
      setMobileAmount(val);
      setMobileAmountType('debit');
    } else if (entry.credit && Math.abs(parseFloat(entry.credit)) > 0.0001) {
      const val = Math.abs(parseFloat(entry.credit)).toFixed(2);
      setNewCredit(val);
      setNewDebit('');
      setMobileAmount(val);
      setMobileAmountType('credit');
    }
  };

  // Automatically scroll to the end with the extra blank row
  useEffect(() => {
    const timer = setTimeout(() => {
      if (tableBottomRef.current) {
        tableBottomRef.current.scrollIntoView({ behavior: 'smooth', block: 'center' });
      }
      if (mobileBottomRef.current) {
        mobileBottomRef.current.scrollIntoView({ behavior: 'smooth', block: 'center' });
      }
    }, 150);
    return () => clearTimeout(timer);
  }, [account.id, registerEntries.length]);

  const handleResetNewRow = () => {
    setNewDate(new Date().toISOString().split('T')[0]);
    setNewNum('');
    setNewDescription('');
    setNewNotes('');
    setNewMemo('');
    setNewTransferAccountId('');
    setNewDebit('');
    setNewCredit('');
    setMobileAmount('');
    setMobileAmountType('debit');
    setNewRowError(null);
  };

  // Desktop save handler
  const handleSaveNewRow = async () => {
    if (!onCreateTransaction) {
      console.warn('handleSaveNewRow: onCreateTransaction prop is not defined');
      return;
    }

    const debitNum = parseFloat(newDebit) || 0;
    const creditNum = parseFloat(newCredit) || 0;

    if (debitNum <= 0 && creditNum <= 0) {
      setNewRowError(`Please enter an amount in either ${debitHeader} or ${creditHeader}`);
      return;
    }

    if (!newTransferAccountId) {
      setNewRowError('Please select a transfer account');
      return;
    }

    const isDeposit = debitNum > 0;
    const amountVal = isDeposit ? debitNum : creditNum;

    // Strict double-entry: 2 splits, sum must equal 0.00
    const currentSplitAmount = isDeposit ? amountVal : -amountVal;
    const transferSplitAmount = isDeposit ? -amountVal : amountVal;

    const payload: CreateTransactionPayload = {
      post_date: newDate,
      num: newNum.trim() || undefined,
      description: newDescription.trim() || 'Transaction',
      notes: newNotes.trim() || undefined,
      splits: [
        {
          account_id: account.id,
          amount: currentSplitAmount.toFixed(2),
          memo: newMemo.trim() || undefined,
        },
        {
          account_id: newTransferAccountId,
          amount: transferSplitAmount.toFixed(2),
          memo: newMemo.trim() || undefined,
        },
      ],
    };

    try {
      setIsSubmittingNew(true);
      setNewRowError(null);
      await onCreateTransaction(payload);
      handleResetNewRow();
    } catch (err: any) {
      console.error('Failed to create transaction:', err);
      setNewRowError(err.message || 'Failed to record transaction');
    } finally {
      setIsSubmittingNew(false);
    }
  };

  // Mobile save handler
  const handleSaveMobileNewCard = async () => {
    if (!onCreateTransaction) return;

    const amtNum = parseFloat(mobileAmount) || 0;
    if (amtNum <= 0) {
      setNewRowError('Please enter a valid transaction amount');
      return;
    }

    if (!newTransferAccountId) {
      setNewRowError('Please select a transfer account');
      return;
    }

    const isDeposit = mobileAmountType === 'debit';
    const currentSplitAmount = isDeposit ? amtNum : -amtNum;
    const transferSplitAmount = isDeposit ? -amtNum : amtNum;

    const payload: CreateTransactionPayload = {
      post_date: newDate,
      num: newNum.trim() || undefined,
      description: newDescription.trim() || 'Transaction',
      notes: newNotes.trim() || undefined,
      splits: [
        {
          account_id: account.id,
          amount: currentSplitAmount.toFixed(2),
          memo: newMemo.trim() || undefined,
        },
        {
          account_id: newTransferAccountId,
          amount: transferSplitAmount.toFixed(2),
          memo: newMemo.trim() || undefined,
        },
      ],
    };

    try {
      setIsSubmittingNew(true);
      setNewRowError(null);
      await onCreateTransaction(payload);
      handleResetNewRow();
    } catch (err: any) {
      setNewRowError(err.message || 'Failed to record transaction');
    } finally {
      setIsSubmittingNew(false);
    }
  };

  const handleRowKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter' && !isPayeeDropdownOpen) {
      e.preventDefault();
      handleSaveNewRow();
    } else if (e.key === 'Escape') {
      e.preventDefault();
      handleResetNewRow();
    }
  };

  return (
    <div className="space-y-4">
      {/* Account Header Banner */}
      <div className="bg-white rounded-xl border border-slate-200 p-4 sm:p-5 shadow-sm">
        <div className="flex flex-col md:flex-row md:items-center justify-between gap-4">
          <div className="flex items-center space-x-3">
            <div className="w-10 h-10 sm:w-11 sm:h-11 rounded-lg bg-emerald-50 border border-emerald-200 flex items-center justify-center text-emerald-600 shrink-0">
              <BookOpen className="w-5 h-5 sm:w-6 sm:h-6" />
            </div>
            <div className="min-w-0">
              <div className="flex items-center space-x-2 flex-wrap gap-y-1">
                {/* Account Switcher dropdown */}
                <select
                  value={account.id}
                  onChange={(e) => {
                    const selected = allAccounts.find((a) => a.id === e.target.value);
                    if (selected) onSelectAccount(selected);
                  }}
                  className="font-bold text-base sm:text-lg text-slate-900 bg-transparent hover:bg-slate-100 border border-transparent hover:border-slate-300 rounded px-1.5 py-0.5 cursor-pointer focus:outline-none max-w-full"
                >
                  {allAccounts
                    .filter((a) => !a.placeholder)
                    .map((a) => (
                      <option key={a.id} value={a.id}>
                        {a.code ? `${a.code} - ` : ''}
                        {a.name} ({a.account_type}){a.hidden ? ' [Hidden]' : ''}
                      </option>
                    ))}
                </select>
                <span className="text-[11px] sm:text-xs px-2 py-0.5 rounded-md capitalize font-medium bg-slate-100 text-slate-600 border border-slate-200">
                  {account.account_type}
                </span>
                {account.hidden && (
                  <span className="inline-flex items-center space-x-1 text-[11px] sm:text-xs px-2 py-0.5 rounded-md font-medium bg-amber-100 text-amber-800 border border-amber-300">
                    <EyeOff className="w-3 h-3 text-amber-700" />
                    <span>Hidden Account</span>
                  </span>
                )}
              </div>
              <p className="text-[11px] sm:text-xs text-slate-400 mt-0.5 truncate">
                {account.description || `Ledger register for ${account.name}`}
              </p>
            </div>
          </div>

          {/* Current Running Balance Display & Actions */}
          <div className="flex items-center justify-between md:justify-end space-x-3 sm:space-x-4 pt-2 md:pt-0 border-t md:border-t-0 border-slate-100">
            <div className="text-left md:text-right">
              <span className="text-[10px] sm:text-[11px] font-semibold text-slate-400 uppercase tracking-wider block">
                Ending Balance
              </span>
              <span
                className={`text-xl sm:text-2xl font-bold font-mono ${
                  parseFloat(account.display_balance) < 0 ? 'text-rose-600' : 'text-slate-900'
                }`}
              >
                {formatCurrency(account.display_balance, currentCurrency || account.currency)}
              </span>
            </div>

            <div className="flex items-center space-x-1.5 sm:space-x-2">
              {onToggleHideAccount && (
                <button
                  onClick={() => onToggleHideAccount(account)}
                  className={`flex items-center space-x-1.5 px-2.5 sm:px-3 py-1.5 sm:py-2 text-xs font-semibold rounded-lg border transition ${
                    account.hidden
                      ? 'text-amber-800 bg-amber-100 hover:bg-amber-200 border-amber-300'
                      : 'text-slate-600 bg-slate-50 hover:bg-slate-100 border-slate-200'
                  }`}
                  title={account.hidden ? 'Unhide Account (Make visible in tree)' : 'Hide Account (Omit from tree)'}
                >
                  {account.hidden ? <EyeOff className="w-4 h-4 text-amber-700" /> : <Eye className="w-4 h-4 text-slate-500" />}
                  <span className="hidden sm:inline">{account.hidden ? 'Unhide' : 'Hide'}</span>
                </button>
              )}

              {['bank', 'credit', 'cash'].includes(account.account_type.toLowerCase()) && (
                <button
                  onClick={() => onOpenReconcile(account)}
                  className="flex items-center space-x-1.5 px-2.5 sm:px-3 py-1.5 sm:py-2 text-xs font-semibold text-emerald-700 bg-emerald-50 hover:bg-emerald-100 border border-emerald-200 rounded-lg transition"
                >
                  <CheckCircle2 className="w-4 h-4" />
                  <span className="hidden xs:inline">Reconcile</span>
                </button>
              )}

              <button
                onClick={() => onOpenNewTx(account.id)}
                className="flex items-center space-x-1.5 px-3 sm:px-3.5 py-1.5 sm:py-2 text-xs font-semibold text-white bg-emerald-600 hover:bg-emerald-500 rounded-lg transition shadow-sm shrink-0"
              >
                <PlusCircle className="w-4 h-4" />
                <span>Add Entry</span>
              </button>
            </div>
          </div>
        </div>
      </div>

      {/* Register Table Container */}
      <div className="bg-white rounded-xl border border-slate-200 shadow-sm overflow-hidden">
        {/* Table Filter Bar */}
        <div className="p-3 sm:p-3.5 border-b border-slate-200 bg-slate-50/70 flex flex-col sm:flex-row items-stretch sm:items-center justify-between gap-2.5 sm:gap-3">
          <div className="flex items-center space-x-2">
            <span className="text-xs font-semibold text-slate-600 uppercase tracking-wider">
              Ledger Register
            </span>
            <span className="text-xs text-slate-500 font-mono">
              ({filteredEntries.length} {filteredEntries.length === 1 ? 'transaction' : 'transactions'})
            </span>
          </div>

          <div className="relative w-full sm:w-72">
            <Search className="w-3.5 h-3.5 text-slate-400 absolute left-3 top-1/2 -translate-y-1/2" />
            <input
              type="text"
              placeholder="Search in register..."
              value={searchTerm}
              onChange={(e) => setSearchTerm(e.target.value)}
              className="w-full pl-8 pr-3 py-1.5 sm:py-1 text-xs bg-white border border-slate-300 rounded-md focus:outline-none focus:ring-1 focus:ring-emerald-500"
            />
          </div>
        </div>

        {/* Mobile View: Clean Card Stream (Visible on < md) */}
        <div className="md:hidden divide-y divide-slate-100">
          {filteredEntries.length === 0 ? (
            <div className="py-12 text-center text-xs text-slate-400">
              No transactions recorded for this account. Tap "Add Entry" to create one.
            </div>
          ) : (
            filteredEntries.map((entry) => {
              const isExpanded = expandedTxId === entry.transaction_id;
              const isMultiSplit = entry.all_splits.length > 2;
              const hasDeposit = !!entry.debit && parseFloat(entry.debit) > 0;

              return (
                <div key={entry.split_id} className="p-3 space-y-2 hover:bg-slate-50 transition">
                  <div className="flex items-center justify-between text-xs">
                    <div className="flex items-center space-x-1.5">
                      <span className="font-mono text-slate-500 font-semibold">{entry.post_date}</span>
                      {entry.num && (
                        <span className="font-mono text-[10px] bg-slate-100 text-slate-500 px-1 rounded">
                          #{entry.num}
                        </span>
                      )}
                      {getReconciledBadge(entry.reconciled_state)}
                    </div>

                    {/* Amount */}
                    <div className="text-right">
                      {hasDeposit ? (
                        <span className="font-mono font-bold text-emerald-700">
                          +{formatCurrency(entry.debit, currentCurrency || account.currency)}
                        </span>
                      ) : (
                        <span className="font-mono font-bold text-rose-700">
                          -{formatCurrency(entry.credit, currentCurrency || account.currency)}
                        </span>
                      )}
                    </div>
                  </div>

                  <div className="flex items-center justify-between text-xs">
                    <div className="min-w-0 pr-2">
                      <div className="font-bold text-slate-900 truncate">{entry.description}</div>
                      <div className="text-[11px] text-slate-500 truncate mt-0.5">
                        {isMultiSplit ? (
                          <span className="text-emerald-700 font-medium">-- Split ({entry.all_splits.length}) --</span>
                        ) : (
                          entry.transfer_account_name
                        )}
                      </div>
                    </div>

                    <div className="text-right shrink-0">
                      <span className="text-[10px] text-slate-400 block uppercase">Balance</span>
                      <span className="font-mono font-semibold text-slate-700 text-xs">
                        {formatCurrency(entry.running_balance, currentCurrency || account.currency)}
                      </span>
                    </div>
                  </div>

                  {/* Second Row with Details / Memo / Notes */}
                  <div className="bg-slate-50 rounded-lg p-2 border border-slate-100 text-[11px] space-y-1">
                    <div className="flex items-baseline space-x-1.5">
                      <span className="text-[10px] font-semibold text-slate-400 uppercase tracking-wider shrink-0">
                        Notes:
                      </span>
                      <span className={entry.notes ? 'text-slate-700 font-medium' : 'text-slate-400 italic'}>
                        {entry.notes || '—'}
                      </span>
                    </div>
                    <div className="flex items-baseline space-x-1.5">
                      <span className="text-[10px] font-semibold text-slate-400 uppercase tracking-wider shrink-0">
                        Memo:
                      </span>
                      <span className={entry.memo || (entry.all_splits.find(s => s.memo)?.memo) ? 'text-slate-700 font-medium' : 'text-slate-400 italic'}>
                        {entry.memo || (entry.all_splits.find(s => s.memo)?.memo) || '—'}
                      </span>
                    </div>
                  </div>

                  {/* Mobile Actions: Expand splits, Edit, or Delete */}
                  <div className="flex items-center justify-between pt-1 border-t border-slate-100 text-xs">
                    {isMultiSplit ? (
                      <button
                        onClick={() => toggleExpand(entry.transaction_id)}
                        className="inline-flex items-center space-x-1 text-emerald-700 font-medium text-[11px]"
                      >
                        <Split className="w-3 h-3" />
                        <span>{isExpanded ? 'Hide Splits' : 'View Splits'}</span>
                      </button>
                    ) : (
                      <span />
                    )}

                    <div className="flex items-center space-x-1">
                      {onEditTransaction && (
                        <button
                          onClick={() => onEditTransaction(entry.transaction_id)}
                          className="p-1.5 text-slate-400 hover:text-emerald-700 hover:bg-emerald-50 rounded transition"
                          title="Edit Transaction"
                        >
                          <Pencil className="w-3.5 h-3.5" />
                        </button>
                      )}
                      <button
                        onClick={() => {
                          if (confirm('Delete this transaction and all its splits?')) {
                            onDeleteTransaction(entry.transaction_id);
                          }
                        }}
                        className="p-1.5 text-slate-400 hover:text-rose-600 hover:bg-rose-50 rounded transition"
                        title="Delete"
                      >
                        <Trash2 className="w-3.5 h-3.5" />
                      </button>
                    </div>
                  </div>

                  {/* Expanded Splits in Mobile Card */}
                  {isExpanded && (
                    <div className="mt-2 bg-slate-50 rounded-lg p-2.5 border border-slate-200 text-xs space-y-1">
                      <div className="font-semibold text-slate-700 text-[11px] mb-1">
                        Double-Entry Splits:
                      </div>
                      {entry.all_splits.map((s) => {
                        const amt = parseFloat(s.amount);
                        return (
                          <div key={s.id} className="flex justify-between py-0.5 text-[11px] border-b border-slate-100 last:border-none">
                            <span className={s.account_id === account.id ? 'font-bold text-slate-900' : 'text-slate-600'}>
                              {s.account_name}
                            </span>
                            <span className={`font-mono ${amt > 0 ? 'text-emerald-700 font-semibold' : 'text-rose-700'}`}>
                              {formatCurrency(Math.abs(amt), currentCurrency)}
                            </span>
                          </div>
                        );
                      })}
                    </div>
                  )}
                </div>
              );
            })
          )}

          {/* Mobile View: Inline Blank New Transaction Card */}
          <div ref={mobileBottomRef} className="p-3.5 bg-emerald-50/30 border-t-2 border-emerald-500/50 space-y-3">
            <div className="flex items-center justify-between">
              <span className="text-xs font-bold text-emerald-800 flex items-center space-x-1.5">
                <Plus className="w-3.5 h-3.5" />
                <span>New Transaction (Blank Entry)</span>
              </span>
              <span className="text-[10px] uppercase font-bold tracking-wider px-1.5 py-0.5 rounded bg-emerald-100 text-emerald-800 border border-emerald-300">
                2 Splits
              </span>
            </div>

            {newRowError && (
              <div className="flex items-center space-x-1.5 text-xs text-rose-700 bg-rose-50 border border-rose-200 p-2 rounded-lg">
                <AlertCircle className="w-3.5 h-3.5 shrink-0" />
                <span>{newRowError}</span>
              </div>
            )}

            <div className="grid grid-cols-3 gap-2">
              <div className="col-span-2">
                <label className="block text-[10px] font-semibold text-slate-500 mb-0.5">Date</label>
                <input
                  type="date"
                  value={newDate}
                  onChange={(e) => setNewDate(e.target.value)}
                  className="w-full text-xs font-mono py-1.5 px-2 border border-slate-300 rounded-lg bg-white focus:outline-none focus:ring-1 focus:ring-emerald-500"
                />
              </div>
              <div>
                <label className="block text-[10px] font-semibold text-slate-500 mb-0.5">Num</label>
                <input
                  type="text"
                  placeholder="# / Check"
                  value={newNum}
                  onChange={(e) => setNewNum(e.target.value)}
                  className="w-full text-xs font-mono py-1.5 px-2 border border-slate-300 rounded-lg bg-white focus:outline-none focus:ring-1 focus:ring-emerald-500"
                />
              </div>
            </div>

            <div className="relative">
              <label className="block text-[10px] font-semibold text-slate-500 mb-0.5">Payee / Description</label>
              <input
                type="text"
                placeholder="Payee or description..."
                value={newDescription}
                onChange={(e) => {
                  setNewDescription(e.target.value);
                  setIsPayeeDropdownOpen(true);
                }}
                onFocus={() => setIsPayeeDropdownOpen(true)}
                onClick={() => setIsPayeeDropdownOpen(true)}
                className="w-full text-xs py-1.5 px-2.5 border border-slate-300 rounded-lg bg-white focus:outline-none focus:ring-1 focus:ring-emerald-500 font-medium"
              />
              {/* Payee autocomplete suggestions (dropup) */}
              {isPayeeDropdownOpen && filteredPayees.length > 0 && (
                <div data-testid="mobile-payee-autocomplete-dropdown" className="absolute z-50 left-0 right-0 bottom-full mb-1 bg-white border border-slate-200 rounded-xl shadow-xl overflow-hidden max-h-48 overflow-y-auto">
                  <div className="px-2.5 py-1 bg-slate-50 border-b border-slate-200 text-[10px] font-semibold text-slate-500 flex items-center justify-between">
                    <span>Recent Payees ({filteredPayees.length})</span>
                    <span className="text-emerald-700 font-medium">Autofills transfer</span>
                  </div>
                  {filteredPayees.map((item, idx) => (
                    <button
                      key={item.payee}
                      type="button"
                      onClick={() => handleSelectPayee(item)}
                      className={`w-full text-left px-2.5 py-1.5 text-xs flex items-center justify-between hover:bg-emerald-50/80 transition ${
                        idx === payeeHighlightIndex ? 'bg-emerald-50 text-emerald-900 font-medium' : 'text-slate-800'
                      }`}
                    >
                      <span className="truncate">{item.payee}</span>
                      <span className="text-[10px] text-slate-400 font-mono ml-2 shrink-0">{item.count}×</span>
                    </button>
                  ))}
                </div>
              )}
            </div>

            <div>
              <label className="block text-[10px] font-semibold text-slate-500 mb-0.5">Transfer Account (Split 2)</label>
              <AccountAutocomplete
                accounts={transferAccounts}
                value={newTransferAccountId}
                onChange={setNewTransferAccountId}
                placeholder="Select transfer account..."
                dropdownPosition="top"
              />
            </div>

            <div>
              <label className="block text-[10px] font-semibold text-slate-500 mb-0.5">Amount ({currentCurrency || account.currency})</label>
              <div className="flex rounded-lg border border-slate-300 overflow-hidden bg-white focus-within:ring-1 focus-within:ring-emerald-500 focus-within:border-emerald-500">
                <div className="flex bg-slate-100 border-r border-slate-300 p-0.5">
                  <button
                    type="button"
                    onClick={() => setMobileAmountType('debit')}
                    className={`px-2 py-1 text-[11px] font-bold rounded transition ${
                      mobileAmountType === 'debit'
                        ? 'bg-emerald-600 text-white shadow-sm'
                        : 'text-slate-600 hover:text-slate-900'
                    }`}
                  >
                    + {debitHeader.split('/')[0].trim()}
                  </button>
                  <button
                    type="button"
                    onClick={() => setMobileAmountType('credit')}
                    className={`px-2 py-1 text-[11px] font-bold rounded transition ${
                      mobileAmountType === 'credit'
                        ? 'bg-rose-600 text-white shadow-sm'
                        : 'text-slate-600 hover:text-slate-900'
                    }`}
                  >
                    - {creditHeader.split('/')[0].trim()}
                  </button>
                </div>
                <input
                  type="number"
                  step="0.01"
                  min="0"
                  placeholder="0.00"
                  value={mobileAmount}
                  onChange={(e) => setMobileAmount(e.target.value)}
                  className="flex-1 py-1.5 px-2.5 text-xs font-mono font-bold text-slate-800 focus:outline-none"
                />
              </div>
            </div>

            <div className="grid grid-cols-2 gap-2">
              <div>
                <label className="block text-[10px] font-semibold text-slate-500 mb-0.5">Notes (Optional)</label>
                <input
                  type="text"
                  placeholder="Notes..."
                  value={newNotes}
                  onChange={(e) => setNewNotes(e.target.value)}
                  className="w-full text-xs py-1.5 px-2 border border-slate-300 rounded-lg bg-white focus:outline-none focus:ring-1 focus:ring-emerald-500"
                />
              </div>
              <div>
                <label className="block text-[10px] font-semibold text-slate-500 mb-0.5">Memo (Optional)</label>
                <input
                  type="text"
                  placeholder="Memo..."
                  value={newMemo}
                  onChange={(e) => setNewMemo(e.target.value)}
                  className="w-full text-xs py-1.5 px-2 border border-slate-300 rounded-lg bg-white focus:outline-none focus:ring-1 focus:ring-emerald-500"
                />
              </div>
            </div>

            <div className="flex items-center space-x-2 pt-1">
              <button
                type="button"
                onClick={handleSaveMobileNewCard}
                disabled={isSubmittingNew}
                className="flex-1 flex items-center justify-center space-x-1.5 py-2 px-3 text-xs font-bold text-white bg-emerald-600 hover:bg-emerald-500 rounded-lg shadow-sm transition disabled:opacity-50"
              >
                <Check className="w-4 h-4" />
                <span>Record Transaction</span>
              </button>
              <button
                type="button"
                onClick={handleResetNewRow}
                className="py-2 px-3 text-xs font-semibold text-slate-500 hover:text-slate-700 bg-white border border-slate-300 rounded-lg transition"
              >
                Clear
              </button>
            </div>
          </div>
        </div>

        {/* Desktop View: Full Classic GnuCash Table (Visible on >= md) */}
        <div className="hidden md:block overflow-x-auto">
          <table className="w-full text-left border-collapse">
            <thead>
              <tr className="bg-slate-100 text-slate-700 text-xs font-bold border-b border-slate-300 select-none">
                <th className="py-2.5 px-3 w-28">Date</th>
                <th className="py-2.5 px-2 w-20">Num</th>
                <th className="py-2.5 px-3">Description / Payee</th>
                <th className="py-2.5 px-3 w-48">Transfer Account</th>
                <th className="py-2.5 px-2 w-10 text-center" title="Reconciliation Status">R</th>
                <th className="py-2.5 px-3 w-32 text-right">{debitHeader}</th>
                <th className="py-2.5 px-3 w-32 text-right">{creditHeader}</th>
                <th className="py-2.5 px-3 w-36 text-right font-bold bg-slate-200/50">Balance</th>
                <th className="py-2.5 px-2 w-14 text-center">Actions</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-slate-200 font-sans text-xs">
              {filteredEntries.length === 0 ? (
                <tr>
                  <td colSpan={9} className="py-12 text-center text-slate-400">
                    No transactions recorded for this account. Click "Add Entry" to create one.
                  </td>
                </tr>
              ) : (
                filteredEntries.map((entry) => {
                  const isExpanded = expandedTxId === entry.transaction_id;
                  const isMultiSplit = entry.all_splits.length > 2;

                  return (
                    <React.Fragment key={entry.split_id}>
                      <tr
                        className={`hover:bg-amber-50/40 transition-colors ${
                          isExpanded ? 'bg-emerald-50/30' : ''
                        }`}
                      >
                        {/* Date */}
                        <td className="py-2 px-3 font-mono text-slate-600 whitespace-nowrap">
                          {entry.post_date}
                        </td>

                        {/* Num */}
                        <td className="py-2 px-2 font-mono text-slate-500 text-[11px]">
                          {entry.num || ''}
                        </td>

                        {/* Description */}
                        <td className="py-2 px-3 font-medium text-slate-900">
                          <div className="flex items-center space-x-1.5">
                            {isMultiSplit && (
                              <button
                                onClick={() => toggleExpand(entry.transaction_id)}
                                className="p-0.5 text-slate-400 hover:text-emerald-600 rounded"
                                title="Toggle Split breakdown"
                              >
                                {isExpanded ? (
                                  <ChevronUp className="w-3.5 h-3.5" />
                                ) : (
                                  <ChevronDown className="w-3.5 h-3.5" />
                                )}
                              </button>
                            )}
                            <span>{entry.description}</span>
                            {entry.memo && (
                              <span className="text-[11px] text-slate-400 font-normal">
                                ({entry.memo})
                              </span>
                            )}
                          </div>
                        </td>

                        {/* Transfer Account */}
                        <td className="py-2 px-3 text-slate-600 truncate max-w-[200px]">
                          {isMultiSplit ? (
                            <button
                              onClick={() => toggleExpand(entry.transaction_id)}
                              className="inline-flex items-center space-x-1 text-emerald-700 font-semibold hover:underline"
                            >
                              <Split className="w-3 h-3" />
                              <span>-- Split ({entry.all_splits.length}) --</span>
                            </button>
                          ) : (
                            <span className="font-medium">{entry.transfer_account_name}</span>
                          )}
                        </td>

                        {/* Reconciled status */}
                        <td className="py-2 px-2 text-center">
                          {getReconciledBadge(entry.reconciled_state)}
                        </td>

                        {/* Debit */}
                        <td className="py-2 px-3 text-right font-mono text-slate-800">
                          {entry.debit ? formatCurrency(entry.debit, currentCurrency || account.currency) : ''}
                        </td>

                        {/* Credit */}
                        <td className="py-2 px-3 text-right font-mono text-slate-800">
                          {entry.credit ? formatCurrency(entry.credit, currentCurrency || account.currency) : ''}
                        </td>

                        {/* Running Balance */}
                        <td className="py-2 px-3 text-right font-mono font-semibold bg-slate-50/60 text-slate-900">
                          {formatCurrency(entry.running_balance, currentCurrency || account.currency)}
                        </td>

                        {/* Delete action */}
                        <td className="py-2 px-2 text-center">
                          <div className="flex items-center justify-center space-x-1">
                            {onEditTransaction && (
                              <button
                                onClick={() => onEditTransaction(entry.transaction_id)}
                                className="p-1 text-slate-400 hover:text-emerald-700 hover:bg-emerald-50 rounded transition"
                                title="Edit Transaction"
                              >
                                <Pencil className="w-3.5 h-3.5" />
                              </button>
                            )}
                            <button
                              onClick={() => {
                                if (confirm('Delete this transaction and all its splits?')) {
                                  onDeleteTransaction(entry.transaction_id);
                                }
                              }}
                              className="p-1 text-slate-300 hover:text-rose-600 hover:bg-rose-50 rounded transition"
                              title="Delete Transaction"
                            >
                              <Trash2 className="w-3.5 h-3.5" />
                            </button>
                          </div>
                        </td>
                      </tr>

                      {/* Second Row with Details / Memo / Notes text visible */}
                      <tr
                        className={`border-b border-slate-200 text-slate-500 text-[11px] transition-colors ${
                          isExpanded ? 'bg-emerald-50/20' : 'bg-slate-50/50 hover:bg-amber-50/20'
                        }`}
                      >
                        <td className="py-1 px-3 text-slate-400 font-mono text-[10px]"></td>
                        <td className="py-1 px-2 text-slate-400 text-[10px]"></td>
                        <td colSpan={6} className="py-1 px-3">
                          <div className="flex flex-wrap items-center gap-x-4 gap-y-1 text-slate-600">
                            <div className="flex items-center space-x-1.5 min-w-0">
                              <span className="font-semibold text-slate-400 uppercase text-[9px] tracking-wider shrink-0">
                                Notes:
                              </span>
                              <span className={entry.notes ? 'text-slate-700 font-medium truncate' : 'text-slate-400 italic'}>
                                {entry.notes || '—'}
                              </span>
                            </div>
                            <div className="flex items-center space-x-1.5 min-w-0">
                              <span className="font-semibold text-slate-400 uppercase text-[9px] tracking-wider shrink-0">
                                Memo:
                              </span>
                              <span className={entry.memo || (entry.all_splits.find(s => s.memo)?.memo) ? 'text-slate-700 font-medium truncate' : 'text-slate-400 italic'}>
                                {entry.memo || (entry.all_splits.find(s => s.memo)?.memo) || '—'}
                              </span>
                            </div>
                          </div>
                        </td>
                        <td className="py-1 px-2 text-center"></td>
                      </tr>

                      {/* Expanded Splits Row (Journal view inside register) */}
                      {isExpanded && (
                        <tr className="bg-emerald-50/20 border-b border-emerald-100">
                          <td colSpan={9} className="py-3 px-6">
                            <div className="bg-white rounded-lg border border-slate-200 p-3 shadow-inner">
                              <div className="flex items-center space-x-1.5 text-xs font-bold text-slate-700 mb-2">
                                <CornerDownRight className="w-3.5 h-3.5 text-emerald-600" />
                                <span>Full Double-Entry Split Breakdown:</span>
                              </div>

                              <table className="w-full text-xs">
                                <thead>
                                  <tr className="text-slate-500 border-b border-slate-100 font-semibold">
                                    <th className="py-1 text-left">Account</th>
                                    <th className="py-1 text-left">Type</th>
                                    <th className="py-1 text-left">Memo</th>
                                    <th className="py-1 text-center w-12">Rec</th>
                                    <th className="py-1 text-right w-28">Debit</th>
                                    <th className="py-1 text-right w-28">Credit</th>
                                  </tr>
                                </thead>
                                <tbody>
                                  {entry.all_splits.map((split) => {
                                    const amountNum = parseFloat(split.amount);
                                    const debitVal = amountNum > 0 ? amountNum : null;
                                    const creditVal = amountNum < 0 ? -amountNum : null;

                                    return (
                                      <tr
                                        key={split.id}
                                        className={
                                          split.account_id === account.id
                                            ? 'font-bold bg-slate-50'
                                            : 'text-slate-600'
                                        }
                                      >
                                        <td className="py-1">
                                          {split.account_name}
                                          {split.account_id === account.id && (
                                            <span className="ml-1 text-[10px] text-emerald-600 uppercase font-bold">
                                              (Current)
                                            </span>
                                          )}
                                        </td>
                                        <td className="py-1 capitalize text-slate-400">
                                          {split.account_type}
                                        </td>
                                        <td className="py-1 text-slate-400 italic">
                                          {split.memo || '—'}
                                        </td>
                                        <td className="py-1 text-center">
                                          {getReconciledBadge(split.reconciled_state)}
                                        </td>
                                        <td className="py-1 text-right font-mono">
                                          {debitVal !== null ? formatCurrency(debitVal, currentCurrency) : ''}
                                        </td>
                                        <td className="py-1 text-right font-mono">
                                          {creditVal !== null ? formatCurrency(creditVal, currentCurrency) : ''}
                                        </td>
                                      </tr>
                                    );
                                  })}
                                </tbody>
                              </table>
                            </div>
                          </td>
                        </tr>
                      )}
                    </React.Fragment>
                  );
                })
              )}

              {/* Desktop View: Blank New Transaction Row at the End */}
              <tr
                ref={tableBottomRef}
                data-testid="blank-new-row-main"
                className="bg-emerald-50/40 border-t-2 border-emerald-500 hover:bg-emerald-50/60 transition-colors"
              >
                {/* Date */}
                <td className="py-2 px-2.5">
                  <input
                    type="date"
                    value={newDate}
                    onChange={(e) => setNewDate(e.target.value)}
                    onKeyDown={handleRowKeyDown}
                    className="w-full text-xs font-mono py-1 px-1.5 border border-slate-300 rounded focus:border-emerald-500 focus:outline-none bg-white shadow-inner"
                    title="Transaction Date (defaults to today)"
                  />
                </td>

                {/* Num */}
                <td className="py-2 px-1.5">
                  <input
                    type="text"
                    placeholder="Num"
                    value={newNum}
                    onChange={(e) => setNewNum(e.target.value)}
                    onKeyDown={handleRowKeyDown}
                    className="w-full text-xs font-mono py-1 px-1 border border-slate-300 rounded focus:border-emerald-500 focus:outline-none bg-white text-center shadow-inner"
                    title="Transaction Number / Check #"
                  />
                </td>

                {/* Description / Payee with dropup autocomplete */}
                <td className="py-2 px-2.5 relative">
                  <div className="relative" ref={payeeRef}>
                    <input
                      type="text"
                      placeholder="Description / Payee (autocomplete)..."
                      value={newDescription}
                      onChange={(e) => {
                        setNewDescription(e.target.value);
                        setIsPayeeDropdownOpen(true);
                      }}
                      onFocus={() => setIsPayeeDropdownOpen(true)}
                      onClick={() => setIsPayeeDropdownOpen(true)}
                      onKeyDown={(e) => {
                        if (isPayeeDropdownOpen && filteredPayees.length > 0) {
                          if (e.key === 'ArrowDown') {
                            e.preventDefault();
                            setPayeeHighlightIndex((prev) =>
                              prev < filteredPayees.length - 1 ? prev + 1 : 0
                            );
                            return;
                          } else if (e.key === 'ArrowUp') {
                            e.preventDefault();
                            setPayeeHighlightIndex((prev) =>
                              prev > 0 ? prev - 1 : filteredPayees.length - 1
                            );
                            return;
                          } else if (e.key === 'Enter') {
                            e.preventDefault();
                            if (filteredPayees[payeeHighlightIndex]) {
                              handleSelectPayee(filteredPayees[payeeHighlightIndex]);
                            }
                            return;
                          } else if (e.key === 'Escape') {
                            e.preventDefault();
                            setIsPayeeDropdownOpen(false);
                            return;
                          }
                        }
                        handleRowKeyDown(e);
                      }}
                      className="w-full text-xs font-medium py-1 px-2 border border-slate-300 rounded focus:border-emerald-500 focus:outline-none bg-white shadow-inner"
                    />

                    {/* Payee dropup suggestions */}
                    {isPayeeDropdownOpen && filteredPayees.length > 0 && (
                      <div data-testid="payee-autocomplete-dropdown" className="absolute z-50 left-0 right-0 bottom-full mb-1 bg-white border border-slate-200 rounded-xl shadow-2xl overflow-hidden max-h-48 overflow-y-auto">
                        <div className="px-2.5 py-1 bg-slate-50 border-b border-slate-200 text-[10px] font-semibold text-slate-500 flex items-center justify-between">
                          <span>Payee History ({filteredPayees.length})</span>
                          <span className="text-emerald-700 font-medium">Autofills transfer</span>
                        </div>
                        {filteredPayees.map((item, idx) => (
                          <button
                            key={item.payee}
                            type="button"
                            onClick={() => handleSelectPayee(item)}
                            className={`w-full text-left px-2.5 py-1.5 text-xs flex items-center justify-between hover:bg-emerald-50/80 transition ${
                              idx === payeeHighlightIndex ? 'bg-emerald-50 text-emerald-900 font-medium' : 'text-slate-800'
                            }`}
                          >
                            <span className="truncate">{item.payee}</span>
                            <span className="text-[10px] text-slate-400 font-mono ml-2 shrink-0">{item.count}×</span>
                          </button>
                        ))}
                      </div>
                    )}
                  </div>
                </td>

                {/* Transfer Account with dropup autocomplete */}
                <td className="py-2 px-2.5">
                  <AccountAutocomplete
                    accounts={transferAccounts}
                    value={newTransferAccountId}
                    onChange={setNewTransferAccountId}
                    placeholder="Select transfer account..."
                    dropdownPosition="top"
                    className="text-xs"
                  />
                </td>

                {/* Reconcile Status (Fixed 'n') */}
                <td className="py-2 px-1 text-center">
                  <span className="inline-flex items-center px-1.5 py-0.5 rounded text-[10px] font-medium text-slate-400 select-none" title="Not Reconciled (n)">
                    n
                  </span>
                </td>

                {/* Debit (Deposit / Increase) */}
                <td className="py-2 px-2.5 text-right">
                  <input
                    type="number"
                    step="0.01"
                    min="0"
                    placeholder="0.00"
                    value={newDebit}
                    onChange={(e) => {
                      setNewDebit(e.target.value);
                      if (e.target.value) setNewCredit('');
                      setNewRowError(null);
                    }}
                    onKeyDown={handleRowKeyDown}
                    className="w-full text-right text-xs font-mono font-bold text-emerald-700 py-1 px-2 border border-slate-300 rounded focus:border-emerald-500 focus:outline-none bg-white shadow-inner"
                    title={`Enter ${debitHeader}`}
                  />
                </td>

                {/* Credit (Withdrawal / Decrease) */}
                <td className="py-2 px-2.5 text-right">
                  <input
                    type="number"
                    step="0.01"
                    min="0"
                    placeholder="0.00"
                    value={newCredit}
                    onChange={(e) => {
                      setNewCredit(e.target.value);
                      if (e.target.value) setNewDebit('');
                      setNewRowError(null);
                    }}
                    onKeyDown={handleRowKeyDown}
                    className="w-full text-right text-xs font-mono font-bold text-rose-700 py-1 px-2 border border-slate-300 rounded focus:border-emerald-500 focus:outline-none bg-white shadow-inner"
                    title={`Enter ${creditHeader}`}
                  />
                </td>

                {/* Balance placeholder */}
                <td className="py-2 px-3 text-right font-mono text-[11px] text-slate-400 bg-slate-100/50 select-none">
                  (New)
                </td>

                {/* Actions: Save / Enter & Clear */}
                <td className="py-2 px-2 text-center">
                  <div className="flex items-center justify-center space-x-1">
                    <button
                      type="button"
                      onClick={handleSaveNewRow}
                      disabled={isSubmittingNew}
                      className="p-1 text-white bg-emerald-600 hover:bg-emerald-500 rounded transition shadow-sm disabled:opacity-50"
                      title="Record Transaction (Enter)"
                    >
                      <Check className="w-3.5 h-3.5" />
                    </button>
                    <button
                      type="button"
                      onClick={handleResetNewRow}
                      className="p-1 text-slate-400 hover:text-slate-600 rounded transition"
                      title="Clear Row (Esc)"
                    >
                      <X className="w-3.5 h-3.5" />
                    </button>
                  </div>
                </td>
              </tr>

              {/* Desktop View: Blank New Transaction Row 2 (Notes & Memo) */}
              <tr
                data-testid="blank-new-row-notes"
                className="bg-emerald-50/40 border-b-2 border-emerald-500/50"
              >
                <td className="py-1.5 px-3"></td>
                <td className="py-1.5 px-2"></td>
                <td colSpan={6} className="py-1.5 px-2.5">
                  <div className="flex items-center space-x-3">
                    <div className="flex-1 flex items-center space-x-1.5">
                      <span className="text-[10px] font-semibold text-slate-500 uppercase tracking-wider shrink-0">
                        Notes:
                      </span>
                      <input
                        type="text"
                        placeholder="Transaction notes / details..."
                        value={newNotes}
                        onChange={(e) => setNewNotes(e.target.value)}
                        onKeyDown={handleRowKeyDown}
                        className="w-full text-xs py-1 px-2 border border-slate-300 rounded focus:border-emerald-500 focus:outline-none bg-white shadow-inner"
                      />
                    </div>
                    <div className="flex-1 flex items-center space-x-1.5">
                      <span className="text-[10px] font-semibold text-slate-500 uppercase tracking-wider shrink-0">
                        Memo:
                      </span>
                      <input
                        type="text"
                        placeholder="Split memo..."
                        value={newMemo}
                        onChange={(e) => setNewMemo(e.target.value)}
                        onKeyDown={handleRowKeyDown}
                        className="w-full text-xs py-1 px-2 border border-slate-300 rounded focus:border-emerald-500 focus:outline-none bg-white shadow-inner"
                      />
                    </div>
                  </div>
                </td>
                <td className="py-1.5 px-2 text-center"></td>
              </tr>

              {/* Error banner below new row if validation fails */}
              {newRowError && (
                <tr className="bg-rose-50 border-b border-rose-200">
                  <td colSpan={9} className="py-2 px-4 text-xs text-rose-700 font-medium">
                    <div className="flex items-center space-x-1.5">
                      <AlertCircle className="w-3.5 h-3.5 shrink-0" />
                      <span>{newRowError}</span>
                    </div>
                  </td>
                </tr>
              )}
            </tbody>
          </table>
        </div>
      </div>
    </div>
  );
};
