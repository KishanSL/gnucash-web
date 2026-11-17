import { useState } from 'react';
import { Account, Transaction } from '../types';

/** Open/close state for the transaction, account and import modals. */
export function useAppModals() {
  const [isTxModalOpen, setIsTxModalOpen] = useState(false);
  const [defaultTxAccountId, setDefaultTxAccountId] = useState<string | undefined>();
  const [editingTransaction, setEditingTransaction] = useState<Transaction | null>(null);
  const [isAccountModalOpen, setIsAccountModalOpen] = useState(false);
  const [newAccountParentId, setNewAccountParentId] = useState<string | undefined>();
  const [editingAccount, setEditingAccount] = useState<Account | null>(null);
  const [isImportModalOpen, setIsImportModalOpen] = useState(false);

  const openNewTransaction = (accountId?: string) => {
    setDefaultTxAccountId(accountId);
    setEditingTransaction(null);
    setIsTxModalOpen(true);
  };

  const openEditTransaction = (tx: Transaction) => {
    setEditingTransaction(tx);
    setIsTxModalOpen(true);
  };

  const closeTransactionModal = () => {
    setIsTxModalOpen(false);
    setEditingTransaction(null);
  };

  const openNewAccount = (parentId?: string) => {
    setEditingAccount(null);
    setNewAccountParentId(parentId);
    setIsAccountModalOpen(true);
  };

  const openEditAccount = (acc: Account) => {
    setEditingAccount(acc);
    setNewAccountParentId(acc.parent_id || undefined);
    setIsAccountModalOpen(true);
  };

  const closeAccountModal = () => {
    setIsAccountModalOpen(false);
    setEditingAccount(null);
  };

  return {
    isTxModalOpen,
    defaultTxAccountId,
    editingTransaction,
    openNewTransaction,
    openEditTransaction,
    closeTransactionModal,
    isAccountModalOpen,
    newAccountParentId,
    editingAccount,
    openNewAccount,
    openEditAccount,
    closeAccountModal,
    isImportModalOpen,
    openImport: () => setIsImportModalOpen(true),
    closeImport: () => setIsImportModalOpen(false),
  };
}
