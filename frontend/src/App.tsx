import React, { useState, useEffect, useCallback } from 'react';
import { Navbar, ActiveTab } from './components/Navbar';
import { AccountsTree } from './components/AccountsTree';
import { AccountRegister } from './components/AccountRegister';
import { TransactionsJournal } from './components/TransactionsJournal';
import { TransactionModal } from './components/TransactionModal';
import { NewAccountModal } from './components/NewAccountModal';
import { ReconcileModal } from './components/ReconcileModal';
import { ImportModal } from './components/ImportModal';
import { BalanceSheetView } from './components/reports/BalanceSheetView';
import { IncomeStatementView } from './components/reports/IncomeStatementView';
import { TrialBalanceView } from './components/reports/TrialBalanceView';
import { AuthView } from './components/AuthView';
import {
  Account,
  AccountTreeNode,
  CreateAccountPayload,
  CreateTransactionPayload,
  RegisterEntry,
  Transaction,
  UpdateAccountPayload,
  UpdateTransactionPayload,
  User,
} from './types';
import {
  checkHealth,
  createAccount,
  createTransaction,
  deleteAccount,
  deleteTransaction,
  fetchAccounts,
  fetchAccountTree,
  fetchAccountRegister,
  fetchTransactions,
  fetchTransactionById,
  exportGnucashSqlite,
  seedDatabase,
  resetDataset,
  updateAccount,
  updateTransaction,
  getStoredUser,
  getStoredToken,
  logout,
  fetchCurrentUser,
} from './api';
import { Scale, TrendingUp, BookCheck, AlertCircle } from 'lucide-react';

export const App: React.FC = () => {
  const [activeTab, setActiveTab] = useState<ActiveTab>('accounts');
  const [reportTab, setReportTab] = useState<'balance-sheet' | 'income-statement' | 'trial-balance'>(
    'balance-sheet'
  );

  const [accounts, setAccounts] = useState<Account[]>([]);
  const [tree, setTree] = useState<AccountTreeNode[]>([]);
  const [transactions, setTransactions] = useState<Transaction[]>([]);
  const [selectedAccount, setSelectedAccount] = useState<Account | null>(null);
  const [registerEntries, setRegisterEntries] = useState<RegisterEntry[]>([]);

  // Authentication state
  const [currentUser, setCurrentUser] = useState<User | null>(() => getStoredUser());

  const [backendOnline, setBackendOnline] = useState(false);
  const [loading, setLoading] = useState(true);
  const [statusMessage, setStatusMessage] = useState<string | null>(null);

  // Currency preference (defaults to INR)
  const [currency, setCurrency] = useState<string>(() => {
    return localStorage.getItem('gnucash_currency') || 'INR';
  });

  const handleCurrencyChange = (newCurrency: string) => {
    setCurrency(newCurrency);
    localStorage.setItem('gnucash_currency', newCurrency);
  };

  // Modals
  const [isTxModalOpen, setIsTxModalOpen] = useState(false);
  const [defaultTxAccountId, setDefaultTxAccountId] = useState<string | undefined>();
  const [isAccountModalOpen, setIsAccountModalOpen] = useState(false);
  const [newAccountParentId, setNewAccountParentId] = useState<string | undefined>();
  const [editingAccount, setEditingAccount] = useState<Account | null>(null);
  const [editingTransaction, setEditingTransaction] = useState<Transaction | null>(null);
  const [isImportModalOpen, setIsImportModalOpen] = useState(false);

  // Logout handler
  const handleLogout = useCallback(() => {
    logout();
    setCurrentUser(null);
    setAccounts([]);
    setTree([]);
    setTransactions([]);
    setSelectedAccount(null);
    setRegisterEntries([]);
  }, []);

  // Listen for 401 unauthorized token invalidation
  useEffect(() => {
    const handleAuthInvalid = () => {
      handleLogout();
    };
    window.addEventListener('gnucash_auth_invalid', handleAuthInvalid);
    return () => {
      window.removeEventListener('gnucash_auth_invalid', handleAuthInvalid);
    };
  }, [handleLogout]);

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
    } catch (err: any) {
      console.error('Failed to load data from backend:', err);
      setBackendOnline(false);
    } finally {
      setLoading(false);
    }
  }, []);

  // Sync / verify current user on mount if token exists
  useEffect(() => {
    if (getStoredToken()) {
      fetchCurrentUser()
        .then((user) => {
          setCurrentUser(user);
          loadInitialData();
        })
        .catch(() => {
          handleLogout();
        });
    } else {
      setLoading(false);
    }
  }, [handleLogout, loadInitialData]);

  // Load register when selectedAccount changes
  const handleSelectAccountForRegister = async (account: Account) => {
    setSelectedAccount(account);
    setActiveTab('register');
    try {
      const reg = await fetchAccountRegister(account.id);
      setRegisterEntries(reg);
    } catch (err) {
      console.error('Failed to load register:', err);
    }
  };

  const handleSelectAccountForReconcile = (account: Account) => {
    setSelectedAccount(account);
    setActiveTab('reconcile');
  };

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

  // Create Transaction
  const handleCreateTransaction = async (payload: CreateTransactionPayload) => {
    await createTransaction(payload);
    await refreshData();
  };

  // Open Edit Transaction Modal
  const handleOpenEditTransaction = async (txId: string) => {
    let tx = transactions.find((t) => t.id === txId);
    if (!tx) {
      try {
        tx = await fetchTransactionById(txId);
      } catch (e) {
        console.error('Failed to fetch transaction for edit:', e);
      }
    }
    if (tx) {
      setEditingTransaction(tx);
      setIsTxModalOpen(true);
    }
  };

  // Update Transaction
  const handleUpdateTransaction = async (id: string, payload: UpdateTransactionPayload) => {
    await updateTransaction(id, payload);
    await refreshData();
  };

  // Delete Transaction
  const handleDeleteTransaction = async (txId: string) => {
    await deleteTransaction(txId);
    await refreshData();
  };

  // Create Account
  const handleCreateAccount = async (payload: CreateAccountPayload) => {
    await createAccount(payload);
    await refreshData();
  };

  // Update Account
  const handleUpdateAccount = async (id: string, payload: UpdateAccountPayload) => {
    await updateAccount(id, payload);
    await refreshData();
  };

  // 1-Click Toggle Hidden Status
  const handleToggleHideAccount = async (acc: Account) => {
    try {
      await updateAccount(acc.id, { hidden: !acc.hidden });
      await refreshData();
    } catch (err: any) {
      alert(err.message || 'Failed to update account');
    }
  };

  // Open Edit Account Modal
  const handleOpenEditAccount = (acc: Account) => {
    setEditingAccount(acc);
    setNewAccountParentId(acc.parent_id || undefined);
    setIsAccountModalOpen(true);
  };

  // Delete Account
  const handleDeleteAccount = async (id: string) => {
    if (confirm('Are you sure you want to delete this account?')) {
      await deleteAccount(id);
      await refreshData();
    }
  };

  // Seed Data
  const handleSeedData = async () => {
    if (confirm('Seed standard GnuCash Chart of Accounts and realistic sample transactions?')) {
      try {
        const res = await seedDatabase();
        setStatusMessage(res.message);
        setTimeout(() => setStatusMessage(null), 5000);
        await refreshData();
      } catch (err: any) {
        alert(err.message || 'Failed to seed');
      }
    }
  };

  // Export GnuCash SQLite
  const handleExport = async () => {
    try {
      setStatusMessage('Generating GnuCash SQLite export (.gnucash)...');
      await exportGnucashSqlite();
      setStatusMessage('GnuCash SQLite file exported successfully!');
      setTimeout(() => setStatusMessage(null), 3500);
    } catch (err: any) {
      alert(err.message || 'Export failed');
      setStatusMessage(null);
    }
  };

  // Reset Entire Database
  const handleResetDataset = async () => {
    const confirmed = confirm(
      '⚠️ Reset Entire Database?\n\n' +
      'This will permanently delete ALL accounts, transactions, and journal entries.\n\n' +
      'Are you sure you want to proceed?'
    );
    if (!confirmed) return;

    try {
      setLoading(true);
      const res = await resetDataset();
      setStatusMessage(res.message);
      setTimeout(() => setStatusMessage(null), 5000);
      setSelectedAccount(null);
      setAccounts([]);
      setTree([]);
      setTransactions([]);
      setRegisterEntries([]);
      await refreshData();
    } catch (err: any) {
      alert(err.message || 'Failed to reset database');
    } finally {
      setLoading(false);
    }
  };

  // If user is not authenticated, display login / signup view
  if (!currentUser) {
    return (
      <AuthView
        onAuthSuccess={(user) => {
          setCurrentUser(user);
          loadInitialData();
        }}
      />
    );
  }

  return (
    <div className="min-h-screen flex flex-col bg-slate-50 text-slate-800">
      <Navbar
        activeTab={activeTab}
        setActiveTab={setActiveTab}
        currentUser={currentUser}
        onLogout={handleLogout}
        onOpenNewTx={() => {
          setDefaultTxAccountId(selectedAccount?.id);
          setIsTxModalOpen(true);
        }}
        onOpenNewAccount={() => {
          setEditingAccount(null);
          setNewAccountParentId(undefined);
          setIsAccountModalOpen(true);
        }}
        onOpenImport={() => setIsImportModalOpen(true)}
        onExport={handleExport}
        onSeedData={handleSeedData}
        onResetDataset={handleResetDataset}
        backendOnline={backendOnline}
        selectedAccountName={selectedAccount?.name}
        currency={currency}
        onCurrencyChange={handleCurrencyChange}
      />

      {statusMessage && (
        <div className="bg-emerald-600 text-white text-xs py-2 px-4 text-center font-medium shadow-sm transition">
          {statusMessage}
        </div>
      )}

      {!backendOnline && (
        <div className="bg-rose-500 text-white text-xs py-2 px-4 text-center font-semibold flex items-center justify-center space-x-2">
          <AlertCircle className="w-4 h-4" />
          <span>Backend server disconnected. Please ensure Rust backend is running on port 8082.</span>
        </div>
      )}

      <main className="flex-1 max-w-7xl w-full mx-auto px-3 sm:px-6 lg:px-8 py-4 sm:py-6 pb-24 md:pb-8">
        {loading ? (
          <div className="py-24 text-center text-slate-400 text-sm">
            Connecting to GnuCash Rust backend on port 8082...
          </div>
        ) : (
          <>
            {/* Tab 1: Accounts Tree */}
            {activeTab === 'accounts' && (
              <AccountsTree
                tree={tree}
                currentCurrency={currency}
                onSelectAccountForRegister={handleSelectAccountForRegister}
                onSelectAccountForReconcile={handleSelectAccountForReconcile}
                onDeleteAccount={handleDeleteAccount}
                onOpenNewAccount={(parentId) => {
                  setEditingAccount(null);
                  setNewAccountParentId(parentId);
                  setIsAccountModalOpen(true);
                }}
                onEditAccount={handleOpenEditAccount}
                onToggleHideAccount={handleToggleHideAccount}
              />
            )}

            {/* Tab 2: Register */}
            {activeTab === 'register' && (
              <>
                {selectedAccount ? (
                  <AccountRegister
                    account={selectedAccount}
                    registerEntries={registerEntries}
                    allAccounts={accounts}
                    currentCurrency={currency}
                    onSelectAccount={handleSelectAccountForRegister}
                    onOpenNewTx={(accId) => {
                      setDefaultTxAccountId(accId);
                      setEditingTransaction(null);
                      setIsTxModalOpen(true);
                    }}
                    onOpenReconcile={handleSelectAccountForReconcile}
                    onDeleteTransaction={handleDeleteTransaction}
                    onEditTransaction={handleOpenEditTransaction}
                    onToggleHideAccount={handleToggleHideAccount}
                    onCreateTransaction={handleCreateTransaction}
                  />
                ) : (
                  <div className="py-16 text-center text-slate-400 bg-white rounded-xl border border-slate-200">
                    Please select an account from the Accounts tab to open its ledger register.
                  </div>
                )}
              </>
            )}

            {/* Tab 3: Transactions Journal */}
            {activeTab === 'journal' && (
              <TransactionsJournal
                transactions={transactions}
                currentCurrency={currency}
                onOpenNewTx={() => {
                  setDefaultTxAccountId(undefined);
                  setEditingTransaction(null);
                  setIsTxModalOpen(true);
                }}
                onDeleteTransaction={handleDeleteTransaction}
                onEditTransaction={handleOpenEditTransaction}
              />
            )}

            {/* Tab 4: Reconcile Tool */}
            {activeTab === 'reconcile' && (
              <ReconcileModal
                initialAccount={selectedAccount || undefined}
                allAccounts={accounts}
                currentCurrency={currency}
                onReconciledSuccess={refreshData}
              />
            )}

            {/* Tab 5: Financial Reports */}
            {activeTab === 'reports' && (
              <div className="space-y-6">
                {/* Reports Sub-nav buttons */}
                <div className="flex space-x-2 border-b border-slate-200 pb-3 overflow-x-auto no-scrollbar">
                  <button
                    onClick={() => setReportTab('balance-sheet')}
                    className={`flex items-center space-x-2 px-3.5 py-2 rounded-lg text-xs font-semibold transition ${
                      reportTab === 'balance-sheet'
                        ? 'bg-emerald-600 text-white shadow-sm'
                        : 'bg-white text-slate-600 hover:bg-slate-100 border border-slate-200'
                    }`}
                  >
                    <Scale className="w-4 h-4" />
                    <span>Balance Sheet</span>
                  </button>

                  <button
                    onClick={() => setReportTab('income-statement')}
                    className={`flex items-center space-x-2 px-3.5 py-2 rounded-lg text-xs font-semibold transition ${
                      reportTab === 'income-statement'
                        ? 'bg-emerald-600 text-white shadow-sm'
                        : 'bg-white text-slate-600 hover:bg-slate-100 border border-slate-200'
                    }`}
                  >
                    <TrendingUp className="w-4 h-4" />
                    <span>Income Statement (P&L)</span>
                  </button>

                  <button
                    onClick={() => setReportTab('trial-balance')}
                    className={`flex items-center space-x-2 px-3.5 py-2 rounded-lg text-xs font-semibold transition ${
                      reportTab === 'trial-balance'
                        ? 'bg-emerald-600 text-white shadow-sm'
                        : 'bg-white text-slate-600 hover:bg-slate-100 border border-slate-200'
                    }`}
                  >
                    <BookCheck className="w-4 h-4" />
                    <span>Trial Balance</span>
                  </button>
                </div>

                {reportTab === 'balance-sheet' && <BalanceSheetView currentCurrency={currency} />}
                {reportTab === 'income-statement' && <IncomeStatementView currentCurrency={currency} />}
                {reportTab === 'trial-balance' && <TrialBalanceView currentCurrency={currency} />}
              </div>
            )}
          </>
        )}
      </main>

      {/* Transaction Modal */}
      <TransactionModal
        isOpen={isTxModalOpen}
        onClose={() => {
          setIsTxModalOpen(false);
          setEditingTransaction(null);
        }}
        onSubmit={handleCreateTransaction}
        onUpdate={handleUpdateTransaction}
        editingTransaction={editingTransaction}
        accounts={accounts}
        defaultAccountId={defaultTxAccountId}
        currentCurrency={currency}
        transactions={transactions}
      />

      {/* New / Edit Account Modal */}
      <NewAccountModal
        isOpen={isAccountModalOpen}
        onClose={() => {
          setIsAccountModalOpen(false);
          setEditingAccount(null);
        }}
        onSubmit={handleCreateAccount}
        onUpdate={handleUpdateAccount}
        editingAccount={editingAccount}
        accounts={accounts}
        defaultParentId={newAccountParentId}
        currentCurrency={currency}
      />

      {/* Import GnuCash SQLite Modal */}
      <ImportModal
        isOpen={isImportModalOpen}
        onClose={() => setIsImportModalOpen(false)}
        onImportSuccess={async () => {
          await refreshData();
          setStatusMessage('GnuCash SQLite database imported successfully!');
          setTimeout(() => setStatusMessage(null), 5000);
        }}
        currentCurrency={currency}
      />

      {/* Footer */}
      <footer className="border-t border-slate-200 bg-white py-4 mb-16 md:mb-0 mt-auto">
        <div className="max-w-7xl mx-auto px-4 flex flex-col sm:flex-row items-center justify-between text-xs text-slate-400 gap-2">
          <p>GnuCash Web • Pure Double-Entry Accounting Engine</p>
          <p>Backend: Axum + SQLx + PostgreSQL (Port 8082) • Frontend: React + Vite (Port 8081)</p>
        </div>
      </footer>
    </div>
  );
};
export default App;
