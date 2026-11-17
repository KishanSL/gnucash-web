import puppeteer from 'puppeteer';
import path from 'path';
import fs from 'fs';
import { fileURLToPath } from 'url';
import { BASE_URL } from './lib/e2e.mjs';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const screenshotsDir = path.resolve(__dirname, '../screenshots');

if (!fs.existsSync(screenshotsDir)) {
  fs.mkdirSync(screenshotsDir, { recursive: true });
}

async function testRegisterAutocomplete() {
  console.log('🚀 Running Register Complete History Autocomplete Test...');
  const browser = await puppeteer.launch({
    headless: 'new',
    args: ['--no-sandbox', '--disable-setuid-sandbox'],
  });

  try {
    const page = await browser.newPage();
    await page.setViewport({ width: 1440, height: 900 });

    console.log(`1. Navigating to ${BASE_URL}...`);
    await page.goto(BASE_URL, { waitUntil: 'networkidle2' });

    const loginInput = await page.$('input[type="text"]');
    if (loginInput) {
      console.log('Logging in...');
      await page.type('input[type="text"]', 'kishan');
      await page.type('input[type="password"]', 'Kishan@123');
      await page.click('button[type="submit"]');
      await new Promise((r) => setTimeout(r, 2000));
    }

    // Double-click Cash in Wallet row
    console.log('2. Double-clicking Cash in Wallet...');
    await page.evaluate(() => {
      const rows = Array.from(document.querySelectorAll('tbody tr'));
      const cashRow = rows.find((r) => r.innerText.includes('Cash in Wallet'));
      if (cashRow) {
        cashRow.dispatchEvent(new MouseEvent('dblclick', { bubbles: true, cancelable: true }));
      }
    });
    await new Promise((r) => setTimeout(r, 2000));

    // Scroll to blank new row
    console.log('3. Scrolling to blank new row...');
    await page.evaluate(() => {
      document.querySelector('tr[data-testid="blank-new-row-main"]')?.scrollIntoView({ block: 'center' });
    });
    await new Promise((r) => setTimeout(r, 500));

    // Click description input
    console.log('4. Clicking description input...');
    const descInput = await page.$('tr[data-testid="blank-new-row-main"] input[placeholder*="Description"]');
    if (!descInput) {
      throw new Error('Description input on blank new row not found!');
    }

    const beforeClick = await page.evaluate(() => {
      const tableRows = document.querySelectorAll('tbody tr');
      const allText = Array.from(tableRows).map(r => r.innerText.slice(0, 50));
      return {
        rowCount: tableRows.length,
        firstThreeRows: allText.slice(0, 3),
        lastThreeRows: allText.slice(-3),
      };
    });
    console.log('Before click debug:', JSON.stringify(beforeClick, null, 2));

    await descInput.focus();
    await descInput.click();
    await new Promise((r) => setTimeout(r, 500));

    await page.screenshot({ path: path.join(screenshotsDir, '50-register-autocomplete-open.png') });
    console.log('Captured 50-register-autocomplete-open.png');

    const dropdownData = await page.evaluate(() => {
      const dropdown = document.querySelector('[data-testid="payee-autocomplete-dropdown"]');
      const allDropdowns = document.querySelectorAll('.absolute');
      const dropdownHTMLs = Array.from(allDropdowns).map(d => d.outerHTML.slice(0, 100));
      if (!dropdown) return { open: false, items: [], allDropdownsCount: allDropdowns.length, dropdownHTMLs };
      const items = Array.from(dropdown.querySelectorAll('button')).map((b) => ({
        payee: b.querySelector('span.truncate')?.textContent?.trim() || '',
        count: b.querySelector('span.font-mono')?.textContent?.trim() || '',
      }));
      return { open: true, items };
    });

    console.log('Payee dropdown state:', JSON.stringify(dropdownData, null, 2));

    if (!dropdownData.open || dropdownData.items.length === 0) {
      throw new Error('Payee autocomplete dropdown did not open or has 0 items!');
    }

    // Select the first item
    const firstPayee = dropdownData.items[0].payee;
    console.log(`5. Selecting first payee: "${firstPayee}"...`);
    await page.evaluate((payeeText) => {
      const dropdown = document.querySelector('[data-testid="payee-autocomplete-dropdown"]');
      const btns = Array.from(dropdown?.querySelectorAll('button') || []);
      const targetBtn = btns.find((b) => b.textContent?.includes(payeeText));
      if (targetBtn) {
        targetBtn.dispatchEvent(new MouseEvent('click', { bubbles: true, cancelable: true }));
      }
    }, firstPayee);

    await new Promise((r) => setTimeout(r, 600));
    await page.screenshot({ path: path.join(screenshotsDir, '51-register-autocomplete-autofilled.png') });
    console.log('Captured 51-register-autocomplete-autofilled.png');

    // Verify fields were autofilled
    const filledValues = await page.evaluate(() => {
      const mainRow = document.querySelector('tr[data-testid="blank-new-row-main"]');
      const detailsRow = document.querySelector('tr[data-testid="blank-new-row-details"]');
      return {
        desc: mainRow?.querySelector('input[placeholder*="Description"]')?.value,
        transferAccount: mainRow?.querySelector('input[placeholder*="Select transfer account"]')?.value,
        debit: mainRow?.querySelector('input[placeholder*="Deposit"]')?.value,
        credit: mainRow?.querySelector('input[placeholder*="Withdrawal"]')?.value,
        notes: detailsRow?.querySelector('input[placeholder*="Optional notes"]')?.value,
        memo: detailsRow?.querySelector('input[placeholder*="Optional memo"]')?.value,
      };
    });

    console.log('Autofilled form values:', filledValues);

    if (filledValues.desc !== firstPayee) {
      throw new Error(`Expected description to be "${firstPayee}", got "${filledValues.desc}"`);
    }
    if (!filledValues.transferAccount) {
      throw new Error('Transfer account was not autofilled!');
    }

    // 6. Test typing in description to filter history
    console.log('6. Testing typing "Wal" to filter payee history...');
    await page.evaluate(() => {
      const input = document.querySelector('tr[data-testid="blank-new-row-main"] input[placeholder*="Description"]');
      if (input) {
        input.focus();
        const tracker = input._valueTracker;
        if (tracker) tracker.setValue('');
        Object.getOwnPropertyDescriptor(window.HTMLInputElement.prototype, 'value').set.call(input, 'Wal');
        input.dispatchEvent(new Event('input', { bubbles: true }));
      }
    });
    await new Promise((r) => setTimeout(r, 400));

    const filteredItems = await page.evaluate(() => {
      const dropdown = document.querySelector('[data-testid="payee-autocomplete-dropdown"]');
      if (!dropdown) return [];
      return Array.from(dropdown.querySelectorAll('button span.truncate')).map((el) => el.textContent.trim());
    });
    console.log('Filtered payee options for "Wal":', filteredItems);
    if (!filteredItems.some((name) => name.includes('Walgreens'))) {
      throw new Error('Filtering by "Wal" did not return "Walgreens Pharmacy"!');
    }

    await page.screenshot({ path: path.join(screenshotsDir, '52-register-autocomplete-filtered.png') });
    console.log('Captured 52-register-autocomplete-filtered.png');

    console.log('✅ ALL AUTOCOMPLETE VERIFICATIONS PASSED SUCCESSFULLY!');
  } catch (err) {
    console.error('❌ Test failed:', err);
    process.exit(1);
  } finally {
    await browser.close();
  }
}

testRegisterAutocomplete();
