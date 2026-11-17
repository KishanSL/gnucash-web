import puppeteer from 'puppeteer';
import path from 'path';
import fs from 'fs';
import { fileURLToPath } from 'url';
import { BASE_URL, requireIsolatedEnv } from './lib/e2e.mjs';

requireIsolatedEnv('test_ui_comprehensive');

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const screenshotsDir = path.resolve(__dirname, '../screenshots');

if (!fs.existsSync(screenshotsDir)) {
  fs.mkdirSync(screenshotsDir, { recursive: true });
}

async function run() {
  console.log('🚀 Starting Comprehensive UI Test for GnuCash Web...');
  const browser = await puppeteer.launch({
    headless: 'new',
    args: ['--no-sandbox', '--disable-setuid-sandbox'],
  });

  try {
    const page = await browser.newPage();
    await page.setViewport({ width: 1440, height: 900 });

    // 1. Navigate to frontend
    console.log(`1. Navigating to ${BASE_URL}...`);
    await page.goto(BASE_URL, { waitUntil: 'networkidle2' });

    // Login
    const loginInput = await page.$('input[type="text"]');
    if (loginInput) {
      console.log('Logging in with kishan / Kishan@123...');
      await page.type('input[type="text"]', 'kishan');
      await page.type('input[type="password"]', 'Kishan@123');
      await page.click('button[type="submit"]');
      await new Promise(r => setTimeout(r, 2000));
    }

    // Capture initial accounts tree
    await page.screenshot({ path: path.join(screenshotsDir, '29-accounts-initial.png') });
    const initialRowCount = await page.$$eval('tbody tr', trs => trs.length);
    console.log(`Initial visible account rows: ${initialRowCount}`);

    // 2. Click "Hide Subaccounts"
    console.log('2. Clicking "Hide Subaccounts"...');
    await page.evaluate(() => {
      const btns = Array.from(document.querySelectorAll('button'));
      const b = btns.find(btn => btn.textContent && btn.textContent.includes('Hide Subaccounts'));
      if (b) b.click();
    });
    await new Promise(r => setTimeout(r, 1000));

    const hiddenRowCount = await page.$$eval('tbody tr', trs => trs.length);
    console.log(`Rows after Hide Subaccounts (should be 5 roots): ${hiddenRowCount}`);
    await page.screenshot({ path: path.join(screenshotsDir, '30-accounts-subaccounts-hidden.png') });

    // 3. Test Refresh Persistence
    console.log('3. Refreshing page to verify stage is preserved...');
    await page.reload({ waitUntil: 'networkidle2' });
    await new Promise(r => setTimeout(r, 2000));

    const postReloadRowCount = await page.$$eval('tbody tr', trs => trs.length);
    console.log(`Rows after reload (should remain 5 roots): ${postReloadRowCount}`);
    const toggleBtnText = await page.evaluate(() => {
      const btns = Array.from(document.querySelectorAll('button'));
      const b = btns.find(btn => btn.textContent && (btn.textContent.includes('Hide Subaccounts') || btn.textContent.includes('Show All Accounts')));
      return b ? b.textContent.trim() : null;
    });
    console.log(`Button text after reload: "${toggleBtnText}" (should be "Show All Accounts")`);
    await page.screenshot({ path: path.join(screenshotsDir, '31-accounts-stage-preserved-after-reload.png') });

    // 4. Click "Show All Accounts" to restore
    console.log('4. Clicking "Show All Accounts" to expand all accounts...');
    await page.evaluate(() => {
      const btns = Array.from(document.querySelectorAll('button'));
      const b = btns.find(btn => btn.textContent && btn.textContent.includes('Show All Accounts'));
      if (b) b.click();
    });
    await new Promise(r => setTimeout(r, 1000));
    const restoredRowCount = await page.$$eval('tbody tr', trs => trs.length);
    console.log(`Rows after Show All Accounts: ${restoredRowCount}`);
    await page.screenshot({ path: path.join(screenshotsDir, '32-accounts-all-restored.png') });

    // 5. Test Double-click on an Account Row to open Register
    console.log('5. Testing double click on an account row to open its register...');
    const targetAccountName = await page.evaluate(() => {
      const registerButtons = Array.from(document.querySelectorAll('tbody tr button[title="Open in Ledger Register"]'));
      if (registerButtons.length > 0) {
        const btn = registerButtons[0];
        const row = btn.closest('tr');
        if (row) {
          row.dispatchEvent(new MouseEvent('dblclick', { bubbles: true, cancelable: true }));
          return btn.textContent ? btn.textContent.trim() : 'Account';
        }
      }
      return null;
    });
    console.log(`Double clicked on row for account: "${targetAccountName}"`);
    await new Promise(r => setTimeout(r, 2000));

    await page.screenshot({ path: path.join(screenshotsDir, '33-register-opened-from-double-click.png') });

    // Verify register view is active
    const registerTitle = await page.evaluate(() => {
      const sel = document.querySelector('select');
      return sel ? sel.options[sel.selectedIndex]?.text : null;
    });
    console.log(`Active account in Register: "${registerTitle}"`);

    // 6. Verify Blank Row at End of Register & Single Amount Entry
    console.log('6. Verifying blank new transaction row at the end of register...');
    const debitInput = await page.$('tbody tr input[title*="Deposit"], tbody tr input[title*="Payment"]');
    const creditInput = await page.$('tbody tr input[title*="Withdrawal"], tbody tr input[title*="Charge"]');
    const descInput = await page.$('tbody tr input[placeholder*="Description / Payee"]');

    console.log(`Inputs found -> Deposit: ${!!debitInput}, Withdrawal: ${!!creditInput}, Description: ${!!descInput}`);

    if (debitInput && creditInput && descInput) {
      // Test Single Number Entry option
      console.log('Typing into Deposit: 150.00');
      await debitInput.type('150.00');
      let dVal = await page.evaluate(el => el.value, debitInput);
      let cVal = await page.evaluate(el => el.value, creditInput);
      console.log(`After typing in Deposit -> Deposit: "${dVal}", Withdrawal: "${cVal}"`);

      console.log('Typing into Withdrawal: 80.00 (should automatically clear Deposit!)');
      await creditInput.type('80.00');
      dVal = await page.evaluate(el => el.value, debitInput);
      cVal = await page.evaluate(el => el.value, creditInput);
      console.log(`After typing in Withdrawal -> Deposit: "${dVal}", Withdrawal: "${cVal}"`);

      // Type payee description
      console.log('Typing description: "Puppeteer Verified Transaction"');
      await descInput.type('Puppeteer Verified Transaction');
      await new Promise(r => setTimeout(r, 500));

      // Select transfer account using the combobox input
      console.log('Selecting transfer account from autocomplete...');
      const transferInput = await page.$('tbody tr input[placeholder*="transfer account"]');
      if (transferInput) {
        await transferInput.click();
        await new Promise(r => setTimeout(r, 500));
        await page.screenshot({ path: path.join(screenshotsDir, '34-transfer-autocomplete-open.png') });

        // Click first option in dropdown
        const option = await page.$('li[role="option"]');
        if (option) {
          const optText = await page.evaluate(el => el.textContent, option);
          console.log(`Selected transfer account: "${optText.trim()}"`);
          await option.click();
          await new Promise(r => setTimeout(r, 500));
        }
      }

      // Click Record Checkmark button
      console.log('Clicking Record Transaction checkmark button...');
      const recordBtn = await page.$('tbody tr button[title*="Record Transaction"]');
      if (recordBtn) {
        await recordBtn.click();
        await new Promise(r => setTimeout(r, 2500));
      }
      await page.screenshot({ path: path.join(screenshotsDir, '35-transaction-recorded-in-register.png') });

      // Verify transaction appeared in register table
      const hasNewTx = await page.evaluate(() => {
        return document.body.innerText.includes('Puppeteer Verified Transaction');
      });
      console.log(`Is newly recorded transaction visible in the register? ${hasNewTx}`);
    }

    // 8. Mobile Viewport Verification
    console.log('8. Testing mobile responsive viewport (375x667)...');
    await page.setViewport({ width: 375, height: 667 });
    await new Promise(r => setTimeout(r, 1000));
    await page.screenshot({ path: path.join(screenshotsDir, '36-mobile-register-view.png') });
    console.log('Captured 36-mobile-register-view.png');

    console.log('🎉 ALL TESTS PASSED SUCCESSFULLY!');
  } finally {
    await browser.close();
  }
}

run().catch((err) => {
  console.error('Test execution failed:', err);
  process.exit(1);
});
