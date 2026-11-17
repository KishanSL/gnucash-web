import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import {
  absAmount,
  balancingFields,
  centsToAmount,
  netCents,
  splitTotals,
  toCents,
  toDebitCredit,
  twoSplitAmounts,
} from '../src/lib/money.ts';

describe('toCents', () => {
  it('parses plain amounts', () => {
    assert.equal(toCents('10'), 1000);
    assert.equal(toCents('10.5'), 1050);
    assert.equal(toCents('10.50'), 1050);
    assert.equal(toCents('.75'), 75);
    assert.equal(toCents('-42.01'), -4201);
    assert.equal(toCents(12.34), 1234);
  });

  it('rounds half away from zero at the third decimal without float error', () => {
    assert.equal(toCents('1.005'), 101);
    assert.equal(toCents('10.004'), 1000);
    assert.equal(toCents('-1.005'), -101);
    assert.equal(toCents('0.125'), 13);
  });

  it('treats empty and invalid input as zero', () => {
    assert.equal(toCents(''), 0);
    assert.equal(toCents(null), 0);
    assert.equal(toCents(undefined), 0);
    assert.equal(toCents('abc'), 0);
    assert.equal(toCents('-0'), 0);
  });

  it('keeps parseFloat semantics for unusual input', () => {
    assert.equal(toCents('12abc'), 1200);
    assert.equal(toCents('1e2'), 10000);
  });
});

describe('centsToAmount', () => {
  it('always formats exactly 2 decimals (ACC-3)', () => {
    assert.equal(centsToAmount(0), '0.00');
    assert.equal(centsToAmount(5), '0.05');
    assert.equal(centsToAmount(1050), '10.50');
    assert.equal(centsToAmount(-1050), '-10.50');
    assert.equal(centsToAmount(123456789), '1234567.89');
  });
});

describe('absAmount / toDebitCredit / netCents', () => {
  it('absAmount returns null for zero', () => {
    assert.equal(absAmount('-25.5'), '25.50');
    assert.equal(absAmount('0.00'), null);
  });

  it('toDebitCredit puts positive amounts in debit and negative in credit', () => {
    assert.deepEqual(toDebitCredit('100'), { debit: '100.00', credit: '' });
    assert.deepEqual(toDebitCredit('-100'), { debit: '', credit: '100.00' });
    assert.deepEqual(toDebitCredit('0'), { debit: '', credit: '' });
  });

  it('netCents prefers a positive debit, otherwise minus the credit', () => {
    assert.equal(netCents('25.00', ''), 2500);
    assert.equal(netCents('', '25.00'), -2500);
    assert.equal(netCents('', ''), 0);
  });
});

describe('splitTotals', () => {
  it('detects a balanced transaction', () => {
    const t = splitTotals([
      { debit: '100.00', credit: '' },
      { debit: '', credit: '60.00' },
      { debit: '', credit: '40.00' },
    ]);
    assert.deepEqual(t, { totalDebitCents: 10000, totalCreditCents: 10000, imbalanceCents: 0, isBalanced: true });
  });

  it('is exact where float addition is not (0.1 + 0.2)', () => {
    const t = splitTotals([
      { debit: '0.1', credit: '' },
      { debit: '0.2', credit: '' },
      { debit: '', credit: '0.3' },
    ]);
    assert.equal(t.isBalanced, true);
  });

  it('checks the same rounded amounts that are sent to the API', () => {
    // 10.005 + 10.005 rounds to 10.01 + 10.01 = 20.02, which does not balance 20.01
    const t = splitTotals([
      { debit: '10.005', credit: '' },
      { debit: '10.005', credit: '' },
      { debit: '', credit: '20.01' },
    ]);
    assert.equal(t.imbalanceCents, 1);
    assert.equal(t.isBalanced, false);
  });

  it('a zero-value transaction is not balanced', () => {
    assert.equal(splitTotals([{ debit: '', credit: '' }, { debit: '', credit: '' }]).isBalanced, false);
  });
});

describe('balancingFields', () => {
  it('fills an empty last row with the missing credit', () => {
    assert.deepEqual(balancingFields({ debit: '', credit: '' }, 7000), { debit: '', credit: '70.00' });
  });

  it('increases an existing credit on the last row', () => {
    // debit 100 vs credit 30: imbalance 70, last row credit becomes 100
    assert.deepEqual(balancingFields({ debit: '', credit: '30.00' }, 7000), { debit: '', credit: '100.00' });
  });

  it('fills the missing debit when credits exceed debits', () => {
    assert.deepEqual(balancingFields({ debit: '', credit: '' }, -2500), { debit: '25.00', credit: '' });
  });

  it('balances when the last row is on the same side as the imbalance', () => {
    // debits 100 and 50 (last row): imbalance 150, last row must become a credit of 100
    const fields = balancingFields({ debit: '50.00', credit: '' }, 15000);
    assert.deepEqual(fields, { debit: '', credit: '100.00' });
    assert.equal(splitTotals([{ debit: '100.00', credit: '' }, fields]).isBalanced, true);
  });
});

describe('twoSplitAmounts', () => {
  it('produces amounts that sum to exactly zero (ACC-2)', () => {
    assert.deepEqual(twoSplitAmounts('500.50', true), ['500.50', '-500.50']);
    assert.deepEqual(twoSplitAmounts('500.50', false), ['-500.50', '500.50']);
  });

  it('never sends more than 2 decimals (ACC-3)', () => {
    const [a, b] = twoSplitAmounts('1.235', true);
    assert.equal(a, '1.24');
    assert.equal(b, '-1.24');
    assert.equal(toCents(a) + toCents(b), 0);
  });
});
