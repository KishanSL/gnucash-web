import {
  Account,
  AccountTreeNode,
  BalanceSheetReport,
  CreateAccountPayload,
  CreateTransactionPayload,
  IncomeStatementReport,
  ReconcileData,
  RegisterEntry,
  Transaction,
  TrialBalanceReport,
} from './types';

const API_BASE = '/api';

export async function checkHealth(): Promise<{ status: string; service: string }> {
  const res = await fetch(`${API_BASE}/health`);
  if (!res.ok) throw new Error('Health check failed');
  return res.json();
}

export async function fetchAccounts(): Promise<Account[]> {
  const res = await fetch(`${API_BASE}/accounts`);
  if (!res.ok) throw new Error('Failed to fetch accounts');
  return res.json();
}

export async function fetchAccountTree(): Promise<AccountTreeNode[]> {
  const res = await fetch(`${API_BASE}/accounts/tree`);
  if (!res.ok) throw new Error('Failed to fetch account tree');
  return res.json();
}

export async function createAccount(payload: CreateAccountPayload): Promise<Account> {
  const res = await fetch(`${API_BASE}/accounts`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(payload),
  });
  if (!res.ok) {
    const err = await res.json().catch(() => ({}));
    throw new Error(err.error || 'Failed to create account');
  }
  return res.json();
}

export async function deleteAccount(id: string): Promise<void> {
  const res = await fetch(`${API_BASE}/accounts/${id}`, { method: 'DELETE' });
  if (!res.ok) {
    const err = await res.json().catch(() => ({}));
    throw new Error(err.error || 'Failed to delete account');
  }
}

export async function fetchTransactions(params?: {
  account_id?: string;
  from_date?: string;
  to_date?: string;
  search?: string;
}): Promise<Transaction[]> {
  const query = new URLSearchParams();
  if (params?.account_id) query.set('account_id', params.account_id);
  if (params?.from_date) query.set('from_date', params.from_date);
  if (params?.to_date) query.set('to_date', params.to_date);
  if (params?.search) query.set('search', params.search);

  const res = await fetch(`${API_BASE}/transactions?${query.toString()}`);
  if (!res.ok) throw new Error('Failed to fetch transactions');
  return res.json();
}

export async function fetchAccountRegister(accountId: string): Promise<RegisterEntry[]> {
  const res = await fetch(`${API_BASE}/accounts/${accountId}/register`);
  if (!res.ok) throw new Error('Failed to fetch account register');
  return res.json();
}

export async function createTransaction(payload: CreateTransactionPayload): Promise<Transaction> {
  const res = await fetch(`${API_BASE}/transactions`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(payload),
  });
  if (!res.ok) {
    const err = await res.json().catch(() => ({}));
    throw new Error(err.error || 'Failed to create transaction');
  }
  return res.json();
}

export async function deleteTransaction(id: string): Promise<void> {
  const res = await fetch(`${API_BASE}/transactions/${id}`, { method: 'DELETE' });
  if (!res.ok) {
    const err = await res.json().catch(() => ({}));
    throw new Error(err.error || 'Failed to delete transaction');
  }
}

export async function fetchReconcileInfo(accountId: string, statementDate?: string): Promise<ReconcileData> {
  const query = statementDate ? `?statement_date=${statementDate}` : '';
  const res = await fetch(`${API_BASE}/reconcile/${accountId}${query}`);
  if (!res.ok) throw new Error('Failed to fetch reconcile info');
  return res.json();
}

export async function finishReconciliation(
  accountId: string,
  payload: { statement_date: string; reconciled_split_ids: string[] }
): Promise<{ message: string }> {
  const res = await fetch(`${API_BASE}/reconcile/${accountId}`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(payload),
  });
  if (!res.ok) {
    const err = await res.json().catch(() => ({}));
    throw new Error(err.error || 'Failed to reconcile splits');
  }
  return res.json();
}

export async function fetchBalanceSheet(date?: string): Promise<BalanceSheetReport> {
  const query = date ? `?date=${date}` : '';
  const res = await fetch(`${API_BASE}/reports/balance-sheet${query}`);
  if (!res.ok) throw new Error('Failed to fetch balance sheet');
  return res.json();
}

export async function fetchIncomeStatement(fromDate?: string, toDate?: string): Promise<IncomeStatementReport> {
  const query = new URLSearchParams();
  if (fromDate) query.set('from_date', fromDate);
  if (toDate) query.set('to_date', toDate);

  const res = await fetch(`${API_BASE}/reports/income-statement?${query.toString()}`);
  if (!res.ok) throw new Error('Failed to fetch income statement');
  return res.json();
}

export async function fetchTrialBalance(date?: string): Promise<TrialBalanceReport> {
  const query = date ? `?date=${date}` : '';
  const res = await fetch(`${API_BASE}/reports/trial-balance${query}`);
  if (!res.ok) throw new Error('Failed to fetch trial balance');
  return res.json();
}

export async function seedDatabase(): Promise<{ message: string }> {
  const res = await fetch(`${API_BASE}/seed`, { method: 'POST' });
  if (!res.ok) throw new Error('Failed to seed database');
  return res.json();
}

export async function importGnuCashSqlite(
  file: File,
  wipe: boolean = true,
  defaultCurrency: string = 'INR'
): Promise<{
  success: boolean;
  accounts_imported: number;
  transactions_imported: number;
  splits_imported: number;
  message: string;
}> {
  const formData = new FormData();
  formData.append('file', file);

  const query = new URLSearchParams({
    wipe: wipe ? 'true' : 'false',
    default_currency: defaultCurrency,
  });

  const res = await fetch(`${API_BASE}/import/gnucash-sqlite?${query.toString()}`, {
    method: 'POST',
    body: formData,
  });

  if (!res.ok) {
    const err = await res.json().catch(() => ({}));
    throw new Error(err.error || 'Failed to import GnuCash database');
  }

  return res.json();
}

export const SUPPORTED_CURRENCIES = [
  { code: 'INR', symbol: '₹', name: 'Indian Rupee (INR)' },
  { code: 'USD', symbol: '$', name: 'US Dollar (USD)' },
  { code: 'EUR', symbol: '€', name: 'Euro (EUR)' },
  { code: 'GBP', symbol: '£', name: 'British Pound (GBP)' },
  { code: 'JPY', symbol: '¥', name: 'Japanese Yen (JPY)' },
  { code: 'CAD', symbol: 'C$', name: 'Canadian Dollar (CAD)' },
  { code: 'AUD', symbol: 'A$', name: 'Australian Dollar (AUD)' },
  { code: 'AED', symbol: 'د.إ', name: 'UAE Dirham (AED)' },
  { code: 'SGD', symbol: 'S$', name: 'Singapore Dollar (SGD)' },
  { code: 'CHF', symbol: 'CHF', name: 'Swiss Franc (CHF)' },
];

export function formatCurrency(
  amount: string | number | undefined | null,
  currency: string = 'INR'
): string {
  const currCode = currency ? currency.toUpperCase() : 'INR';
  if (amount === undefined || amount === null || amount === '') {
    return currCode === 'INR' ? '₹0.00' : `${currCode} 0.00`;
  }
  const num = typeof amount === 'string' ? parseFloat(amount) : amount;
  if (isNaN(num)) {
    return currCode === 'INR' ? '₹0.00' : `${currCode} 0.00`;
  }

  // For INR, use en-IN locale which formats lakhs/crores: ₹1,23,456.78
  if (currCode === 'INR') {
    return new Intl.NumberFormat('en-IN', {
      style: 'currency',
      currency: 'INR',
      minimumFractionDigits: 2,
      maximumFractionDigits: 2,
    }).format(num);
  }

  try {
    return new Intl.NumberFormat('en-US', {
      style: 'currency',
      currency: currCode,
      minimumFractionDigits: 2,
      maximumFractionDigits: 2,
    }).format(num);
  } catch {
    // Fallback if unsupported code
    return `${currCode} ${num.toFixed(2)}`;
  }
}
