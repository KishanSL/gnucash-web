import puppeteer from 'puppeteer';
import { execSync } from 'child_process';
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
  console.log('🚀 Running Fund Account Decimals Verification (ACC-3 & INV-5)...');

  // 1. Ensure user 'kishan' has an account with commodity_fraction = 1000
  console.log('1. Ensuring test fund account exists with commodity_fraction = 1000 in DB...');
  const checkCmd = `podman exec -i gnucash-postgres psql -U gnucash -d gnucash -t -c "SELECT id FROM accounts WHERE user_id = (SELECT id FROM users WHERE username = 'kishan') AND commodity_fraction = 1000 LIMIT 1;"`;
  let fundAccId = execSync(checkCmd, { encoding: 'utf-8' }).trim();
  if (!fundAccId) {
    const insertCmd = `podman exec -i gnucash-postgres psql -U gnucash -d gnucash -c "INSERT INTO accounts (id, name, account_type, currency, user_id, commodity_namespace, commodity_fraction) VALUES (gen_random_uuid(), 'Aditya Birla Mutual Fund', 'asset', 'INF209K01VA3', (SELECT id FROM users WHERE username = 'kishan'), 'FUND', 1000);"`;
    execSync(insertCmd);
    fundAccId = execSync(checkCmd, { encoding: 'utf-8' }).trim();
    console.log(`Created new fund account with id: ${fundAccId}`);
  } else {
    console.log(`Found existing fund account with id: ${fundAccId}`);
  }

  const browser = await puppeteer.launch({
    headless: 'new',
    args: ['--no-sandbox', '--disable-setuid-sandbox'],
  });

  try {
    const page = await browser.newPage();
    await page.setViewport({ width: 1440, height: 900 });

    // 2. Navigate & Login
    console.log('2. Navigating to http://localhost:8081...');
    await page.goto('http://localhost:8081', { waitUntil: 'networkidle2' });

    const loginInput = await page.$('input[type="text"]');
    if (loginInput) {
      console.log('Logging in with kishan / Kishan@123...');
      await page.type('input[type="text"]', 'kishan');
      await page.type('input[type="password"]', 'Kishan@123');
      await page.click('button[type="submit"]');
      await new Promise((r) => setTimeout(r, 2000));
    }

    // 3. Open Register for Aditya Birla Mutual Fund
    console.log('3. Opening register for Aditya Birla Mutual Fund via double-click...');
    const openedAccount = await page.evaluate(() => {
      const rows = Array.from(document.querySelectorAll('tbody tr'));
      const fundRow = rows.find((r) => r.innerText.includes('Aditya Birla Mutual Fund'));
      if (fundRow) {
        fundRow.dispatchEvent(new MouseEvent('dblclick', { bubbles: true, cancelable: true }));
        return 'Aditya Birla Mutual Fund';
      }
      return null;
    });

    if (!openedAccount) {
      throw new Error('Aditya Birla Mutual Fund account row not found in accounts tree!');
    }
    console.log(`Opened register for: "${openedAccount}"`);
    await new Promise((r) => setTimeout(r, 2000));

    // 4. Verify Ending Balance in header has exactly 2 decimals (INV-5: commodity fraction must not affect display balance)
    const endingBalanceText = await page.evaluate(() => {
      const el = document.querySelector('.text-xl.sm\\:text-2xl.font-bold.font-mono');
      return el ? el.textContent?.trim() : null;
    });
    console.log(`Ending balance display in header: "${endingBalanceText}"`);
    if (endingBalanceText) {
      const match = endingBalanceText.match(/\.(\d+)/);
      if (match && match[1].length !== 2) {
        throw new Error(`Ending balance has ${match[1].length} decimal places, expected exactly 2! (INV-5 violation)`);
      }
    }
    console.log('✅ Display balance confirmed formatted to 2 decimals.');

    // 5. Intercept POST /api/transactions
    let capturedPayload = null;
    let postResponseStatus = null;
    page.on('request', (req) => {
      if (req.url().includes('/api/transactions') && req.method() === 'POST') {
        try {
          capturedPayload = JSON.parse(req.postData() || '{}');
          console.log('Captured POST /api/transactions payload:', JSON.stringify(capturedPayload));
        } catch (e) {
          console.error('Failed to parse POST body:', e);
        }
      }
    });

    page.on('response', (res) => {
      if (res.url().includes('/api/transactions') && res.request().method() === 'POST') {
        postResponseStatus = res.status();
        console.log(`POST /api/transactions response status: ${postResponseStatus}`);
      }
    });

    // 6. Enter a transaction in the blank new row
    console.log('6. Entering new transaction on fund account in blank new row...');
    const uniquePayee = `Fund SIP ${Date.now()}`;

    await page.evaluate(({ payee }) => {
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
      setReactValue(debit, '500.50');
    }, { payee: uniquePayee });

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

    // 7. Verify Captured Payload and Server Response
    if (!capturedPayload) {
      throw new Error('Failed to capture POST /api/transactions request!');
    }

    console.log('7. Verifying split amounts in payload adhere to ACC-3 (<= 2 decimal places)...');
    for (const split of capturedPayload.splits) {
      console.log(`Split for account ${split.account_id}: amount = "${split.amount}"`);
      const parts = split.amount.split('.');
      if (parts.length > 1 && parts[1].length > 2) {
        throw new Error(`Split amount "${split.amount}" exceeds 2 decimal places! ACC-3 violation.`);
      }
    }

    if (postResponseStatus !== 200 && postResponseStatus !== 201) {
      throw new Error(`Backend rejected transaction with status ${postResponseStatus}!`);
    }

    console.log('✅ Transaction accepted by backend and split amounts strictly use 2 decimals.');
    await page.screenshot({ path: path.join(screenshotsDir, '45-fund-account-2-decimals-success.png') });
    console.log('Captured 45-fund-account-2-decimals-success.png');
    console.log('🎉 Fund account 2-decimal test passed completely!');
  } finally {
    await browser.close();
  }
}

run().catch((err) => {
  console.error('❌ Test failed:', err);
  process.exit(1);
});
