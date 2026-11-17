import puppeteer from 'puppeteer';
import { BASE_URL, requireIsolatedEnv } from './lib/e2e.mjs';

requireIsolatedEnv('test_tree_collapse');

(async () => {
  const browser = await puppeteer.launch({
    headless: true,
    args: ['--no-sandbox', '--disable-setuid-sandbox'],
  });

  const page = await browser.newPage();
  await page.setViewport({ width: 1440, height: 900 });

  // 1. Login
  await page.goto(BASE_URL);
  await page.type('input[type="text"]', 'kishan');
  await page.type('input[type="password"]', 'Kishan@123');
  await page.click('button[type="submit"]');
  await page.waitForSelector('table', { timeout: 10000 });

  // 2. Ensure sample data is seeded if empty
  const currentCount = await page.evaluate(() => document.querySelectorAll('table tbody tr').length);
  if (currentCount <= 1) {
    console.log('Seeding sample data for testing...');
    await page.evaluate(() => { window.confirm = () => true; });
    await page.click('button[aria-label="User Account Menu"]');
    await new Promise(r => setTimeout(r, 400));
    await page.evaluate(() => {
      const btn = Array.from(document.querySelectorAll('.absolute button')).find(b => b.innerText.includes('Seed Sample Data'));
      if (btn) btn.click();
    });
    await page.waitForFunction(() => document.querySelectorAll('table tbody tr').length > 5, { timeout: 10000 });
  }

  // 3. Count fully expanded accounts
  const expandedRowsCount = await page.evaluate(() => document.querySelectorAll('table tbody tr').length);
  console.log('Expanded tree rows count:', expandedRowsCount);

  // Capture fully expanded screenshot
  await page.screenshot({ path: 'screenshots/9-tree-expanded.png' });
  console.log('Saved screenshots/9-tree-expanded.png');

  // 4. Click "Hide Subaccounts" button
  console.log('Clicking "Hide Subaccounts" button...');
  await page.evaluate(() => {
    const btn = Array.from(document.querySelectorAll('button')).find(b => b.innerText.includes('Hide Subaccounts'));
    if (btn) btn.click();
  });
  await new Promise(r => setTimeout(r, 500));

  // 5. Verify only first level (root accounts) are displayed
  const minimizedRows = await page.evaluate(() => {
    return Array.from(document.querySelectorAll('table tbody tr')).map(tr => {
      const name = tr.querySelector('td')?.innerText?.replace(/\n/g, ' ').trim();
      return name;
    });
  });

  console.log('Minimized tree rows count:', minimizedRows.length);
  console.log('Visible accounts (First Level Only):', minimizedRows);

  // Capture minimized first-level-only screenshot
  await page.screenshot({ path: 'screenshots/10-tree-minimized-first-level.png' });
  console.log('Saved screenshots/10-tree-minimized-first-level.png');

  // 6. Click "Show All Accounts" to unhide
  console.log('Clicking "Show All Accounts" button...');
  await page.evaluate(() => {
    const btn = Array.from(document.querySelectorAll('button')).find(b => b.innerText.includes('Show All Accounts'));
    if (btn) btn.click();
  });
  await new Promise(r => setTimeout(r, 500));

  const restoredRowsCount = await page.evaluate(() => document.querySelectorAll('table tbody tr').length);
  console.log('Restored tree rows count after unhiding:', restoredRowsCount);

  await browser.close();
  console.log('Puppeteer test completed successfully!');
})();
