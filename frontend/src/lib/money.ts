// Money arithmetic for transaction entry. All maths is done in integer cents so that the
// balance check always matches the amounts sent to the API (ACC-2: splits sum to exactly 0.00,
// ACC-3: at most 2 decimal places).

/**
 * Parses an amount (as typed, or as returned by the API) into integer cents, rounding half away
 * from zero at the third decimal. Empty or invalid input is 0.
 */
export function toCents(value: string | number | null | undefined): number {
  if (value === null || value === undefined) return 0;
  const str = String(value).trim();
  const match = /^([+-]?)(\d*)(?:\.(\d*))?$/.exec(str);
  if (!match || (match[2] === '' && !match[3])) {
    // Exponent notation or trailing garbage: keep parseFloat semantics
    const n = parseFloat(str);
    return Number.isFinite(n) ? Math.sign(n) * Math.round(Math.abs(n) * 100) : 0;
  }
  const sign = match[1] === '-' ? -1 : 1;
  const whole = parseInt(match[2] || '0', 10);
  const frac = match[3] || '';
  let cents = whole * 100 + parseInt((frac + '00').slice(0, 2), 10);
  if (frac.length > 2 && frac[2] >= '5') cents += 1;
  return cents === 0 ? 0 : sign * cents;
}

/** Formats integer cents as a 2-decimal amount string, e.g. -1050 → "-10.50". */
export function centsToAmount(cents: number): string {
  const abs = Math.abs(cents);
  const sign = cents < 0 ? '-' : '';
  return `${sign}${Math.floor(abs / 100)}.${String(abs % 100).padStart(2, '0')}`;
}

/** Absolute value of an amount as a 2-decimal string, or null when it is zero. */
export function absAmount(value: string | number | null | undefined): string | null {
  const cents = Math.abs(toCents(value));
  return cents === 0 ? null : centsToAmount(cents);
}

/** Splits a signed amount into debit/credit input fields (positive = debit). */
export function toDebitCredit(value: string | number | null | undefined): { debit: string; credit: string } {
  const cents = toCents(value);
  return {
    debit: cents > 0 ? centsToAmount(cents) : '',
    credit: cents < 0 ? centsToAmount(-cents) : '',
  };
}

/** Signed amount in cents of a debit/credit field pair: the debit if positive, otherwise minus the credit. */
export function netCents(debit: string, credit: string): number {
  const debitCents = toCents(debit);
  return debitCents > 0 ? debitCents : 0 - toCents(credit);
}

export interface SplitTotals {
  totalDebitCents: number;
  totalCreditCents: number;
  imbalanceCents: number;
  isBalanced: boolean;
}

/** Totals of debit/credit field pairs. Balanced means debits equal credits and are non-zero. */
export function splitTotals(splits: { debit: string; credit: string }[]): SplitTotals {
  let totalDebitCents = 0;
  let totalCreditCents = 0;
  for (const s of splits) {
    totalDebitCents += toCents(s.debit);
    totalCreditCents += toCents(s.credit);
  }
  const imbalanceCents = totalDebitCents - totalCreditCents;
  return {
    totalDebitCents,
    totalCreditCents,
    imbalanceCents,
    isBalanced: imbalanceCents === 0 && totalDebitCents > 0,
  };
}

/** New debit/credit fields for the row that absorbs the imbalance so the transaction balances. */
export function balancingFields(
  row: { debit: string; credit: string },
  imbalanceCents: number
): { debit: string; credit: string } {
  return toDebitCredit(centsToAmount(netCents(row.debit, row.credit) - imbalanceCents));
}

/** Amounts for a two-split entry: the current account and the transfer account always sum to zero. */
export function twoSplitAmounts(amount: string | number, isDeposit: boolean): [string, string] {
  const cents = Math.abs(toCents(amount));
  const current = isDeposit ? cents : -cents;
  return [centsToAmount(current), centsToAmount(-current)];
}
