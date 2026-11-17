import puppeteer from 'puppeteer';
import path from 'path';
import fs from 'fs';
import { fileURLToPath } from 'url';
import { BASE_URL } from './lib/e2e.mjs';

// Read-only: opens the Reconcile tab, ticks every listed split, sets the statement ending
// balance to the cleared balance, and checks the difference reaches exactly zero (and is
// non-zero one cent away). It never clicks "Finish Reconcile".

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const screenshotsDir = path.resolve(__dirname, '../screenshots');
if (!fs.existsSync(screenshotsDir)) fs.mkdirSync(screenshotsDir, { recursive: true });

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

const clickButtonWithText = (page, text) =>
  page.evaluate((t) => {
    const btns = Array.from(document.querySelectorAll('button')).filter((b) => b.innerText.trim() === t);
    btns.forEach((b) => b.click());
    return btns.length;
  }, text);

// Reads the value shown under a summary label, e.g. "Cleared Balance" -> "₹9,144.91"
const summaryValue = (page, label) =>
  page.evaluate((l) => {
    const span = Array.from(document.querySelectorAll('span')).find((s) => s.innerText.trim().toLowerCase() === l.toLowerCase());
    return span && span.nextElementSibling ? span.nextElementSibling.innerText.trim() : null;
  }, label);

const toNumber = (text) => Number(text.replace(/[^0-9.-]/g, ''));

const setStatementBalance = (page, value) =>
  page.evaluate((val) => {
    const el = document.querySelector('input[type="number"][placeholder="0.00"]');
    if (!el) return false;
    const tracker = el._valueTracker;
    if (tracker) tracker.setValue('');
    Object.getOwnPropertyDescriptor(window.HTMLInputElement.prototype, 'value').set.call(el, val);
    el.dispatchEvent(new Event('input', { bubbles: true }));
    return true;
  }, value);

const finishEnabled = (page) =>
  page.evaluate(() => {
    const btn = Array.from(document.querySelectorAll('button')).find((b) => b.innerText.includes('Finish Reconcile'));
    return btn ? !btn.disabled : null;
  });

(async () => {
  const browser = await puppeteer.launch({ headless: 'new', args: ['--no-sandbox', '--disable-setuid-sandbox'] });
  try {
    const page = await browser.newPage();
    await page.setViewport({ width: 1440, height: 900 });

    console.log(`1. Navigating to ${BASE_URL} and logging in...`);
    await page.goto(BASE_URL, { waitUntil: 'networkidle2' });
    if (await page.$('input[type="password"]')) {
      await page.type('input[type="text"]', 'kishan');
      await page.type('input[type="password"]', 'Kishan@123');
      await page.click('button[type="submit"]');
      await sleep(2000);
    }

    console.log('2. Opening the Reconcile tab...');
    if (!(await clickButtonWithText(page, 'Reconcile'))) throw new Error('Reconcile nav button not found');
    await sleep(2000);

    console.log('3. Ticking every listed deposit and withdrawal...');
    // Each panel's "Select / Clear All" toggles that panel only, so press it per panel until the
    // panel is fully ticked (panels can start partly ticked when splits are already cleared)
    const panelStates = () =>
      page.evaluate(() =>
        Array.from(document.querySelectorAll('button'))
          .filter((b) => b.innerText.trim() === 'Select / Clear All')
          .map((b) => {
            const boxes = Array.from(b.closest('.flex-col').querySelectorAll('input[type="checkbox"]'));
            return { total: boxes.length, checked: boxes.filter((x) => x.checked).length };
          })
      );
    const clickPanelToggle = (index) =>
      page.evaluate((i) => {
        const btns = Array.from(document.querySelectorAll('button')).filter((b) => b.innerText.trim() === 'Select / Clear All');
        btns[i].click();
      }, index);

    const initial = await panelStates();
    if (initial.length !== 2) throw new Error(`Expected 2 reconcile panels, found ${initial.length}`);
    for (let i = 0; i < initial.length; i++) {
      for (let attempt = 0; attempt < 2; attempt++) {
        const panel = (await panelStates())[i];
        if (panel.checked === panel.total) break;
        await clickPanelToggle(i);
        await sleep(400);
      }
    }
    const boxes = (await panelStates()).reduce(
      (acc, p) => ({ total: acc.total + p.total, checked: acc.checked + p.checked }),
      { total: 0, checked: 0 }
    );
    if (boxes.total === 0) throw new Error('No uncleared splits listed; the seed data should provide some');
    if (boxes.checked !== boxes.total) throw new Error(`Only ${boxes.checked} of ${boxes.total} splits ticked`);

    // Independently total the listed amounts ("+₹850.00" / "-₹92.40") and compare with the summary
    const listed = await page.evaluate(() => {
      const amounts = Array.from(document.querySelectorAll('span'))
        .map((s) => s.innerText.trim())
        .filter((t) => /^[+-]₹[0-9,]+\.\d{2}$/.test(t));
      const cents = (t) => Math.round(Number(t.replace(/[^0-9.]/g, '')) * 100);
      return {
        depositCents: amounts.filter((t) => t.startsWith('+')).reduce((a, t) => a + cents(t), 0),
        withdrawalCents: amounts.filter((t) => t.startsWith('-')).reduce((a, t) => a + cents(t), 0),
      };
    });
    const centsOf = (text) => Math.round(toNumber(text) * 100);
    const starting = await summaryValue(page, 'Starting Balance');
    const cleared = await summaryValue(page, 'Cleared Balance');
    if (!starting || !cleared) throw new Error('Summary balances not shown');
    const expectedCleared = centsOf(starting) + listed.depositCents - listed.withdrawalCents; // checking is debit-normal
    if (centsOf(cleared) !== expectedCleared) {
      throw new Error(`Cleared balance ${cleared} does not equal starting ${starting} + listed deposits - withdrawals (${expectedCleared / 100})`);
    }
    if (listed.depositCents + listed.withdrawalCents === 0) throw new Error('Listed split amounts not found');
    const clearedNum = toNumber(cleared);
    console.log(`✅ Cleared balance ${cleared} = starting ${starting} + ${listed.depositCents / 100} - ${listed.withdrawalCents / 100}`);

    console.log('4. Setting statement ending balance to the cleared balance...');
    if (!(await setStatementBalance(page, clearedNum.toFixed(2)))) throw new Error('Statement balance input not found');
    await sleep(500);
    const diffZero = await summaryValue(page, 'Difference');
    if (toNumber(diffZero) !== 0) throw new Error(`Expected a zero difference, got ${diffZero}`);
    if ((await finishEnabled(page)) !== true) throw new Error('Finish Reconcile should be enabled at zero difference');
    console.log(`✅ Difference ${diffZero}, Finish Reconcile enabled`);
    await page.screenshot({ path: path.join(screenshotsDir, '47-reconcile-zero-difference.png') });

    console.log('5. Setting statement ending balance one cent higher...');
    await setStatementBalance(page, (clearedNum + 0.01).toFixed(2));
    await sleep(500);
    const diffCent = await summaryValue(page, 'Difference');
    if (Math.abs(toNumber(diffCent)) !== 0.01) throw new Error(`Expected a 0.01 difference, got ${diffCent}`);
    if ((await finishEnabled(page)) !== false) throw new Error('Finish Reconcile should be disabled when not reconciled');
    console.log(`✅ Difference ${diffCent}, Finish Reconcile disabled`);

    console.log('🎉 Reconcile view test passed (no reconciliation was submitted)!');
  } finally {
    await browser.close();
  }
})().catch((err) => {
  console.error('❌ Test failed:', err);
  process.exit(1);
});
