import { useState, useEffect, useMemo, useRef } from 'react';
import { Account, CreateTransactionPayload, Transaction, UpdateTransactionPayload } from '../../types';
import { formatCurrency, getErrorMessage } from '../../api';
import { balancingFields, centsToAmount, netCents, splitTotals, toCents, toDebitCredit } from '../../lib/money';

export interface SplitRowState {
  id: string;
  account_id: string;
  memo: string;
  debit: string;
  credit: string;
  reconciled_state?: string;
  reconciled_date?: string | null;
}

export interface UseTransactionModalProps {
  isOpen: boolean;
  onClose: () => void;
  onSubmit: (payload: CreateTransactionPayload) => Promise<void>;
  onUpdate?: (id: string, payload: UpdateTransactionPayload) => Promise<void>;
  editingTransaction?: Transaction | null;
  accounts: Account[];
  defaultAccountId?: string;
  currentCurrency: string;
  transactions?: Transaction[];
}

export function useTransactionModal({
  isOpen,
  onClose,
  onSubmit,
  onUpdate,
  editingTransaction,
  accounts,
  defaultAccountId,
  currentCurrency,
  transactions = [],
}: UseTransactionModalProps) {
  const [postDate, setPostDate] = useState(new Date().toISOString().split('T')[0]);
  const [num, setNum] = useState('');
  const [description, setDescription] = useState('');
  const [notes, setNotes] = useState('');
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [autofillBanner, setAutofillBanner] = useState<string | null>(null);

  // Payee autocomplete dropdown states
  const [isPayeeDropdownOpen, setIsPayeeDropdownOpen] = useState(false);
  const [payeeHighlightIndex, setPayeeHighlightIndex] = useState(0);
  const payeeRef = useRef<HTMLDivElement>(null);
  const payeeListRef = useRef<HTMLUListElement>(null);

  // Non-placeholder accounts only
  const validAccounts = useMemo(() => accounts.filter((a) => !a.placeholder), [accounts]);

  // Group distinct previous payees mapped to their latest transaction
  const payeeHistory = useMemo(() => {
    if (!transactions || transactions.length === 0) return [];

    // Sort transactions by post_date DESC, then created_at DESC
    const sorted = [...transactions].sort((a, b) => {
      const timeA = new Date(a.post_date).getTime();
      const timeB = new Date(b.post_date).getTime();
      if (timeA !== timeB) return timeB - timeA;
      return (b.created_at || '').localeCompare(a.created_at || '');
    });

    const map = new Map<string, { payee: string; latestTx: Transaction; count: number }>();
    for (const tx of sorted) {
      const p = (tx.description || '').trim();
      if (!p) continue;
      const key = p.toLowerCase();
      if (!map.has(key)) {
        map.set(key, { payee: p, latestTx: tx, count: 1 });
      } else {
        map.get(key)!.count += 1;
      }
    }

    return Array.from(map.values());
  }, [transactions]);

  // Filtered payees matching current description input
  const filteredPayees = useMemo(() => {
    const q = description.trim().toLowerCase();
    if (!q) {
      return payeeHistory.slice(0, 8);
    }
    return payeeHistory
      .filter((item) => item.payee.toLowerCase().includes(q))
      .slice(0, 8);
  }, [payeeHistory, description]);

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

  // Initialize splits
  const [splits, setSplits] = useState<SplitRowState[]>([
    {
      id: '1',
      account_id: defaultAccountId || (validAccounts[0]?.id || ''),
      memo: '',
      debit: '',
      credit: '',
    },
    {
      id: '2',
      account_id: validAccounts[1]?.id || '',
      memo: '',
      debit: '',
      credit: '',
    },
  ]);

  useEffect(() => {
    if (isOpen) {
      if (editingTransaction) {
        setPostDate(editingTransaction.post_date);
        setNum(editingTransaction.num || '');
        setDescription(editingTransaction.description || '');
        setNotes(editingTransaction.notes || '');
        setError(null);
        setAutofillBanner(null);
        setIsPayeeDropdownOpen(false);
        if (editingTransaction.splits && editingTransaction.splits.length >= 2) {
          setSplits(
            editingTransaction.splits.map((s, idx) => {
              return {
                id: s.id || String(idx + 1),
                account_id: s.account_id,
                memo: s.memo || '',
                ...toDebitCredit(s.amount),
                reconciled_state: s.reconciled_state,
                reconciled_date: s.reconciled_date,
              };
            })
          );
        }
      } else {
        setPostDate(new Date().toISOString().split('T')[0]);
        setNum('');
        setDescription('');
        setNotes('');
        setError(null);
        setAutofillBanner(null);
        setIsPayeeDropdownOpen(false);
        setSplits([
          {
            id: '1',
            account_id: defaultAccountId || (validAccounts[0]?.id || ''),
            memo: '',
            debit: '',
            credit: '',
          },
          {
            id: '2',
            account_id: validAccounts[1]?.id || '',
            memo: '',
            debit: '',
            credit: '',
          },
        ]);
      }
    }
  }, [isOpen, defaultAccountId, editingTransaction, validAccounts]);

  const handleSelectPayee = (item: { payee: string; latestTx: Transaction; count: number }) => {
    setDescription(item.payee);
    if (item.latestTx.notes) {
      setNotes(item.latestTx.notes);
    }

    // Autofill splits from latest transaction of this payee
    if (item.latestTx.splits && item.latestTx.splits.length >= 2) {
      const newSplits: SplitRowState[] = item.latestTx.splits.map((s) => {
        const exists = validAccounts.some((a) => a.id === s.account_id);
        const finalAccId = exists ? s.account_id : validAccounts[0]?.id || '';
        const { debit, credit } = toDebitCredit(s.amount);

        return {
          id: Math.random().toString(),
          account_id: finalAccId,
          memo: s.memo || '',
          debit,
          credit,
        };
      });

      setSplits(newSplits);
      setAutofillBanner(
        `Autofilled ${newSplits.length} splits and details from last transaction (${item.latestTx.post_date})`
      );
    }

    setIsPayeeDropdownOpen(false);
  };

  const handlePayeeKeyDown = (e: React.KeyboardEvent) => {
    if (!isPayeeDropdownOpen || filteredPayees.length === 0) {
      if (e.key === 'ArrowDown' && filteredPayees.length > 0) {
        e.preventDefault();
        setIsPayeeDropdownOpen(true);
      }
      return;
    }

    if (e.key === 'ArrowDown') {
      e.preventDefault();
      setPayeeHighlightIndex((prev) =>
        prev < filteredPayees.length - 1 ? prev + 1 : 0
      );
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      setPayeeHighlightIndex((prev) =>
        prev > 0 ? prev - 1 : filteredPayees.length - 1
      );
    } else if (e.key === 'Enter') {
      if (filteredPayees[payeeHighlightIndex]) {
        e.preventDefault();
        handleSelectPayee(filteredPayees[payeeHighlightIndex]);
      }
    } else if (e.key === 'Escape') {
      e.preventDefault();
      setIsPayeeDropdownOpen(false);
    }
  };

  const handleSplitChange = (index: number, field: keyof SplitRowState, value: string) => {
    setSplits((prev) => {
      const next = [...prev];
      if (field === 'debit' && value) {
        next[index] = { ...next[index], debit: value, credit: '' };
      } else if (field === 'credit' && value) {
        next[index] = { ...next[index], credit: value, debit: '' };
      } else {
        next[index] = { ...next[index], [field]: value };
      }
      return next;
    });
  };

  const addSplitRow = () => {
    setSplits((prev) => [
      ...prev,
      {
        id: Math.random().toString(),
        account_id: validAccounts[0]?.id || '',
        memo: '',
        debit: '',
        credit: '',
      },
    ]);
  };

  const removeSplitRow = (index: number) => {
    if (splits.length <= 2) {
      alert('A double-entry transaction must contain at least 2 splits.');
      return;
    }
    setSplits((prev) => prev.filter((_, i) => i !== index));
  };

  // Calculate totals in integer cents, from the same rounded amounts that are submitted
  const { totalDebitCents, totalCreditCents, imbalanceCents, isBalanced } = splitTotals(splits);
  const totalDebits = totalDebitCents / 100;
  const totalCredits = totalCreditCents / 100;
  const imbalance = imbalanceCents / 100;

  // Auto-balance button: adjusts or fills the remaining difference
  const handleAutoBalance = () => {
    if (imbalanceCents === 0) return;

    setSplits((prev) => {
      const next = [...prev];
      const lastIndex = next.length - 1;

      // Adjust the last row's net amount by the imbalance (works whichever side it is on)
      next[lastIndex] = { ...next[lastIndex], ...balancingFields(next[lastIndex], imbalanceCents) };
      return next;
    });
  };

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setError(null);

    if (!description.trim()) {
      setError('Please provide a description or payee for the transaction.');
      return;
    }

    if (splits.length < 2) {
      setError('A transaction must have at least 2 splits.');
      return;
    }

    if (!isBalanced) {
      setError(
        `Transaction is not balanced! Total Debits (${formatCurrency(
          totalDebits,
          currentCurrency
        )}) must equal Total Credits (${formatCurrency(totalCredits, currentCurrency)}). Imbalance: ${formatCurrency(
          imbalance,
          currentCurrency
        )}`
      );
      return;
    }

    // Build payload ensuring 2 decimal places (ACC-3)
    const payloadSplits = splits.map((s) => {
      const amountCents = netCents(s.debit, s.credit);

      // Find original split from editingTransaction
      const origSplit = editingTransaction?.splits?.find((orig) => orig.id === s.id);
      const isUnchanged =
        origSplit &&
        origSplit.account_id === s.account_id &&
        toCents(origSplit.amount) === amountCents;

      return {
        id: s.id && s.id.length === 36 ? s.id : undefined,
        account_id: s.account_id,
        amount: centsToAmount(amountCents),
        memo: s.memo.trim() ? s.memo.trim() : null,
        reconciled_state: isUnchanged ? s.reconciled_state : 'n',
      };
    });

    try {
      setSubmitting(true);
      if (editingTransaction && onUpdate) {
        await onUpdate(editingTransaction.id, {
          post_date: postDate,
          num: num.trim() ? num.trim() : null,
          description: description.trim(),
          notes: notes.trim() ? notes.trim() : null,
          splits: payloadSplits,
        });
      } else {
        await onSubmit({
          post_date: postDate,
          num: num.trim() ? num.trim() : null,
          description: description.trim(),
          notes: notes.trim() ? notes.trim() : null,
          splits: payloadSplits,
        });
      }
      onClose();
    } catch (err: unknown) {
      setError(getErrorMessage(err));
    } finally {
      setSubmitting(false);
    }
  };

  return {
    postDate,
    setPostDate,
    num,
    setNum,
    description,
    setDescription,
    notes,
    setNotes,
    submitting,
    error,
    setError,
    autofillBanner,
    setAutofillBanner,
    isPayeeDropdownOpen,
    setIsPayeeDropdownOpen,
    payeeHighlightIndex,
    setPayeeHighlightIndex,
    payeeRef,
    payeeListRef,
    validAccounts,
    payeeHistory,
    filteredPayees,
    handleSelectPayee,
    handlePayeeKeyDown,
    splits,
    handleSplitChange,
    addSplitRow,
    removeSplitRow,
    totalDebits,
    totalCredits,
    imbalance,
    isBalanced,
    handleAutoBalance,
    handleSubmit,
  };
}
