import React, { useState, useRef } from 'react';
import {
  X,
  Upload,
  Database,
  FileCheck,
  AlertCircle,
  CheckCircle2,
  HelpCircle,
} from 'lucide-react';
import { importGnuCashSqlite, SUPPORTED_CURRENCIES } from '../api';

interface ImportModalProps {
  isOpen: boolean;
  onClose: () => void;
  onImportSuccess: () => void;
  currentCurrency: string;
}

export const ImportModal: React.FC<ImportModalProps> = ({
  isOpen,
  onClose,
  onImportSuccess,
  currentCurrency,
}) => {
  const [selectedFile, setSelectedFile] = useState<File | null>(null);
  const [wipeExisting, setWipeExisting] = useState(true);
  const [defaultCurrency, setDefaultCurrency] = useState(currentCurrency || 'INR');
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [result, setResult] = useState<{
    accounts: number;
    transactions: number;
    splits: number;
    message: string;
  } | null>(null);

  const fileInputRef = useRef<HTMLInputElement>(null);

  if (!isOpen) return null;

  const handleFileChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    if (e.target.files && e.target.files[0]) {
      setSelectedFile(e.target.files[0]);
      setError(null);
      setResult(null);
    }
  };

  const handleDrop = (e: React.DragEvent<HTMLDivElement>) => {
    e.preventDefault();
    if (e.dataTransfer.files && e.dataTransfer.files[0]) {
      setSelectedFile(e.dataTransfer.files[0]);
      setError(null);
      setResult(null);
    }
  };

  const handleDragOver = (e: React.DragEvent<HTMLDivElement>) => {
    e.preventDefault();
  };

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!selectedFile) {
      setError('Please select a .gnucash SQLite database file to import.');
      return;
    }

    try {
      setSubmitting(true);
      setError(null);
      const res = await importGnuCashSqlite(selectedFile, wipeExisting, defaultCurrency);
      setResult({
        accounts: res.accounts_imported,
        transactions: res.transactions_imported,
        splits: res.splits_imported,
        message: res.message,
      });
      onImportSuccess();
    } catch (err: any) {
      setError(err.message || 'Failed to import GnuCash database');
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <div className="fixed inset-0 z-50 overflow-y-auto bg-slate-900/60 backdrop-blur-sm flex items-center justify-center p-2 sm:p-4">
      <div className="bg-white rounded-2xl border border-slate-200 shadow-2xl max-w-xl w-full overflow-hidden max-h-[94vh] flex flex-col animate-in fade-in zoom-in-95 duration-150">
        {/* Header */}
        <div className="px-4 sm:px-6 py-3 sm:py-4 border-b border-slate-200 bg-slate-50 flex items-center justify-between shrink-0">
          <div className="flex items-center space-x-2">
            <div className="w-8 h-8 rounded-lg bg-emerald-100 text-emerald-700 flex items-center justify-center shrink-0">
              <Database className="w-4 h-4" />
            </div>
            <div>
              <h3 className="font-bold text-slate-800 text-sm sm:text-base">
                Import GnuCash File (.gnucash)
              </h3>
              <p className="text-[11px] sm:text-xs text-slate-400">
                Supports all official GnuCash files (standard compressed XML & SQLite formats)
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

        {/* Content */}
        <form onSubmit={handleSubmit} className="p-4 sm:p-6 space-y-4 sm:space-y-5 overflow-y-auto flex-1">
          {error && (
            <div className="p-3 bg-rose-50 border border-rose-200 text-rose-700 rounded-lg text-xs flex items-center space-x-2">
              <AlertCircle className="w-4 h-4 shrink-0 text-rose-500" />
              <span>{error}</span>
            </div>
          )}

          {result && (
            <div className="p-4 bg-emerald-50 border border-emerald-200 text-emerald-900 rounded-xl space-y-2">
              <div className="flex items-center space-x-2 font-bold text-xs text-emerald-800">
                <CheckCircle2 className="w-4 h-4 text-emerald-600" />
                <span>Import Succeeded!</span>
              </div>
              <p className="text-xs text-emerald-700">{result.message}</p>
              <div className="grid grid-cols-3 gap-2 pt-2 border-t border-emerald-200/60 text-center font-mono">
                <div className="bg-white/80 p-2 rounded-lg border border-emerald-200">
                  <span className="text-[10px] text-slate-400 uppercase block font-sans">
                    Accounts
                  </span>
                  <span className="text-base font-bold text-emerald-800">
                    {result.accounts}
                  </span>
                </div>
                <div className="bg-white/80 p-2 rounded-lg border border-emerald-200">
                  <span className="text-[10px] text-slate-400 uppercase block font-sans">
                    Transactions
                  </span>
                  <span className="text-base font-bold text-emerald-800">
                    {result.transactions}
                  </span>
                </div>
                <div className="bg-white/80 p-2 rounded-lg border border-emerald-200">
                  <span className="text-[10px] text-slate-400 uppercase block font-sans">
                    Splits
                  </span>
                  <span className="text-base font-bold text-emerald-800">
                    {result.splits}
                  </span>
                </div>
              </div>
            </div>
          )}

          {/* File Upload Zone */}
          <div
            onDrop={handleDrop}
            onDragOver={handleDragOver}
            onClick={() => fileInputRef.current?.click()}
            className={`border-2 border-dashed rounded-xl p-6 text-center cursor-pointer transition ${
              selectedFile
                ? 'border-emerald-400 bg-emerald-50/30'
                : 'border-slate-300 hover:border-emerald-400 bg-slate-50/50 hover:bg-slate-50'
            }`}
          >
            <input
              type="file"
              ref={fileInputRef}
              onChange={handleFileChange}
              accept=".gnucash,.sqlite,.db"
              className="hidden"
            />

            {selectedFile ? (
              <div className="flex items-center justify-center space-x-3">
                <div className="w-10 h-10 rounded-lg bg-emerald-100 text-emerald-700 flex items-center justify-center">
                  <FileCheck className="w-5 h-5" />
                </div>
                <div className="text-left">
                  <p className="text-xs font-bold text-slate-800">{selectedFile.name}</p>
                  <p className="text-[11px] text-slate-400 font-mono">
                    {(selectedFile.size / 1024).toFixed(1)} KB • Click or drop to change
                  </p>
                </div>
              </div>
            ) : (
              <div className="space-y-2">
                <div className="w-12 h-12 rounded-xl bg-slate-100 text-slate-500 mx-auto flex items-center justify-center">
                  <Upload className="w-6 h-6" />
                </div>
                <div>
                  <p className="text-xs font-semibold text-slate-700">
                    Click to select file or drag & drop here
                  </p>
                  <p className="text-[11px] text-slate-400">
                    Supports <span className="font-mono">.gnucash</span>,{' '}
                    <span className="font-mono">.sqlite</span>, and{' '}
                    <span className="font-mono">.db</span> databases
                  </p>
                </div>
              </div>
            )}
          </div>

          {/* Import Settings */}
          <div className="space-y-3 bg-slate-50 p-4 rounded-xl border border-slate-200 text-xs">
            {/* Import Mode */}
            <div>
              <label className="block font-semibold text-slate-700 mb-1.5">
                Import Behavior
              </label>
              <div className="space-y-1.5">
                <label className="flex items-start space-x-2 cursor-pointer">
                  <input
                    type="radio"
                    name="wipe"
                    checked={wipeExisting}
                    onChange={() => setWipeExisting(true)}
                    className="mt-0.5 text-emerald-600 focus:ring-emerald-500"
                  />
                  <div>
                    <span className="font-medium text-slate-800">
                      Replace existing database (Recommended)
                    </span>
                    <p className="text-[11px] text-slate-400">
                      Clears current accounts and imports the full GnuCash chart of accounts cleanly.
                    </p>
                  </div>
                </label>

                <label className="flex items-start space-x-2 cursor-pointer">
                  <input
                    type="radio"
                    name="wipe"
                    checked={!wipeExisting}
                    onChange={() => setWipeExisting(false)}
                    className="mt-0.5 text-emerald-600 focus:ring-emerald-500"
                  />
                  <div>
                    <span className="font-medium text-slate-800">
                      Merge with existing data
                    </span>
                    <p className="text-[11px] text-slate-400">
                      Appends accounts and transactions without removing existing records.
                    </p>
                  </div>
                </label>
              </div>
            </div>

            {/* Currency Fallback */}
            <div className="pt-2 border-t border-slate-200 flex items-center justify-between">
              <div>
                <span className="font-semibold text-slate-700 block">
                  Default Currency
                </span>
                <span className="text-[11px] text-slate-400">
                  Applied to accounts without explicit commodity
                </span>
              </div>
              <select
                value={defaultCurrency}
                onChange={(e) => setDefaultCurrency(e.target.value)}
                className="py-1 px-2.5 text-xs bg-white border border-slate-300 rounded-lg font-mono font-medium focus:ring-emerald-500"
              >
                {SUPPORTED_CURRENCIES.map((c) => (
                  <option key={c.code} value={c.code}>
                    {c.symbol} {c.code}
                  </option>
                ))}
              </select>
            </div>
          </div>

          {/* Help tip */}
          <div className="flex items-start space-x-2 text-[11px] text-slate-400 bg-slate-100/60 p-2.5 rounded-lg">
            <HelpCircle className="w-4 h-4 text-slate-400 shrink-0 mt-0.5" />
            <span>
              Tip: In desktop GnuCash, you can save your book as SQLite by clicking{' '}
              <strong>File → Save As... → Format: sqlite3</strong>.
            </span>
          </div>

          {/* Actions */}
          <div className="flex items-center justify-end space-x-3 pt-2 border-t border-slate-200">
            <button
              type="button"
              onClick={onClose}
              className="px-4 py-2 text-xs font-semibold text-slate-600 hover:text-slate-800 bg-slate-100 hover:bg-slate-200 rounded-lg transition"
            >
              {result ? 'Done' : 'Cancel'}
            </button>

            {!result && (
              <button
                type="submit"
                disabled={submitting || !selectedFile}
                className="px-5 py-2 text-xs font-semibold text-white bg-emerald-600 hover:bg-emerald-500 disabled:opacity-50 disabled:cursor-not-allowed rounded-lg shadow-sm transition active:scale-95 flex items-center space-x-2"
              >
                <Upload className="w-4 h-4" />
                <span>{submitting ? 'Importing SQLite...' : 'Start Import'}</span>
              </button>
            )}
          </div>
        </form>
      </div>
    </div>
  );
};
