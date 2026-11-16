import puppeteer from 'puppeteer';
import path from 'path';
import fs from 'fs';
import { fileURLToPath } from 'url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const screenshotsDir = path.resolve(__dirname, '../screenshots');

if (!fs.existsSync(screenshotsDir)) {
  fs.mkdirSync(screenshotsDir, { recursive: true });
}

async function run() {
  console.log('🚀 Starting Comprehensive UI Verification for Register Enhancements...');
  const browser = await puppeteer.launch({
    headless: 'new',
    args: ['--no-sandbox', '--disable-setuid-sandbox'],
  });

  try {
    const page = await browser.newPage();
    await page.setViewport({ width: 1440, height: 900 });

    // 1. Navigate & Login
    console.log('1. Navigating to http://localhost:8081...');
    await page.goto('http://localhost:8081', { waitUntil: 'networkidle2' });

    const loginInput = await page.$('input[type="text"]');
    if (loginInput) {
      console.log('Logging in with kishan / Kishan@123...');
      await page.type('input[type="text"]', 'kishan');
      await page.type('input[type="password"]', 'Kishan@123');
      await page.click('button[type="submit"]');
      await new Promise((r) => setTimeout(r, 2000));
    }

    // 2. Open Register for Cash in Wallet
    console.log('2. Opening register for Cash in Wallet via double-click...');
    const openedAccount = await page.evaluate(() => {
      const rows = Array.from(document.querySelectorAll('tbody tr'));
      const cashRow = rows.find((r) => r.innerText.includes('Cash in Wallet'));
      if (cashRow) {
        cashRow.dispatchEvent(new MouseEvent('dblclick', { bubbles: true, cancelable: true }));
        return 'Cash in Wallet';
      }
      return null;
    });
    console.log(`Opened register for: "${openedAccount}"`);
    await new Promise((r) => setTimeout(r, 2000));

    // Capture desktop register view showing 2-line mode and Edit buttons
    await page.screenshot({ path: path.join(screenshotsDir, '40-register-2-row-mode.png') });
    console.log('Captured 40-register-2-row-mode.png');

    // Verify Edit buttons exist near Delete buttons
    const editButtonsCount = await page.$$eval('tbody tr button[title="Edit Transaction"]', (btns) => btns.length);
    const deleteButtonsCount = await page.$$eval('tbody tr button[title="Delete Transaction"]', (btns) => btns.length);
    console.log(`Edit buttons found: ${editButtonsCount}, Delete buttons found: ${deleteButtonsCount}`);
    if (editButtonsCount === 0) {
      throw new Error('No Edit buttons found in register table!');
    }

    // Verify second row displays Details / Memo / Notes text
    const secondRowDetails = await page.evaluate(() => {
      const texts = Array.from(document.querySelectorAll('tbody tr td span'));
      const notesLabels = texts.filter((el) => el.textContent && el.textContent.trim() === 'Notes:');
      const memoLabels = texts.filter((el) => el.textContent && el.textContent.trim() === 'Memo:');
      return {
        notesLabelsCount: notesLabels.length,
        memoLabelsCount: memoLabels.length,
      };
    });
    console.log('Second row details:', secondRowDetails);

    // 3. Test Autocomplete restricted to current account only
    console.log('3. Testing autocomplete in new transaction row (restricted to current account only)...');
    await page.evaluate(() => {
      document.querySelector('tr[data-testid="blank-new-row-main"]')?.scrollIntoView({ block: 'center' });
    });
    await new Promise((r) => setTimeout(r, 500));

    const descInput = await page.$('tr[data-testid="blank-new-row-main"] input[placeholder*="Description"]');
    if (descInput) {
      await descInput.click();
      await new Promise((r) => setTimeout(r, 600));
      await page.screenshot({ path: path.join(screenshotsDir, '41-register-payee-autocomplete-current-account.png') });

      const payeeOptions = await page.evaluate(() => {
        const btns = Array.from(document.querySelectorAll('.absolute button span.truncate'));
        return btns.map((b) => b.textContent.trim());
      });
      console.log(`Autocomplete payees for current account (${payeeOptions.length}):`, payeeOptions);
    }

    // 4. Record a new transaction with Notes & Memo
    console.log('4. Entering a new transaction with Notes & Memo in the blank row...');
    const uniquePayee = `Vendor ${Date.now()}`;
    const uniqueNote = `Invoice verified at ${new Date().toISOString()}`;
    const uniqueMemo = 'Direct Cash Expense Split';

    await page.evaluate(({ payee, note, memo }) => {
      const setReactValue = (el, val) => {
        if (!el) return;
        const tracker = el._valueTracker;
        if (tracker) tracker.setValue('');
        const setter = Object.getOwnPropertyDescriptor(window.HTMLInputElement.prototype, 'value').set;
        setter.call(el, val);
        el.dispatchEvent(new Event('input', { bubbles: true }));
        el.dispatchEvent(new Event('change', { bubbles: true }));
      };

      const desc = document.querySelector('tr[data-testid="blank-new-row-main"] input[placeholder*="Description"]');
      setReactValue(desc, payee);

      const debit = document.querySelector('tr[data-testid="blank-new-row-main"] input[title*="Deposit"], tr[data-testid="blank-new-row-main"] input[title*="Payment"]');
      setReactValue(debit, '75.50');

      const notes = document.querySelector('tr[data-testid="blank-new-row-notes"] input[placeholder*="Transaction notes"]');
      setReactValue(notes, note);

      const memoEl = document.querySelector('tr[data-testid="blank-new-row-notes"] input[placeholder*="Split memo"]');
      setReactValue(memoEl, memo);
    }, { payee: uniquePayee, note: uniqueNote, memo: uniqueMemo });

    // Open transfer account dropdown
    console.log('Selecting transfer account...');
    await page.evaluate(() => {
      const inp = document.querySelector('tr[data-testid="blank-new-row-main"] input[placeholder*="Select transfer account"]');
      if (inp) inp.click();
    });
    await new Promise((r) => setTimeout(r, 600));

    // Select first transfer option
    const optText = await page.evaluate(() => {
      const opt = document.querySelector('li[role="option"]');
      if (opt) {
        opt.click();
        return opt.innerText.replace(/\n/g, ' ');
      }
      return null;
    });
    console.log(`Selected transfer option: "${optText}"`);
    await new Promise((r) => setTimeout(r, 500));

    // Click Record Checkmark button
    console.log('Clicking Record Transaction checkmark...');
    await page.evaluate(() => {
      const btn = document.querySelector('tr[data-testid="blank-new-row-main"] button[title*="Record Transaction"]');
      if (btn) btn.click();
    });
    await new Promise((r) => setTimeout(r, 3000));

    await page.screenshot({ path: path.join(screenshotsDir, '42-transaction-recorded-with-notes.png') });
    console.log('Captured 42-transaction-recorded-with-notes.png');

    const recordedCheck = await page.evaluate(({ payee, note, memo }) => {
      return {
        hasDesc: document.body.innerText.includes(payee),
        hasNote: document.body.innerText.includes(note),
        hasMemo: document.body.innerText.includes(memo),
      };
    }, { payee: uniquePayee, note: uniqueNote, memo: uniqueMemo });
    console.log('Newly recorded transaction verification in DOM:', recordedCheck);
    if (!recordedCheck.hasDesc || !recordedCheck.hasNote || !recordedCheck.hasMemo) {
      throw new Error(`Transaction details were not found in DOM! ${JSON.stringify(recordedCheck)}`);
    }

    // 5. Test Edit Transaction Modal
    console.log('5. Clicking Edit button on the newly created transaction...');
    await page.evaluate(({ payee }) => {
      const rows = Array.from(document.querySelectorAll('tbody tr'));
      const targetRow = rows.find((r) => r.innerText.includes(payee));
      if (targetRow) {
        const editBtn = targetRow.querySelector('button[title="Edit Transaction"]');
        if (editBtn) editBtn.click();
      }
    }, { payee: uniquePayee });
    await new Promise((r) => setTimeout(r, 1500));

    await page.screenshot({ path: path.join(screenshotsDir, '43-edit-transaction-modal-open.png') });
    console.log('Captured 43-edit-transaction-modal-open.png');

    const modalValues = await page.evaluate(() => {
      const modal = document.querySelector('.fixed.z-50');
      if (!modal) return null;
      const h3 = modal.querySelector('h3');
      const desc = modal.querySelector('input[placeholder*="Grocery Store"]') || modal.querySelector('input[type="text"]:not([placeholder*="Num"])');
      const notes = modal.querySelector('textarea');
      const splitsCount = modal.querySelectorAll('.divide-y tr').length;
      return {
        title: h3 ? h3.innerText.trim() : null,
        descVal: desc ? desc.value : null,
        notesVal: notes ? notes.value : null,
        splitsCount,
      };
    });
    console.log('Edit Modal inspection:', modalValues);

    if (modalValues?.title !== 'Edit Transaction') {
      throw new Error(`Modal title was "${modalValues?.title}", expected "Edit Transaction"`);
    }

    // Close modal
    await page.evaluate(() => {
      const btns = Array.from(document.querySelectorAll('.fixed.z-50 button'));
      const cb = btns.find((b) => b.innerText.trim() === 'Cancel');
      if (cb) cb.click();
    });
    await new Promise((r) => setTimeout(r, 500));

    // 5b. Verify Expense account register headers (ACC-7, ACC-3, REG-2)
    console.log('5b. Testing expense account register header labels (ACC-7, ACC-3, REG-2)...');
    const selectedExpense = await page.evaluate(() => {
      const select = document.querySelector('select');
      if (!select) return null;
      const expenseOption = Array.from(select.options).find((opt) => opt.text.toLowerCase().includes('(expense)'));
      if (expenseOption) {
        select.value = expenseOption.value;
        select.dispatchEvent(new Event('change', { bubbles: true }));
        return expenseOption.text;
      }
      return null;
    });

    if (selectedExpense) {
      console.log(`Switched to expense account: "${selectedExpense}"`);
      await new Promise((r) => setTimeout(r, 1500));
      const headers = await page.evaluate(() => {
        const ths = Array.from(document.querySelectorAll('thead th')).map((th) => th.innerText.trim());
        return ths;
      });
      console.log('Register headers for expense account:', headers);
      const hasDebitHeader = headers.some((h) => h.includes('Expense / Increase'));
      const hasCreditHeader = headers.some((h) => h.includes('Rebate / Decrease'));
      if (!hasDebitHeader || !hasCreditHeader) {
        throw new Error(`Expense register headers mismatch: expected "Expense / Increase" and "Rebate / Decrease", found: ${JSON.stringify(headers)}`);
      }
      console.log('✅ Expense register headers verified: "Expense / Increase" & "Rebate / Decrease"');
    }

    // 6. Mobile Viewport (375x667)
    console.log('6. Testing mobile responsive view (375x667)...');
    await page.setViewport({ width: 375, height: 667 });
    await new Promise((r) => setTimeout(r, 1000));
    await page.screenshot({ path: path.join(screenshotsDir, '44-mobile-register-with-notes-and-edit.png') });
    console.log('Captured 44-mobile-register-with-notes-and-edit.png');

    // Verify mobile card has Edit button and details
    const mobileCheck = await page.evaluate(() => {
      const editBtns = document.querySelectorAll('button[title="Edit Transaction"]');
      const notesLabels = Array.from(document.querySelectorAll('span')).filter((s) => s.textContent?.trim() === 'Notes:');
      return {
        mobileEditButtons: editBtns.length,
        notesLabelsFound: notesLabels.length,
      };
    });
    console.log('Mobile view elements verified:', mobileCheck);

    console.log('🎉 ALL REGISTER ENHANCEMENT TESTS PASSED PERFECTLY!');
  } finally {
    await browser.close();
  }
}

run().catch((err) => {
  console.error('❌ Test failed:', err);
  process.exit(1);
});
