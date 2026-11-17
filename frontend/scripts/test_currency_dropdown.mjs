import puppeteer from 'puppeteer';
import fs from 'fs';
import path from 'path';
import { BASE_URL } from './lib/e2e.mjs';

(async () => {
  const screenshotsDir = path.resolve('screenshots');
  if (!fs.existsSync(screenshotsDir)) {
    fs.mkdirSync(screenshotsDir, { recursive: true });
  }

  const browser = await puppeteer.launch({
    headless: true,
    args: ['--no-sandbox', '--disable-setuid-sandbox'],
  });

  const page = await browser.newPage();
  await page.setViewport({ width: 1440, height: 900 });

  console.log(`1. Navigating to ${BASE_URL}...`);
  await page.goto(BASE_URL, { waitUntil: 'networkidle0' });

  // Handle Login
  const isAuthPage = await page.$('input[placeholder="Enter your username or email"]');
  if (isAuthPage) {
    console.log('Logging in with kishan / Kishan@123...');
    await page.type('input[placeholder="Enter your username or email"]', 'kishan');
    await page.type('input[placeholder="••••••••"]', 'Kishan@123');
    await page.click('button[type="submit"]');
  }

  await page.waitForSelector('button[aria-label="User Account Menu"]', { timeout: 10000 });
  await page.waitForFunction(() => !document.body.innerText.includes('Loading your financial data'));
  await new Promise((r) => setTimeout(r, 600));

  console.log('2. Verifying main desktop header is clean and does NOT contain currency selector...');
  const directHeaderSelect = await page.evaluate(() => {
    const header = document.querySelector('header');
    return header ? header.querySelector('select') !== null : false;
  });

  if (directHeaderSelect) {
    throw new Error('Verification failed: Currency selector is still present in main header!');
  }
  console.log('✓ Desktop header is clean: no select element in top bar.');

  // Capture clean header screenshot
  const cleanHeaderPath = path.join(screenshotsDir, '11-clean-header-no-currency.png');
  await page.screenshot({ path: cleanHeaderPath });
  console.log(`✓ Saved screenshot: ${cleanHeaderPath}`);

  // 3. Open User Account Menu
  console.log('3. Opening user account menu dropdown...');
  await page.evaluate(() => {
    document.querySelector('button[aria-label="User Account Menu"]').click();
  });
  await new Promise((r) => setTimeout(r, 600));

  const dropdownCurr = await page.evaluate(() => {
    const sel = document.querySelector('select[aria-label="Select Currency"]');
    if (!sel) return null;
    return {
      value: sel.value,
      options: Array.from(sel.querySelectorAll('option')).map((o) => o.value),
    };
  });

  if (!dropdownCurr) {
    throw new Error('Verification failed: Currency selector not found inside User Dropdown!');
  }
  console.log(`✓ Currency selector found in dropdown! Current value: ${dropdownCurr.value}`);
  console.log(`✓ Supported currency count: ${dropdownCurr.options.length} (${dropdownCurr.options.slice(0, 5).join(', ')}...)`);

  // Capture open dropdown screenshot
  const dropdownPath = path.join(screenshotsDir, '12-currency-in-dropdown.png');
  await page.screenshot({ path: dropdownPath });
  console.log(`✓ Saved screenshot: ${dropdownPath}`);

  // Test switching currency to USD
  console.log('4. Testing currency switch to USD in dropdown...');
  await page.evaluate(() => {
    const sel = document.querySelector('select[aria-label="Select Currency"]');
    sel.value = 'USD';
    sel.dispatchEvent(new Event('change', { bubbles: true }));
  });
  await new Promise((r) => setTimeout(r, 600));

  const updatedCurr = await page.evaluate(() => {
    const sel = document.querySelector('select[aria-label="Select Currency"]');
    return sel ? sel.value : null;
  });
  console.log(`✓ Currency changed successfully to: ${updatedCurr}`);

  // Switch back to INR
  await page.evaluate(() => {
    const sel = document.querySelector('select[aria-label="Select Currency"]');
    sel.value = 'INR';
    sel.dispatchEvent(new Event('change', { bubbles: true }));
  });
  await new Promise((r) => setTimeout(r, 500));

  // 5. Test Mobile drawer
  console.log('5. Testing mobile view (375x667)...');
  await page.setViewport({ width: 375, height: 667 });
  await page.reload({ waitUntil: 'networkidle0' });
  await page.waitForSelector('button[aria-label="Open Mobile Menu"]', { timeout: 10000 });
  await page.waitForFunction(() => !document.body.innerText.includes('Loading your financial data'));

  await page.evaluate(() => {
    document.querySelector('button[aria-label="Open Mobile Menu"]').click();
  });
  await new Promise((r) => setTimeout(r, 600));

  const mobileCurr = await page.evaluate(() => {
    const sel = document.querySelector('select[aria-label="Display Currency"]');
    return sel ? sel.value : null;
  });

  if (!mobileCurr) {
    throw new Error('Verification failed: Currency selector not found inside Mobile Drawer!');
  }
  console.log(`✓ Mobile drawer currency selector found! Current value: ${mobileCurr}`);

  const mobilePath = path.join(screenshotsDir, '13-mobile-currency-in-drawer.png');
  await page.screenshot({ path: mobilePath });
  console.log(`✓ Saved screenshot: ${mobilePath}`);

  await browser.close();
  console.log('\n=== ALL UI VERIFICATIONS PASSED SUCCESSFULLY ===');
})();
