import puppeteer from 'puppeteer';
import path from 'path';
import fs from 'fs';
import { fileURLToPath } from 'url';
import { BASE_URL } from './lib/e2e.mjs';

// Read-only: opens the Reports tab, switches between the three reports, and checks that the
// selected report is remembered after visiting another tab.

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const screenshotsDir = path.resolve(__dirname, '../screenshots');
if (!fs.existsSync(screenshotsDir)) fs.mkdirSync(screenshotsDir, { recursive: true });

const REPORTS = [
  { button: 'Balance Sheet', heading: 'Balance Sheet' },
  { button: 'Income Statement (P&L)', heading: 'Income Statement (P&L)' },
  { button: 'Trial Balance', heading: 'Trial Balance' },
];

const clickButtonWithText = (page, text) =>
  page.evaluate((t) => {
    const btn = Array.from(document.querySelectorAll('button')).find((b) => b.innerText.trim() === t);
    if (btn) btn.click();
    return Boolean(btn);
  }, text);

const visibleHeadings = (page) =>
  page.evaluate(() => Array.from(document.querySelectorAll('main h2')).map((h) => h.innerText.trim()));

const activeReportButton = (page) =>
  page.evaluate(() => {
    const labels = ['Balance Sheet', 'Income Statement (P&L)', 'Trial Balance'];
    const btn = Array.from(document.querySelectorAll('main button')).find(
      (b) => labels.includes(b.innerText.trim()) && b.className.includes('bg-emerald-600')
    );
    return btn ? btn.innerText.trim() : null;
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
      await new Promise((r) => setTimeout(r, 2000));
    }

    console.log('2. Opening the Reports tab...');
    if (!(await clickButtonWithText(page, 'Reports'))) throw new Error('Reports nav button not found');
    await new Promise((r) => setTimeout(r, 1500));

    for (const { button, heading } of REPORTS) {
      console.log(`3. Switching to "${button}"...`);
      if (!(await clickButtonWithText(page, button))) throw new Error(`Report button "${button}" not found`);
      await new Promise((r) => setTimeout(r, 1500));
      const headings = await visibleHeadings(page);
      if (!headings.includes(heading)) {
        throw new Error(`Expected heading "${heading}", found: ${JSON.stringify(headings)}`);
      }
      const active = await activeReportButton(page);
      if (active !== button) throw new Error(`Expected "${button}" to be highlighted, found "${active}"`);
      console.log(`✅ "${heading}" rendered and highlighted`);
    }
    await page.screenshot({ path: path.join(screenshotsDir, '46-reports-trial-balance.png') });

    console.log('4. Visiting Accounts and returning to Reports...');
    await clickButtonWithText(page, 'Accounts');
    await new Promise((r) => setTimeout(r, 1000));
    await clickButtonWithText(page, 'Reports');
    await new Promise((r) => setTimeout(r, 1500));
    const remembered = await activeReportButton(page);
    if (remembered !== 'Trial Balance') {
      throw new Error(`Selected report was not remembered across tabs (found "${remembered}")`);
    }
    console.log('✅ Selected report remembered after switching tabs');
    console.log('🎉 Reports tab test passed!');
  } finally {
    await browser.close();
  }
})().catch((err) => {
  console.error('❌ Test failed:', err);
  process.exit(1);
});
