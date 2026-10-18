import {
  Account,
  AccountTreeNode,
  AuthResponse,
  BalanceSheetReport,
  CreateAccountPayload,
  CreateTransactionPayload,
  IncomeStatementReport,
  LoginPayload,
  ReconcileData,
  RegisterEntry,
  SignupPayload,
  Transaction,
  TrialBalanceReport,
  UpdateAccountPayload,
  User,
} from './types';

const API_BASE = '/api';

// --- Auth Token & LocalStorage Management ---

export function getStoredToken(): string | null {
  return localStorage.getItem('gnucash_token');
}

export function setStoredToken(token: string | null) {
  if (token) {
    localStorage.setItem('gnucash_token', token);
  } else {
    localStorage.removeItem('gnucash_token');
  }
}

export function getStoredUser(): User | null {
  const u = localStorage.getItem('gnucash_user');
  if (!u) return null;
  try {
    return JSON.parse(u);
  } catch {
    return null;
  }
}

export function setStoredUser(user: User | null) {
  if (user) {
    localStorage.setItem('gnucash_user', JSON.stringify(user));
  } else {
    localStorage.removeItem('gnucash_user');
  }
}

export async function fetchWithAuth(input: string, init?: RequestInit): Promise<Response> {
  const token = getStoredToken();
  const headers = new Headers(init?.headers || {});

  if (token) {
    headers.set('Authorization', `Bearer ${token}`);
  }

  const res = await fetch(input, { ...init, headers });

  if (res.status === 401) {
    setStoredToken(null);
    setStoredUser(null);
    window.dispatchEvent(new Event('gnucash_auth_invalid'));
  }

  return res;
}

// --- Auth API Endpoints ---

export async function signup(payload: SignupPayload): Promise<AuthResponse> {
  const res = await fetch(`${API_BASE}/auth/signup`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(payload),
  });
  if (!res.ok) {
    const err = await res.json().catch(() => ({}));
    throw new Error(err.error || 'Signup failed');
  }
  const data: AuthResponse = await res.json();
  setStoredToken(data.token);
  setStoredUser(data.user);
  return data;
}

export async function login(payload: LoginPayload): Promise<AuthResponse> {
  const res = await fetch(`${API_BASE}/auth/login`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(payload),
  });
  if (!res.ok) {
    const err = await res.json().catch(() => ({}));
    throw new Error(err.error || 'Login failed');
  }
  const data: AuthResponse = await res.json();
  setStoredToken(data.token);
  setStoredUser(data.user);
  return data;
}

export async function resetPassword(payload: { username: string; new_password: string }): Promise<{ success: boolean; message: string }> {
  const res = await fetch(`${API_BASE}/auth/reset-password`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(payload),
  });
  if (!res.ok) {
    const err = await res.json().catch(() => ({}));
    throw new Error(err.error || 'Failed to reset password');
  }
  return res.json();
}

export async function changePassword(payload: {
  current_password?: string;
  new_password: string;
}): Promise<{ success: boolean; message: string }> {
  const res = await fetchWithAuth(`${API_BASE}/auth/change-password`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(payload),
  });
  if (!res.ok) {
    const err = await res.json().catch(() => ({}));
    throw new Error(err.error || 'Failed to change password');
  }
  return res.json();
}

export async function fetchCurrentUser(): Promise<User> {
  const res = await fetchWithAuth(`${API_BASE}/auth/me`);
  if (!res.ok) throw new Error('Failed to fetch user');
  const user: User = await res.json();
  setStoredUser(user);
  return user;
}

export function logout() {
  setStoredToken(null);
  setStoredUser(null);
  window.dispatchEvent(new Event('gnucash_auth_invalid'));
}

// --- General Health ---

export async function checkHealth(): Promise<{ status: string; service: string }> {
  const res = await fetch(`${API_BASE}/health`);
  if (!res.ok) throw new Error('Health check failed');
  return res.json();
}

// --- Accounts ---

export async function fetchAccounts(): Promise<Account[]> {
  const res = await fetchWithAuth(`${API_BASE}/accounts`);
  if (!res.ok) throw new Error('Failed to fetch accounts');
  return res.json();
}

export async function fetchAccountTree(): Promise<AccountTreeNode[]> {
  const res = await fetchWithAuth(`${API_BASE}/accounts/tree`);
  if (!res.ok) throw new Error('Failed to fetch account tree');
  return res.json();
}

export async function createAccount(payload: CreateAccountPayload): Promise<Account> {
  const res = await fetchWithAuth(`${API_BASE}/accounts`, {
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

export async function updateAccount(
  id: string,
  payload: UpdateAccountPayload
): Promise<Account> {
  const res = await fetchWithAuth(`${API_BASE}/accounts/${id}`, {
    method: 'PUT',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(payload),
  });
  if (!res.ok) {
    const err = await res.json().catch(() => ({}));
    throw new Error(err.error || 'Failed to update account');
  }
  return res.json();
}

export async function deleteAccount(id: string): Promise<void> {
  const res = await fetchWithAuth(`${API_BASE}/accounts/${id}`, { method: 'DELETE' });
  if (!res.ok) {
    const err = await res.json().catch(() => ({}));
    throw new Error(err.error || 'Failed to delete account');
  }
}

// --- Transactions ---

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

  const res = await fetchWithAuth(`${API_BASE}/transactions?${query.toString()}`);
  if (!res.ok) throw new Error('Failed to fetch transactions');
  return res.json();
}

export async function fetchAccountRegister(accountId: string): Promise<RegisterEntry[]> {
  const res = await fetchWithAuth(`${API_BASE}/accounts/${accountId}/register`);
  if (!res.ok) throw new Error('Failed to fetch account register');
  return res.json();
}

export async function createTransaction(payload: CreateTransactionPayload): Promise<Transaction> {
  const res = await fetchWithAuth(`${API_BASE}/transactions`, {
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
  const res = await fetchWithAuth(`${API_BASE}/transactions/${id}`, { method: 'DELETE' });
  if (!res.ok) {
    const err = await res.json().catch(() => ({}));
    throw new Error(err.error || 'Failed to delete transaction');
  }
}

// --- Reconcile ---

export async function fetchReconcileInfo(accountId: string, statementDate?: string): Promise<ReconcileData> {
  const query = statementDate ? `?statement_date=${statementDate}` : '';
  const res = await fetchWithAuth(`${API_BASE}/reconcile/${accountId}${query}`);
  if (!res.ok) throw new Error('Failed to fetch reconcile info');
  return res.json();
}

export async function finishReconciliation(
  accountId: string,
  payload: { statement_date: string; reconciled_split_ids: string[] }
): Promise<{ message: string }> {
  const res = await fetchWithAuth(`${API_BASE}/reconcile/${accountId}`, {
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

// --- Reports ---

export async function fetchBalanceSheet(date?: string): Promise<BalanceSheetReport> {
  const query = date ? `?date=${date}` : '';
  const res = await fetchWithAuth(`${API_BASE}/reports/balance-sheet${query}`);
  if (!res.ok) throw new Error('Failed to fetch balance sheet');
  return res.json();
}

export async function fetchIncomeStatement(fromDate?: string, toDate?: string): Promise<IncomeStatementReport> {
  const query = new URLSearchParams();
  if (fromDate) query.set('from_date', fromDate);
  if (toDate) query.set('to_date', toDate);

  const res = await fetchWithAuth(`${API_BASE}/reports/income-statement?${query.toString()}`);
  if (!res.ok) throw new Error('Failed to fetch income statement');
  return res.json();
}

export async function fetchTrialBalance(date?: string): Promise<TrialBalanceReport> {
  const query = date ? `?date=${date}` : '';
  const res = await fetchWithAuth(`${API_BASE}/reports/trial-balance${query}`);
  if (!res.ok) throw new Error('Failed to fetch trial balance');
  return res.json();
}

// --- Seed & Import ---

export async function resetDataset(): Promise<{ success: boolean; message: string }> {
  const res = await fetchWithAuth(`${API_BASE}/reset-data`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
  });
  if (!res.ok) {
    const err = await res.json().catch(() => ({}));
    throw new Error(err.error || 'Failed to reset dataset');
  }
  return res.json();
}

export async function seedDatabase(): Promise<{ message: string }> {
  const res = await fetchWithAuth(`${API_BASE}/seed`, { method: 'POST' });
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

  const res = await fetchWithAuth(`${API_BASE}/import/gnucash-sqlite?${query.toString()}`, {
    method: 'POST',
    body: formData,
  });

  if (!res.ok) {
    const err = await res.json().catch(() => ({}));
    throw new Error(err.error || 'Failed to import GnuCash database');
  }

  return res.json();
}

export interface LocalGnuCashFile {
  path: string;
  filename: string;
  size_bytes: number;
}

export async function fetchLocalGnuCashFiles(): Promise<LocalGnuCashFile[]> {
  const res = await fetchWithAuth(`${API_BASE}/import/local-files`);
  if (!res.ok) return [];
  const data = await res.json().catch(() => ({ files: [] }));
  return data.files || [];
}

export async function importFromPath(
  path: string,
  wipe: boolean = true,
  defaultCurrency: string = 'INR'
): Promise<{
  success: boolean;
  accounts_imported: number;
  transactions_imported: number;
  splits_imported: number;
  message: string;
}> {
  const res = await fetchWithAuth(`${API_BASE}/import/from-path`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ path, wipe, default_currency: defaultCurrency }),
  });
  if (!res.ok) {
    const err = await res.json().catch(() => ({}));
    throw new Error(err.error || 'Failed to import GnuCash file from path');
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
    return `${currCode} ${num.toFixed(2)}`;
  }
}
