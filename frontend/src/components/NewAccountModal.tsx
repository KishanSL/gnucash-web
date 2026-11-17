import React, { useState, useEffect } from 'react';
import { X, FolderPlus, Edit3, AlertCircle, EyeOff } from 'lucide-react';
import { Account, CreateAccountPayload, UpdateAccountPayload } from '../types';
import { SUPPORTED_CURRENCIES, getErrorMessage } from '../api';

interface NewAccountModalProps {
  isOpen: boolean;
  onClose: () => void;
  onSubmit: (payload: CreateAccountPayload) => Promise<void>;
  onUpdate?: (id: string, payload: UpdateAccountPayload) => Promise<void>;
  editingAccount?: Account | null;
  accounts: Account[];
  defaultParentId?: string;
  currentCurrency: string;
}

const isTypeCompatible = (typeA: string, typeB: string): boolean => {
  const getGroup = (t: string) => {
    const lower = t.toLowerCase();
    if (['asset', 'bank', 'cash', 'receivable', 'credit', 'liability', 'payable'].includes(lower)) return 'balance_sheet';
    if (['income', 'expense'].includes(lower)) return 'income_expense';
    if (lower === 'equity') return 'equity';
    return lower;
  };
  const gA = getGroup(typeA);
  const gB = getGroup(typeB);
  return gA === gB;
};

export const NewAccountModal: React.FC<NewAccountModalProps> = ({
  isOpen,
  onClose,
  onSubmit,
  onUpdate,
  editingAccount,
  accounts,
  defaultParentId,
  currentCurrency,
}) => {
  const [name, setName] = useState('');
  const [code, setCode] = useState('');
  const [accountType, setAccountType] = useState('bank');
  const [parentId, setParentId] = useState<string>('');
  const [currency, setCurrency] = useState(currentCurrency || 'INR');
  const [description, setDescription] = useState('');
  const [placeholder, setPlaceholder] = useState(false);
  const [hidden, setHidden] = useState(false);
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const isEditing = Boolean(editingAccount);

  useEffect(() => {
    if (isOpen) {
      if (editingAccount) {
        setName(editingAccount.name);
        setCode(editingAccount.code || '');
        setAccountType(editingAccount.account_type);
        setParentId(editingAccount.parent_id || '');
        setCurrency(editingAccount.currency || currentCurrency || 'INR');
        setDescription(editingAccount.description || '');
        setPlaceholder(editingAccount.placeholder);
        setHidden(editingAccount.hidden);
      } else {
        setName('');
        setCode('');
        setAccountType('bank');
        setParentId(defaultParentId || '');
        setCurrency(currentCurrency || 'INR');
        setDescription('');
        setPlaceholder(false);
        setHidden(false);

        // Inherit type from parent if selected
        if (defaultParentId) {
          const parent = accounts.find((a) => a.id === defaultParentId);
          if (parent) {
            setAccountType(parent.account_type);
          }
        }
      }
      setError(null);
    }
  }, [isOpen, editingAccount, defaultParentId, accounts, currentCurrency]);

  if (!isOpen) return null;

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setError(null);

    if (!name.trim()) {
      setError('Please provide an account name.');
      return;
    }

    try {
      setSubmitting(true);
      if (isEditing && editingAccount && onUpdate) {
        await onUpdate(editingAccount.id, {
          name: name.trim(),
          code: code.trim() ? code.trim() : null,
          account_type: accountType,
          parent_id: parentId ? parentId : null,
          currency,
          description: description.trim() ? description.trim() : null,
          placeholder,
          hidden,
        });
      } else {
        await onSubmit({
          name: name.trim(),
          code: code.trim() ? code.trim() : null,
          account_type: accountType,
          parent_id: parentId ? parentId : null,
          currency,
          description: description.trim() ? description.trim() : null,
          placeholder,
          hidden,
        });
      }
      onClose();
    } catch (err: unknown) {
      setError(getErrorMessage(err) || (isEditing ? 'Failed to update account' : 'Failed to create account'));
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <div className="fixed inset-0 z-50 overflow-y-auto bg-slate-900/60 backdrop-blur-sm flex items-center justify-center p-2 sm:p-4">
      <div className="bg-white rounded-2xl border border-slate-200 shadow-2xl max-w-lg w-full overflow-hidden max-h-[94vh] flex flex-col animate-in fade-in zoom-in-95 duration-150">
        <div className="px-4 sm:px-6 py-3 sm:py-4 border-b border-slate-200 bg-slate-50 flex items-center justify-between shrink-0">
          <div className="flex items-center space-x-2">
            <div className={`w-8 h-8 rounded-lg flex items-center justify-center shrink-0 ${
              isEditing ? 'bg-amber-100 text-amber-700' : 'bg-emerald-100 text-emerald-700'
            }`}>
              {isEditing ? <Edit3 className="w-4 h-4" /> : <FolderPlus className="w-4 h-4" />}
            </div>
            <div>
              <h3 className="font-bold text-slate-800 text-sm sm:text-base">
                {isEditing ? `Edit Account: ${editingAccount?.name}` : 'New Account'}
              </h3>
              <p className="text-[11px] sm:text-xs text-slate-400">
                {isEditing ? 'Update account details, placeholder, or hidden status' : 'Add an account to your Chart of Accounts'}
              </p>
            </div>
          </div>
          <button
            onClick={onClose}
            className="p-1 rounded-lg text-slate-400 hover:text-slate-600 hover:bg-slate-200 transition"
          >
            <X className="w-5 h-5" />
          </button>
        </div>

        <form onSubmit={handleSubmit} className="p-4 sm:p-6 space-y-3 sm:space-y-4 overflow-y-auto flex-1">
          {error && (
            <div className="p-3 bg-rose-50 border border-rose-200 text-rose-700 rounded-lg text-xs flex items-center space-x-2">
              <AlertCircle className="w-4 h-4 shrink-0 text-rose-500" />
              <span>{error}</span>
            </div>
          )}

          <div className="grid grid-cols-1 sm:grid-cols-12 gap-3">
            <div className="sm:col-span-8">
              <label className="block text-xs font-semibold text-slate-600 mb-1">
                Account Name *
              </label>
              <input
                type="text"
                required
                placeholder="e.g. Petty Cash, Business Checking"
                value={name}
                onChange={(e) => setName(e.target.value)}
                className="w-full px-3 py-1.5 text-xs bg-slate-50 border border-slate-300 rounded-lg focus:ring-2 focus:ring-emerald-500/20 focus:border-emerald-500 font-medium"
              />
            </div>

            <div className="sm:col-span-4">
              <label className="block text-xs font-semibold text-slate-600 mb-1">
                Account Code
              </label>
              <input
                type="text"
                placeholder="e.g. 1115"
                value={code}
                onChange={(e) => setCode(e.target.value)}
                className="w-full px-3 py-1.5 text-xs bg-slate-50 border border-slate-300 rounded-lg focus:ring-2 focus:ring-emerald-500/20 focus:border-emerald-500 font-mono"
              />
            </div>
          </div>

          <div className="grid grid-cols-1 sm:grid-cols-2 gap-3">
            <div>
              <label className="block text-xs font-semibold text-slate-600 mb-1">
                Account Type *
              </label>
              <select
                value={accountType}
                onChange={(e) => setAccountType(e.target.value)}
                className="w-full px-3 py-1.5 text-xs bg-slate-50 border border-slate-300 rounded-lg focus:ring-2 focus:ring-emerald-500/20 focus:border-emerald-500 font-medium capitalize"
              >
                <option value="bank">Bank Account</option>
                <option value="cash">Cash in Hand</option>
                <option value="asset">Asset (Fixed/Current)</option>
                <option value="credit">Credit Card</option>
                <option value="liability">Liability / Loan</option>
                <option value="equity">Equity</option>
                <option value="income">Income / Revenue</option>
                <option value="expense">Expense</option>
                <option value="receivable">Accounts Receivable</option>
                <option value="payable">Accounts Payable</option>
              </select>
            </div>

            <div>
              <label className="block text-xs font-semibold text-slate-600 mb-1">
                Parent Account
              </label>
              <select
                value={parentId}
                onChange={(e) => setParentId(e.target.value)}
                className="w-full px-3 py-1.5 text-xs bg-slate-50 border border-slate-300 rounded-lg focus:ring-2 focus:ring-emerald-500/20 focus:border-emerald-500"
              >
                <option value="">-- Top Level Account (None) --</option>
                {accounts
                  .filter((a) => (!editingAccount || a.id !== editingAccount.id) && isTypeCompatible(accountType, a.account_type))
                  .map((a) => (
                    <option key={a.id} value={a.id}>
                      {a.name} ({a.account_type}){a.placeholder ? ' [Folder]' : ''}{a.hidden ? ' [Hidden]' : ''}
                    </option>
                  ))}
              </select>
            </div>
          </div>

          <div className="grid grid-cols-1 sm:grid-cols-2 gap-3">
            <div>
              <label className="block text-xs font-semibold text-slate-600 mb-1">Currency</label>
              <select
                value={currency}
                onChange={(e) => setCurrency(e.target.value)}
                className="w-full px-3 py-1.5 text-xs bg-slate-50 border border-slate-300 rounded-lg focus:ring-2 focus:ring-emerald-500/20 focus:border-emerald-500 font-mono"
              >
                {SUPPORTED_CURRENCIES.map((c) => (
                  <option key={c.code} value={c.code}>
                    {c.symbol} {c.name}
                  </option>
                ))}
              </select>
            </div>
          </div>

          {/* Account Flag Options: Placeholder and Hidden */}
          <div className="p-3 bg-slate-50 border border-slate-200 rounded-xl space-y-3">
            <label className="flex items-start space-x-2.5 cursor-pointer select-none">
              <input
                type="checkbox"
                checked={placeholder}
                onChange={(e) => setPlaceholder(e.target.checked)}
                className="mt-0.5 rounded border-slate-300 text-emerald-600 focus:ring-emerald-500 w-4 h-4"
              />
              <div>
                <span className="text-xs text-slate-800 font-semibold block">
                  Placeholder / Folder
                </span>
                <span className="text-[11px] text-slate-500">
                  Used purely to categorize subaccounts. Cannot contain direct transactions.
                </span>
              </div>
            </label>

            <div className="border-t border-slate-200 pt-2.5">
              <label className="flex items-start space-x-2.5 cursor-pointer select-none">
                <input
                  type="checkbox"
                  checked={hidden}
                  onChange={(e) => setHidden(e.target.checked)}
                  className="mt-0.5 rounded border-slate-300 text-amber-600 focus:ring-amber-500 w-4 h-4"
                />
                <div>
                  <div className="flex items-center space-x-1.5">
                    <span className="text-xs text-slate-800 font-semibold">
                      Hidden Account
                    </span>
                    <span className="inline-flex items-center space-x-1 px-1.5 py-0.2 rounded text-[10px] font-medium bg-amber-100 text-amber-800 border border-amber-200">
                      <EyeOff className="w-3 h-3" />
                      <span>GnuCash hidden</span>
                    </span>
                  </div>
                  <span className="text-[11px] text-slate-500">
                    Omit from Chart of Accounts tree and register lists by default unless "Show Hidden" is enabled.
                  </span>
                </div>
              </label>
            </div>
          </div>

          <div>
            <label className="block text-xs font-semibold text-slate-600 mb-1">
              Description / Notes
            </label>
            <textarea
              rows={2}
              placeholder="Account description or purpose..."
              value={description}
              onChange={(e) => setDescription(e.target.value)}
              className="w-full px-3 py-1.5 text-xs bg-slate-50 border border-slate-300 rounded-lg focus:ring-2 focus:ring-emerald-500/20 focus:border-emerald-500"
            />
          </div>

          <div className="flex items-center justify-end space-x-3 pt-3 border-t border-slate-200">
            <button
              type="button"
              onClick={onClose}
              className="px-4 py-2 text-xs font-semibold text-slate-600 hover:text-slate-800 bg-slate-100 hover:bg-slate-200 rounded-lg transition"
            >
              Cancel
            </button>
            <button
              type="submit"
              disabled={submitting}
              className={`px-5 py-2 text-xs font-semibold text-white disabled:opacity-50 rounded-lg transition shadow-sm active:scale-95 ${
                isEditing
                  ? 'bg-amber-600 hover:bg-amber-500'
                  : 'bg-emerald-600 hover:bg-emerald-500'
              }`}
            >
              {submitting
                ? (isEditing ? 'Saving...' : 'Creating...')
                : (isEditing ? 'Save Changes' : 'Create Account')}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
};
