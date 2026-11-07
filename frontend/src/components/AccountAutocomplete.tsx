import React, { useState, useRef, useEffect, useMemo } from 'react';
import { ChevronDown, Search, Check, FolderTree, X } from 'lucide-react';
import { Account } from '../types';

interface AccountAutocompleteProps {
  accounts: Account[];
  value: string; // account_id
  onChange: (accountId: string) => void;
  placeholder?: string;
  className?: string;
  autoFocus?: boolean;
  dropdownPosition?: 'top' | 'bottom' | 'auto';
}

export const AccountAutocomplete: React.FC<AccountAutocompleteProps> = ({
  accounts,
  value,
  onChange,
  placeholder = 'Select or search account...',
  className = '',
  autoFocus = false,
  dropdownPosition = 'auto',
}) => {
  const [isOpen, setIsOpen] = useState(false);
  const [search, setSearch] = useState('');
  const [highlightedIndex, setHighlightedIndex] = useState(0);

  const containerRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLInputElement>(null);
  const listRef = useRef<HTMLUListElement>(null);

  // Find currently selected account
  const selectedAccount = useMemo(() => {
    return accounts.find((a) => a.id === value);
  }, [accounts, value]);

  // Format label for display
  const formatAccountLabel = (acc?: Account) => {
    if (!acc) return '';
    const codePart = acc.code ? `${acc.code} - ` : '';
    return `${codePart}${acc.name}`;
  };

  // Sync search input with selected account when not actively editing
  useEffect(() => {
    if (!isOpen) {
      setSearch(selectedAccount ? formatAccountLabel(selectedAccount) : '');
    }
  }, [selectedAccount, isOpen]);

  // Filter accounts
  const filteredAccounts = useMemo(() => {
    const q = search.trim().toLowerCase();
    if (!q || q === formatAccountLabel(selectedAccount).toLowerCase()) {
      return accounts;
    }
    return accounts.filter((acc) => {
      const matchName = acc.name.toLowerCase().includes(q);
      const matchCode = acc.code ? acc.code.toLowerCase().includes(q) : false;
      const matchType = acc.account_type.toLowerCase().includes(q);
      return matchName || matchCode || matchType;
    });
  }, [accounts, search, selectedAccount]);

  // Reset highlighted index when filtered list changes
  useEffect(() => {
    setHighlightedIndex(0);
  }, [filteredAccounts]);

  // Handle outside click
  useEffect(() => {
    const handleClickOutside = (e: MouseEvent) => {
      if (containerRef.current && !containerRef.current.contains(e.target as Node)) {
        setIsOpen(false);
        setSearch(selectedAccount ? formatAccountLabel(selectedAccount) : '');
      }
    };
    document.addEventListener('mousedown', handleClickOutside);
    return () => {
      document.removeEventListener('mousedown', handleClickOutside);
    };
  }, [selectedAccount]);

  // Keep highlighted item visible when scrolling via arrow keys
  useEffect(() => {
    if (isOpen && listRef.current) {
      const activeElement = listRef.current.children[highlightedIndex] as HTMLElement;
      if (activeElement) {
        activeElement.scrollIntoView({ block: 'nearest' });
      }
    }
  }, [highlightedIndex, isOpen]);

  const handleSelect = (account: Account) => {
    onChange(account.id);
    setSearch(formatAccountLabel(account));
    setIsOpen(false);
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (!isOpen) {
      if (e.key === 'ArrowDown' || e.key === 'ArrowUp' || e.key === 'Enter') {
        e.preventDefault();
        setIsOpen(true);
      }
      return;
    }

    if (e.key === 'ArrowDown') {
      e.preventDefault();
      setHighlightedIndex((prev) =>
        prev < filteredAccounts.length - 1 ? prev + 1 : 0
      );
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      setHighlightedIndex((prev) =>
        prev > 0 ? prev - 1 : filteredAccounts.length - 1
      );
    } else if (e.key === 'Enter') {
      e.preventDefault();
      if (filteredAccounts[highlightedIndex]) {
        handleSelect(filteredAccounts[highlightedIndex]);
      }
    } else if (e.key === 'Escape') {
      e.preventDefault();
      setIsOpen(false);
      setSearch(selectedAccount ? formatAccountLabel(selectedAccount) : '');
    } else if (e.key === 'Tab') {
      if (filteredAccounts[highlightedIndex] && search !== formatAccountLabel(selectedAccount)) {
        handleSelect(filteredAccounts[highlightedIndex]);
      }
      setIsOpen(false);
    }
  };

  const getAccountTypeColor = (type: string) => {
    switch (type.toLowerCase()) {
      case 'asset':
      case 'bank':
      case 'cash':
        return 'bg-blue-50 text-blue-700 border-blue-200';
      case 'liability':
      case 'credit':
        return 'bg-amber-50 text-amber-700 border-amber-200';
      case 'equity':
        return 'bg-purple-50 text-purple-700 border-purple-200';
      case 'income':
        return 'bg-emerald-50 text-emerald-700 border-emerald-200';
      case 'expense':
        return 'bg-rose-50 text-rose-700 border-rose-200';
      default:
        return 'bg-slate-100 text-slate-700 border-slate-200';
    }
  };

  return (
    <div ref={containerRef} className={`relative w-full ${className}`}>
      {/* Combobox Input Trigger */}
      <div className="relative flex items-center">
        <input
          ref={inputRef}
          type="text"
          value={search}
          autoFocus={autoFocus}
          placeholder={placeholder}
          onClick={() => setIsOpen(true)}
          onFocus={() => {
            setIsOpen(true);
            // Select text on focus for effortless replacement
            inputRef.current?.select();
          }}
          onChange={(e) => {
            setSearch(e.target.value);
            if (!isOpen) setIsOpen(true);
          }}
          onKeyDown={handleKeyDown}
          className="w-full py-1.5 pl-2.5 pr-8 text-xs bg-white border border-slate-300 rounded-lg focus:outline-none focus:ring-1 focus:ring-emerald-500 font-medium text-slate-900 transition shadow-sm placeholder:text-slate-400"
          aria-expanded={isOpen}
          aria-label="Account Selector"
          role="combobox"
        />
        {search && isOpen && (
          <button
            type="button"
            tabIndex={-1}
            onClick={(e) => {
              e.stopPropagation();
              setSearch('');
              inputRef.current?.focus();
            }}
            className="absolute right-6 text-slate-400 hover:text-slate-600 p-0.5 rounded transition"
            aria-label="Clear account search"
          >
            <X className="w-3 h-3" />
          </button>
        )}
        <button
          type="button"
          tabIndex={-1}
          onClick={() => {
            setIsOpen((prev) => !prev);
            if (!isOpen) {
              inputRef.current?.focus();
            }
          }}
          className="absolute right-1 text-slate-400 hover:text-slate-600 p-1 rounded transition"
          aria-label="Toggle accounts dropdown"
        >
          <ChevronDown
            className={`w-3.5 h-3.5 transition-transform duration-150 ${
              isOpen ? 'rotate-180 text-emerald-600' : ''
            }`}
          />
        </button>
      </div>

      {/* Autocomplete Dropdown List */}
      {isOpen && (
        <div
          className={`absolute z-50 left-0 min-w-[280px] sm:min-w-[320px] max-w-md w-full bg-white border border-slate-200 rounded-xl shadow-2xl overflow-hidden animate-in fade-in duration-150 ${
            dropdownPosition === 'top'
              ? 'bottom-full mb-1 slide-in-from-bottom-1'
              : 'top-full mt-1 slide-in-from-top-1'
          }`}
        >
          <div className="px-3 py-1.5 bg-slate-50 border-b border-slate-200 flex items-center justify-between text-[11px] text-slate-500 font-semibold">
            <span className="flex items-center space-x-1">
              <FolderTree className="w-3.5 h-3.5 text-emerald-600" />
              <span>Select Account ({filteredAccounts.length})</span>
            </span>
            <span className="text-[10px] text-slate-400">↑↓ to navigate, ↵ to pick</span>
          </div>

          <ul
            ref={listRef}
            className="max-h-56 overflow-y-auto divide-y divide-slate-100 py-1"
            role="listbox"
          >
            {filteredAccounts.length === 0 ? (
              <li className="px-3 py-4 text-center text-xs text-slate-400">
                <Search className="w-4 h-4 mx-auto mb-1 text-slate-300" />
                No accounts matching &ldquo;{search}&rdquo;
              </li>
            ) : (
              filteredAccounts.map((account, index) => {
                const isSelected = account.id === value;
                const isHighlighted = index === highlightedIndex;

                return (
                  <li
                    key={account.id}
                    role="option"
                    aria-selected={isSelected}
                    onMouseEnter={() => setHighlightedIndex(index)}
                    onClick={() => handleSelect(account)}
                    className={`px-3 py-2 text-xs cursor-pointer flex items-center justify-between transition-colors ${
                      isHighlighted
                        ? 'bg-emerald-50/80 text-emerald-900'
                        : isSelected
                        ? 'bg-slate-50 font-medium'
                        : 'hover:bg-slate-50 text-slate-800'
                    }`}
                  >
                    <div className="flex items-center space-x-2 overflow-hidden pr-2">
                      {account.code && (
                        <span className="shrink-0 font-mono text-[10px] bg-slate-100 text-slate-600 px-1.5 py-0.5 rounded border border-slate-200">
                          {account.code}
                        </span>
                      )}
                      <div className="truncate">
                        <span className="font-semibold text-slate-800">{account.name}</span>
                        {account.hidden && (
                          <span className="ml-1 text-[10px] text-slate-400 italic">[Hidden]</span>
                        )}
                      </div>
                    </div>

                    <div className="flex items-center space-x-1.5 shrink-0">
                      <span
                        className={`text-[10px] uppercase font-bold px-1.5 py-0.5 rounded-full border ${getAccountTypeColor(
                          account.account_type
                        )}`}
                      >
                        {account.account_type}
                      </span>
                      {isSelected && (
                        <Check className="w-3.5 h-3.5 text-emerald-600 shrink-0" />
                      )}
                    </div>
                  </li>
                );
              })
            )}
          </ul>
        </div>
      )}
    </div>
  );
};
