import { useState, useRef, useEffect, useMemo } from 'react';
import { Account, CreateTransactionPayload, RegisterEntry } from '../../types';
import { getErrorMessage } from '../../api';
import { absAmount, centsToAmount, toCents, twoSplitAmounts } from '../../lib/money';

export interface UseAccountRegisterProps {
  account: Account;
  registerEntries: RegisterEntry[];
  allAccounts: Account[];
  onCreateTransaction?: (payload: CreateTransactionPayload) => Promise<void>;
}

export function useAccountRegister({
  account,
  registerEntries,
  allAccounts,
  onCreateTransaction,
}: UseAccountRegisterProps) {
  const [expandedTxId, setExpandedTxId] = useState<string | null>(null);
  const [searchTerm, setSearchTerm] = useState('');

  const toggleExpand = (txId: string) => {
    setExpandedTxId((prev) => (prev === txId ? null : txId));
  };

  const filteredEntries = useMemo(() => {
    if (!searchTerm) return registerEntries;
    const term = searchTerm.toLowerCase();
    return registerEntries.filter((entry) => {
      return (
        entry.description.toLowerCase().includes(term) ||
        (entry.num && entry.num.toLowerCase().includes(term)) ||
        (entry.transfer_account_name && entry.transfer_account_name.toLowerCase().includes(term)) ||
        (entry.memo && entry.memo.toLowerCase().includes(term))
      );
    });
  }, [registerEntries, searchTerm]);

  // Debit/Credit column header labels based on account type (ACC-7, ACC-3, REG-2)
  const accType = account.account_type.toLowerCase();
  const isAsset = ['asset', 'bank', 'cash', 'receivable'].includes(accType);
  const isExpense = accType === 'expense';
  const debitHeader = isExpense
    ? 'Expense / Increase'
    : isAsset
    ? 'Deposit / Increase'
    : 'Payment / Decrease';
  const creditHeader = isExpense
    ? 'Rebate / Decrease'
    : isAsset
    ? 'Withdrawal / Decrease'
    : 'Charge / Increase';

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
    const debitAmount = absAmount(entry.debit);
    const creditAmount = absAmount(entry.credit);
    if (debitAmount) {
      const val = debitAmount;
      setNewDebit(val);
      setNewCredit('');
      setMobileAmount(val);
      setMobileAmountType('debit');
    } else if (creditAmount) {
      const val = creditAmount;
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

    const debitCents = toCents(newDebit);
    const creditCents = toCents(newCredit);

    if (debitCents <= 0 && creditCents <= 0) {
      setNewRowError(`Please enter an amount in either ${debitHeader} or ${creditHeader}`);
      return;
    }

    if (!newTransferAccountId) {
      setNewRowError('Please select a transfer account');
      return;
    }

    const isDeposit = debitCents > 0;
    const amountVal = centsToAmount(isDeposit ? debitCents : creditCents);

    // Strict double-entry: 2 splits, sum must equal 0.00
    const [currentSplitAmount, transferSplitAmount] = twoSplitAmounts(amountVal, isDeposit);

    const payload: CreateTransactionPayload = {
      post_date: newDate,
      num: newNum.trim() || undefined,
      description: newDescription.trim() || 'Transaction',
      notes: newNotes.trim() || undefined,
      splits: [
        {
          account_id: account.id,
          amount: currentSplitAmount,
          memo: newMemo.trim() || undefined,
        },
        {
          account_id: newTransferAccountId,
          amount: transferSplitAmount,
          memo: newMemo.trim() || undefined,
        },
      ],
    };

    try {
      setIsSubmittingNew(true);
      setNewRowError(null);
      await onCreateTransaction(payload);
      handleResetNewRow();
    } catch (err: unknown) {
      console.error('Failed to create transaction:', err);
      setNewRowError(getErrorMessage(err));
    } finally {
      setIsSubmittingNew(false);
    }
  };

  // Mobile save handler
  const handleSaveMobileNewCard = async () => {
    if (!onCreateTransaction) return;

    if (toCents(mobileAmount) <= 0) {
      setNewRowError('Please enter a valid transaction amount');
      return;
    }

    if (!newTransferAccountId) {
      setNewRowError('Please select a transfer account');
      return;
    }

    const isDeposit = mobileAmountType === 'debit';
    const [currentSplitAmount, transferSplitAmount] = twoSplitAmounts(mobileAmount, isDeposit);

    const payload: CreateTransactionPayload = {
      post_date: newDate,
      num: newNum.trim() || undefined,
      description: newDescription.trim() || 'Transaction',
      notes: newNotes.trim() || undefined,
      splits: [
        {
          account_id: account.id,
          amount: currentSplitAmount,
          memo: newMemo.trim() || undefined,
        },
        {
          account_id: newTransferAccountId,
          amount: transferSplitAmount,
          memo: newMemo.trim() || undefined,
        },
      ],
    };

    try {
      setIsSubmittingNew(true);
      setNewRowError(null);
      await onCreateTransaction(payload);
      handleResetNewRow();
    } catch (err: unknown) {
      setNewRowError(getErrorMessage(err));
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

  return {
    expandedTxId,
    toggleExpand,
    searchTerm,
    setSearchTerm,
    filteredEntries,
    debitHeader,
    creditHeader,
    newDate,
    setNewDate,
    newNum,
    setNewNum,
    newDescription,
    setNewDescription,
    newNotes,
    setNewNotes,
    newMemo,
    setNewMemo,
    newTransferAccountId,
    setNewTransferAccountId,
    newDebit,
    setNewDebit,
    newCredit,
    setNewCredit,
    mobileAmount,
    setMobileAmount,
    mobileAmountType,
    setMobileAmountType,
    isSubmittingNew,
    newRowError,
    setNewRowError,
    isPayeeDropdownOpen,
    setIsPayeeDropdownOpen,
    payeeHighlightIndex,
    setPayeeHighlightIndex,
    payeeRef,
    tableBottomRef,
    mobileBottomRef,
    transferAccounts,
    filteredPayees,
    handleSelectPayee,
    handleResetNewRow,
    handleSaveNewRow,
    handleSaveMobileNewCard,
    handleRowKeyDown,
  };
}
