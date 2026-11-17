import { useCallback, useState } from 'react';
import { Account, AccountTreeNode, RegisterEntry, Transaction } from '../types';
import {
  checkHealth,
  fetchAccountRegister,
  fetchAccounts,
  fetchAccountTree,
  fetchTransactions,
  getStoredToken,
} from '../api';

/** Accounts, account tree, journal and the selected account's register, with loading and refresh. */
export function useLedgerData() {
  const [accounts, setAccounts] = useState<Account[]>([]);
  const [tree, setTree] = useState<AccountTreeNode[]>([]);
  const [transactions, setTransactions] = useState<Transaction[]>([]);
  const [selectedAccount, setSelectedAccount] = useState<Account | null>(null);
  const [registerEntries, setRegisterEntries] = useState<RegisterEntry[]>([]);
  const [backendOnline, setBackendOnline] = useState(false);
  const [loading, setLoading] = useState(true);

  const clearData = useCallback(() => {
    setAccounts([]);
    setTree([]);
    setTransactions([]);
    setSelectedAccount(null);
    setRegisterEntries([]);
  }, []);

  // Load all foundational data for current user
  const loadInitialData = useCallback(async () => {
    if (!getStoredToken()) {
      setLoading(false);
      return;
    }
    try {
      setLoading(true);
      await checkHealth();
      setBackendOnline(true);

      const [accountsData, treeData, txsData] = await Promise.all([
        fetchAccounts(),
        fetchAccountTree(),
        fetchTransactions(),
      ]);

      setAccounts(accountsData);
      setTree(treeData);
      setTransactions(txsData);

      // Default select a Checking or first Bank account
      if (accountsData.length > 0) {
        const defaultAcc =
          accountsData.find(
            (a) => !a.placeholder && a.name.toLowerCase().includes('checking')
          ) || accountsData.find((a) => !a.placeholder);

        if (defaultAcc) {
          setSelectedAccount(defaultAcc);
          const reg = await fetchAccountRegister(defaultAcc.id);
          setRegisterEntries(reg);
        }
      }
    } catch (err: unknown) {
      console.error('Failed to load data from backend:', err);
      setBackendOnline(false);
    } finally {
      setLoading(false);
    }
  }, []);

  // Reload current view
  const refreshData = async () => {
    const [accountsData, treeData, txsData] = await Promise.all([
      fetchAccounts(),
      fetchAccountTree(),
      fetchTransactions(),
    ]);

    setAccounts(accountsData);
    setTree(treeData);
    setTransactions(txsData);

    if (selectedAccount) {
      const updatedSelected = accountsData.find((a) => a.id === selectedAccount.id);
      if (updatedSelected) setSelectedAccount(updatedSelected);
      const reg = await fetchAccountRegister(selectedAccount.id);
      setRegisterEntries(reg);
    }
  };

  // Select an account and load its register
  const openRegister = async (account: Account) => {
    setSelectedAccount(account);
    try {
      const reg = await fetchAccountRegister(account.id);
      setRegisterEntries(reg);
    } catch (err) {
      console.error('Failed to load register:', err);
    }
  };

  return {
    accounts,
    tree,
    transactions,
    selectedAccount,
    setSelectedAccount,
    registerEntries,
    backendOnline,
    loading,
    setLoading,
    clearData,
    loadInitialData,
    refreshData,
    openRegister,
  };
}
