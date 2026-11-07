export interface User {
  id: string;
  username: string;
  email: string;
  created_at: string;
  updated_at: string;
}

export interface AuthResponse {
  token: string;
  user: User;
}

export interface LoginPayload {
  username: string;
  password: string;
}

export interface SignupPayload {
  username: string;
  email: string;
  password: string;
}

export type AccountType =
  | 'asset'
  | 'bank'
  | 'cash'
  | 'receivable'
  | 'liability'
  | 'credit'
  | 'payable'
  | 'equity'
  | 'income'
  | 'expense';

export interface Account {
  id: string;
  name: string;
  code?: string | null;
  account_type: string;
  parent_id?: string | null;
  currency: string;
  description?: string | null;
  placeholder: boolean;
  hidden: boolean;
  balance: string;
  total_balance: string;
  display_balance: string;
  created_at: string;
  updated_at: string;
}

export interface AccountTreeNode {
  account: Account;
  children: AccountTreeNode[];
}

export interface CreateAccountPayload {
  name: string;
  code?: string | null;
  account_type: string;
  parent_id?: string | null;
  currency?: string;
  description?: string | null;
  placeholder?: boolean;
  hidden?: boolean;
}

export interface UpdateAccountPayload {
  name?: string;
  code?: string | null;
  account_type?: string;
  parent_id?: string | null;
  currency?: string;
  description?: string | null;
  placeholder?: boolean;
  hidden?: boolean;
}

export interface SplitWithAccount {
  id: string;
  transaction_id: string;
  account_id: string;
  account_name: string;
  account_type: string;
  amount: string;
  memo?: string | null;
  reconciled_state: 'n' | 'c' | 'y';
  reconciled_date?: string | null;
}

export interface Transaction {
  id: string;
  post_date: string;
  enter_date: string;
  num?: string | null;
  description: string;
  notes?: string | null;
  splits: SplitWithAccount[];
  created_at: string;
  updated_at: string;
}

export interface CreateSplitPayload {
  account_id: string;
  amount: string;
  memo?: string | null;
  reconciled_state?: string;
}

export interface CreateTransactionPayload {
  post_date: string;
  num?: string | null;
  description: string;
  notes?: string | null;
  splits: CreateSplitPayload[];
}

export interface UpdateTransactionPayload {
  post_date?: string;
  num?: string | null;
  description?: string;
  notes?: string | null;
  splits?: CreateSplitPayload[];
}

export interface RegisterEntry {
  transaction_id: string;
  split_id: string;
  post_date: string;
  num?: string | null;
  description: string;
  notes?: string | null;
  transfer_account_name: string;
  memo?: string | null;
  reconciled_state: 'n' | 'c' | 'y';
  amount: string;
  debit?: string | null;
  credit?: string | null;
  running_balance: string;
  all_splits: SplitWithAccount[];
}

export interface ReconcileSplitItem {
  split_id: string;
  transaction_id: string;
  post_date: string;
  num?: string | null;
  description: string;
  amount: string;
  display_amount: string;
  reconciled_state: string;
}

export interface ReconcileData {
  account: Account;
  statement_date: string;
  starting_balance: string;
  ending_balance: string;
  deposits: ReconcileSplitItem[];
  withdrawals: ReconcileSplitItem[];
}

export interface BalanceSheetItem {
  account_id: string;
  name: string;
  code?: string | null;
  account_type: string;
  depth: number;
  balance: string;
}

export interface BalanceSheetSection {
  title: string;
  items: BalanceSheetItem[];
  total: string;
}

export interface BalanceSheetReport {
  as_of_date: string;
  assets: BalanceSheetSection;
  liabilities: BalanceSheetSection;
  equity: BalanceSheetSection;
  net_income: string;
  total_liabilities_and_equity: string;
  is_balanced: boolean;
}

export interface IncomeStatementItem {
  account_id: string;
  name: string;
  code?: string | null;
  account_type: string;
  depth: number;
  amount: string;
}

export interface IncomeStatementSection {
  title: string;
  items: IncomeStatementItem[];
  total: string;
}

export interface IncomeStatementReport {
  from_date: string;
  to_date: string;
  revenues: IncomeStatementSection;
  expenses: IncomeStatementSection;
  net_income: string;
}

export interface TrialBalanceItem {
  account_id: string;
  code?: string | null;
  name: string;
  account_type: string;
  debit: string;
  credit: string;
}

export interface TrialBalanceReport {
  as_of_date: string;
  items: TrialBalanceItem[];
  total_debit: string;
  total_credit: string;
  is_balanced: boolean;
}
