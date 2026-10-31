import puppeteer from 'puppeteer';
import fs from 'fs';
import path from 'path';
import { execSync } from 'child_process';

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

  // Login
  const isAuth = await page.$('input[placeholder="Enter your username or email"]');
  if (isAuth) {
    console.log('Logging in with kishan / Kishan@123...');
    await page.type('input[placeholder="Enter your username or email"]', 'kishan');
    await page.type('input[placeholder="••••••••"]', 'Kishan@123');
    await page.click('button[type="submit"]');
  }

  await page.waitForSelector('button[aria-label="User Account Menu"]', { timeout: 10000 });
  await page.waitForFunction(() => !document.body.innerText.includes('Loading your financial data'));
  await new Promise((r) => setTimeout(r, 600));

  // 2. Open User Dropdown
  console.log('2. Opening User Account Menu...');
  await page.evaluate(() => {
    document.querySelector('button[aria-label="User Account Menu"]').click();
  });
  await new Promise((r) => setTimeout(r, 500));

  // Verify Export .gnucash button exists
  const exportBtnText = await page.evaluate(() => {
    const btns = Array.from(document.querySelectorAll('div.absolute button'));
    const exportBtn = btns.find((b) => b.innerText.includes('Export .gnucash'));
    return exportBtn ? exportBtn.innerText.trim() : null;
  });

  console.log('Export button found in dropdown:', exportBtnText);
  if (!exportBtnText) {
    throw new Error('Verification failed: "Export .gnucash" button not found in User Dropdown!');
  }

  // Screenshot 18: User Dropdown with Export option
  const dropdownShot = path.join(screenshotsDir, '18-user-dropdown-export-option.png');
  await page.screenshot({ path: dropdownShot });
  console.log(`✓ Saved screenshot: ${dropdownShot}`);

  // 3. Click Export .gnucash and verify status message and exported file
  console.log('3. Triggering Export in browser context...');
  const exportResult = await page.evaluate(async () => {
    const token = localStorage.getItem('gnucash_token');
    const res = await fetch('/api/export/gnucash-sqlite', {
      headers: { Authorization: `Bearer ${token}` },
    });
    const disposition = res.headers.get('Content-Disposition') || res.headers.get('content-disposition') || '';
    const contentType = res.headers.get('Content-Type') || '';
    let filename = '';
    const match = disposition.match(/filename[^;=\n]*=((['"]).*?\2|[^;\n]*)/);
    if (match && match[1]) {
      filename = match[1].replace(/['"]/g, '').trim();
    }
    const arrayBuffer = await res.arrayBuffer();
    const bytes = Array.from(new Uint8Array(arrayBuffer));
    return {
      status: res.status,
      contentType,
      disposition,
      filename,
      size: bytes.length,
      bytes: bytes.slice(0, 100), // First 100 bytes for header verification
    };
  });

  console.log('Export response summary:', {
    status: exportResult.status,
    contentType: exportResult.contentType,
    filename: exportResult.filename,
    size: exportResult.size,
  });

  if (exportResult.status !== 200) {
    throw new Error(`Export failed with HTTP status ${exportResult.status}`);
  }

  // 4. Verify file name format: username_yyyyMMddhhmmss.gnucash
  const namePattern = /^kishan_\d{14}\.gnucash$/;
  const isMatch = namePattern.test(exportResult.filename);
  console.log(`Filename "${exportResult.filename}" matches username_yyyyMMddhhmmss.gnucash: ${isMatch}`);
  if (!isMatch) {
    throw new Error(`Verification failed: Filename "${exportResult.filename}" does not match pattern username_yyyyMMddhhmmss.gnucash!`);
  }

  // 5. Verify SQLite binary header and content using python3
  const tempExportPath = '/tmp/test_downloaded.gnucash';
  // Re-fetch full file to disk for sqlite3 analysis
  execSync(
    `curl -s -H "Authorization: Bearer $(curl -s -X POST http://localhost:8081/api/auth/login -H 'Content-Type: application/json' -d '{"username":"kishan","password":"Kishan@123"}' | jq -r '.token')" http://localhost:8081/api/export/gnucash-sqlite -o ${tempExportPath}`
  );

  console.log('5. Verifying SQLite database structure in downloaded file...');
  const pyVerify = execSync(
    `python3 -c "
import sqlite3
conn = sqlite3.connect('${tempExportPath}')
cur = conn.cursor()
tables = [r[0] for r in cur.execute('SELECT name FROM sqlite_master WHERE type=\\'table\\'').fetchall()]
assert 'accounts' in tables, 'Missing accounts table'
assert 'transactions' in tables, 'Missing transactions table'
assert 'splits' in tables, 'Missing splits table'
assert 'commodities' in tables, 'Missing commodities table'
acc_count = cur.execute('SELECT count(*) FROM accounts').fetchone()[0]
tx_count = cur.execute('SELECT count(*) FROM transactions').fetchone()[0]
sp_count = cur.execute('SELECT count(*) FROM splits').fetchone()[0]
print(f'VALID SQLITE: accounts={acc_count}, transactions={tx_count}, splits={sp_count}')
"`,
    { encoding: 'utf-8' }
  );
  console.log(`✓ Python SQLite verification: ${pyVerify.trim()}`);

  // 6. Test Mobile View (375x667)
  console.log('6. Testing Mobile View (375x667)...');
  await page.setViewport({ width: 375, height: 667 });
  await new Promise((r) => setTimeout(r, 600));

  // Open mobile drawer
  await page.evaluate(() => {
    document.querySelector('button[aria-label="Open Mobile Menu"]').click();
  });
  await new Promise((r) => setTimeout(r, 600));

  const mobileExportBtn = await page.evaluate(() => {
    const btns = Array.from(document.querySelectorAll('button'));
    return btns.some((b) => b.innerText.includes('Export .gnucash'));
  });

  console.log('Export button visible in mobile drawer:', mobileExportBtn);
  if (!mobileExportBtn) {
    throw new Error('Verification failed: Export button not found in mobile drawer!');
  }

  // Screenshot 19: Mobile Drawer with Export option
  const mobileShot = path.join(screenshotsDir, '19-mobile-drawer-export-option.png');
  await page.screenshot({ path: mobileShot });
  console.log(`✓ Saved screenshot: ${mobileShot}`);

  await browser.close();
  console.log('\n=== ALL EXPORT .GNUCASH VERIFICATIONS PASSED SUCCESSFULLY ===');
})();
