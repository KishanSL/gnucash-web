import { useState } from 'react';

/** Display currency preference, persisted in localStorage (defaults to INR). */
export function useCurrencyPreference() {
  const [currency, setCurrency] = useState<string>(() => {
    return localStorage.getItem('gnucash_currency') || 'INR';
  });

  const changeCurrency = (newCurrency: string) => {
    setCurrency(newCurrency);
    localStorage.setItem('gnucash_currency', newCurrency);
  };

  return { currency, changeCurrency };
}
