import React, { useState } from 'react';
import { Navbar, ActiveTab } from './components/Navbar';
import { AccountsTree } from './components/AccountsTree';
import { AccountRegister } from './components/AccountRegister';
import { TransactionsJournal } from './components/TransactionsJournal';
import { TransactionModal } from './components/TransactionModal';
import { NewAccountModal } from './components/NewAccountModal';
import { ReconcileModal } from './components/ReconcileModal';
import { ImportModal } from './components/ImportModal';
import { ReportsView, ReportTab } from './components/reports/ReportsView';
import { AuthView } from './components/AuthView';
import {
  Account,
  CreateAccountPayload,
  CreateTransactionPayload,
  UpdateAccountPayload,
  UpdateTransactionPayload,
} from './types';
import {
  createAccount,
  createTransaction,
  deleteAccount,
  deleteTransaction,
  fetchTransactionById,
  exportGnucashSqlite,
  seedDatabase,
  resetDataset,
  updateAccount,
  updateTransaction,
  getErrorMessage,
} from './api';
import { AlertCircle } from 'lucide-react';
import { useLedgerData } from './app/useLedgerData';
import { useAuthSession } from './app/useAuthSession';
import { useAppModals } from './app/useAppModals';
import { useStatusMessage } from './app/useStatusMessage';
import { useCurrencyPreference } from './app/useCurrencyPreference';

export const App: React.FC = () => {
  const [activeTab, setActiveTab] = useState<ActiveTab>('accounts');
  const [reportTab, setReportTab] = useState<ReportTab>('balance-sheet');

  const ledger = useLedgerData();
  const {
    accounts,
    tree,
    transactions,
    selectedAccount,
    registerEntries,
    backendOnline,
    loading,
    setLoading,
    refreshData,
  } = ledger;
  const { currentUser, handleLogout, handleAuthSuccess } = useAuthSession({
    clearData: ledger.clearData,
    loadInitialData: ledger.loadInitialData,
    setLoading,
  });
  const modals = useAppModals();
  const { statusMessage, setStatusMessage, flashStatus } = useStatusMessage();
  const { currency, changeCurrency } = useCurrencyPreference();

  const handleSelectAccountForRegister = async (account: Account) => {
    setActiveTab('register');
    await ledger.openRegister(account);
  };

  const handleSelectAccountForReconcile = (account: Account) => {
    ledger.setSelectedAccount(account);
    setActiveTab('reconcile');
  };

  const handleCreateTransaction = async (payload: CreateTransactionPayload) => {
    await createTransaction(payload);
    await refreshData();
  };

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
      modals.openEditTransaction(tx);
    }
  };

  const handleUpdateTransaction = async (id: string, payload: UpdateTransactionPayload) => {
    await updateTransaction(id, payload);
    await refreshData();
  };

  const handleDeleteTransaction = async (txId: string) => {
    await deleteTransaction(txId);
    await refreshData();
  };

  const handleCreateAccount = async (payload: CreateAccountPayload) => {
    await createAccount(payload);
    await refreshData();
  };

  const handleUpdateAccount = async (id: string, payload: UpdateAccountPayload) => {
    await updateAccount(id, payload);
    await refreshData();
  };

  // 1-Click Toggle Hidden Status
  const handleToggleHideAccount = async (acc: Account) => {
    try {
      await updateAccount(acc.id, { hidden: !acc.hidden });
      await refreshData();
    } catch (err: unknown) {
      alert(getErrorMessage(err));
    }
  };

  const handleDeleteAccount = async (id: string) => {
    if (confirm('Are you sure you want to delete this account?')) {
      await deleteAccount(id);
      await refreshData();
    }
  };

  const handleSeedData = async () => {
    if (confirm('Seed standard GnuCash Chart of Accounts and realistic sample transactions?')) {
      try {
        const res = await seedDatabase();
        flashStatus(res.message, 5000);
        await refreshData();
      } catch (err: unknown) {
        alert(getErrorMessage(err));
      }
    }
  };

  const handleExport = async () => {
    try {
      setStatusMessage('Generating GnuCash SQLite export (.gnucash)...');
      await exportGnucashSqlite();
      flashStatus('GnuCash SQLite file exported successfully!', 3500);
    } catch (err: unknown) {
      alert(getErrorMessage(err));
      setStatusMessage(null);
    }
  };

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
      flashStatus(res.message, 5000);
      ledger.clearData();
      await refreshData();
    } catch (err: unknown) {
      alert(getErrorMessage(err));
    } finally {
      setLoading(false);
    }
  };

  // If user is not authenticated, display login / signup view
  if (!currentUser) {
    return <AuthView onAuthSuccess={handleAuthSuccess} />;
  }

  return (
    <div className="min-h-screen flex flex-col bg-slate-50 text-slate-800">
      <Navbar
        activeTab={activeTab}
        setActiveTab={setActiveTab}
        currentUser={currentUser}
        onLogout={handleLogout}
        onOpenNewTx={() => modals.openNewTransaction(selectedAccount?.id)}
        onOpenNewAccount={() => modals.openNewAccount()}
        onOpenImport={modals.openImport}
        onExport={handleExport}
        onSeedData={handleSeedData}
        onResetDataset={handleResetDataset}
        backendOnline={backendOnline}
        selectedAccountName={selectedAccount?.name}
        currency={currency}
        onCurrencyChange={changeCurrency}
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
                onOpenNewAccount={modals.openNewAccount}
                onEditAccount={modals.openEditAccount}
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
                    onOpenNewTx={modals.openNewTransaction}
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
                onOpenNewTx={() => modals.openNewTransaction()}
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
              <ReportsView reportTab={reportTab} onReportTabChange={setReportTab} currentCurrency={currency} />
            )}
          </>
        )}
      </main>

      {/* Transaction Modal */}
      <TransactionModal
        isOpen={modals.isTxModalOpen}
        onClose={modals.closeTransactionModal}
        onSubmit={handleCreateTransaction}
        onUpdate={handleUpdateTransaction}
        editingTransaction={modals.editingTransaction}
        accounts={accounts}
        defaultAccountId={modals.defaultTxAccountId}
        currentCurrency={currency}
        transactions={transactions}
      />

      {/* New / Edit Account Modal */}
      <NewAccountModal
        isOpen={modals.isAccountModalOpen}
        onClose={modals.closeAccountModal}
        onSubmit={handleCreateAccount}
        onUpdate={handleUpdateAccount}
        editingAccount={modals.editingAccount}
        accounts={accounts}
        defaultParentId={modals.newAccountParentId}
        currentCurrency={currency}
      />

      {/* Import GnuCash SQLite Modal */}
      <ImportModal
        isOpen={modals.isImportModalOpen}
        onClose={modals.closeImport}
        onImportSuccess={async () => {
          await refreshData();
          flashStatus('GnuCash SQLite database imported successfully!', 5000);
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
