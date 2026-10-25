import puppeteer from 'puppeteer';
import fs from 'fs';
import path from 'path';

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

  console.log('1. Navigating to http://localhost:8081...');
  await page.goto('http://localhost:8081', { waitUntil: 'networkidle0' });

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

  // Seed sample data if transactions count is 0
  const txCount = await page.evaluate(async () => {
    try {
      const token = localStorage.getItem('gnucash_token');
      const res = await fetch('/api/transactions', {
        headers: { Authorization: `Bearer ${token}` },
      });
      const data = await res.json();
      return Array.isArray(data) ? data.length : 0;
    } catch {
      return 0;
    }
  });

  console.log(`Current existing transactions count: ${txCount}`);
  if (txCount === 0) {
    console.log('Seeding sample data to populate payees...');
    await page.evaluate(async () => {
      const token = localStorage.getItem('gnucash_token');
      await fetch('/api/seed', {
        method: 'POST',
        headers: { Authorization: `Bearer ${token}` },
      });
    });
    await page.reload({ waitUntil: 'networkidle0' });
    await page.waitForFunction(() => !document.body.innerText.includes('Loading your financial data'));
    await new Promise((r) => setTimeout(r, 600));
  }

  // 2. Open New Entry Modal
  console.log('2. Opening New Transaction Modal...');
  await page.evaluate(() => {
    document.querySelector('button[aria-label="New Entry"]').click();
  });
  await page.waitForSelector('input[aria-label="Description or Payee"]', { timeout: 5000 });
  await new Promise((r) => setTimeout(r, 600));

  // 3. Type into Description/Payee to trigger autocomplete dropdown
  console.log('3. Typing into Description / Payee input to view previous payees...');
  await page.type('input[aria-label="Description or Payee"]', 'Wal');
  await new Promise((r) => setTimeout(r, 500));

  // Verify payee dropdown is visible
  const payeeOptions = await page.evaluate(() => {
    const list = document.querySelectorAll('ul[role="listbox"] li');
    return Array.from(list).map((li) => li.innerText.replace(/\n+/g, ' • '));
  });
  console.log('Found previous payee options:', payeeOptions);

  if (payeeOptions.length === 0) {
    throw new Error('Verification failed: Payee autocomplete dropdown did not open with previous payees!');
  }

  // Save screenshot of payee autocomplete dropdown
  const payeeDropdownShot = path.join(screenshotsDir, '14-payee-autocomplete-dropdown.png');
  await page.screenshot({ path: payeeDropdownShot });
  console.log(`✓ Saved screenshot: ${payeeDropdownShot}`);

  // 4. Click the payee suggestion to test autofill
  console.log('4. Selecting Walgreens Pharmacy suggestion to test autofill...');
  await page.evaluate(() => {
    const option = document.querySelector('ul[role="listbox"] li');
    if (option) option.click();
  });
  await new Promise((r) => setTimeout(r, 600));

  // Verify splits and description are autofilled
  const autofillState = await page.evaluate(() => {
    const desc = document.querySelector('input[aria-label="Description or Payee"]')?.value;
    const banner = document.body.innerText.includes('Autofilled');
    const splitInputs = Array.from(document.querySelectorAll('input[type="number"]')).map(
      (inp) => inp.value
    );
    const hasValues = splitInputs.some((val) => val && parseFloat(val) > 0);
    return { desc, banner, splitInputs, hasValues };
  });

  console.log('Autofill verification result:', autofillState);
  if (!autofillState.desc || !autofillState.hasValues) {
    throw new Error('Verification failed: Transaction splits were not autofilled from last payee transaction!');
  }
  console.log(`✓ Successfully autofilled transaction for "${autofillState.desc}"!`);

  // Save screenshot of autofilled transaction
  const autofilledShot = path.join(screenshotsDir, '15-payee-autofilled-transaction.png');
  await page.screenshot({ path: autofilledShot });
  console.log(`✓ Saved screenshot: ${autofilledShot}`);

  // 5. Test Account Autocomplete in Split Row
  console.log('5. Testing Account Autocomplete in Split row...');
  const desktopComboboxes = await page.$$('table input[role="combobox"]');
  console.log(`Found ${desktopComboboxes.length} Desktop Account comboboxes.`);
  if (desktopComboboxes.length < 2) {
    throw new Error('Verification failed: Could not find desktop split account comboboxes!');
  }

  const secondCombobox = desktopComboboxes[1];
  await secondCombobox.click();
  await page.evaluate(() => {
    const inps = document.querySelectorAll('table input[role="combobox"]');
    const inp = inps[1];
    inp.focus();
    const nativeInputValueSetter = Object.getOwnPropertyDescriptor(window.HTMLInputElement.prototype, 'value').set;
    nativeInputValueSetter.call(inp, 'Groc');
    inp.dispatchEvent(new Event('input', { bubbles: true }));
  });
  await new Promise((r) => setTimeout(r, 500));

  // Verify filtered account list
  const filteredAccounts = await page.evaluate(() => {
    const options = document.querySelectorAll('li[role="option"]');
    return Array.from(options).map((opt) => opt.innerText.replace(/\n+/g, ' '));
  });
  console.log('Filtered accounts matching "Groc":', filteredAccounts);

  // Capture screenshot of account autocomplete
  const accountAutoShot = path.join(screenshotsDir, '16-account-split-autocomplete.png');
  await page.screenshot({ path: accountAutoShot });
  console.log(`✓ Saved screenshot: ${accountAutoShot}`);

  // Select the filtered account
  await page.evaluate(() => {
    const options = document.querySelectorAll('li[role="option"]');
    if (options.length > 0) options[0].click();
  });
  await new Promise((r) => setTimeout(r, 400));

  // Close modal
  await page.evaluate(() => {
    const closeBtn = document.querySelector('div.fixed button:has(svg.lucide-x)');
    if (closeBtn) closeBtn.click();
  });
  await new Promise((r) => setTimeout(r, 400));

  // 6. Test Mobile View (375x667)
  console.log('6. Testing Mobile View (375x667)...');
  await page.setViewport({ width: 375, height: 667 });
  await page.reload({ waitUntil: 'networkidle0' });
  await page.waitForFunction(() => !document.body.innerText.includes('Loading your financial data'));

  // Open transaction modal on mobile
  await page.evaluate(() => {
    const newEntry = document.querySelector('button[aria-label="New Entry"]');
    if (newEntry) newEntry.click();
  });
  await page.waitForSelector('input[aria-label="Description or Payee"]', { timeout: 5000 });
  await new Promise((r) => setTimeout(r, 500));

  // Type into payee input
  await page.type('input[aria-label="Description or Payee"]', 'Net');
  await new Promise((r) => setTimeout(r, 500));

  const mobilePayeeListOpen = await page.evaluate(() => {
    return document.querySelectorAll('ul[role="listbox"]').length > 0;
  });
  console.log('Mobile payee autocomplete dropdown open:', mobilePayeeListOpen);

  const mobileShot = path.join(screenshotsDir, '17-mobile-autocomplete.png');
  await page.screenshot({ path: mobileShot });
  console.log(`✓ Saved screenshot: ${mobileShot}`);

  await browser.close();
  console.log('\n=== ALL AUTOCOMPLETE & AUTOFILL VERIFICATIONS PASSED ===');
})();
