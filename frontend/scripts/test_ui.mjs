import puppeteer from 'puppeteer';

(async () => {
  const browser = await puppeteer.launch({
    headless: true,
    args: ['--no-sandbox', '--disable-setuid-sandbox'],
  });

  const page = await browser.newPage();
  await page.setViewport({ width: 1440, height: 900 });

  console.log('Navigating to http://localhost:8081...');
  await page.goto('http://localhost:8081', { waitUntil: 'networkidle0' });

  // 1. Take Desktop Auth Screenshot
  await page.screenshot({ path: '/tmp/desktop-auth.png', fullPage: true });
  console.log('Saved /tmp/desktop-auth.png');

  // 2. Perform Login
  await page.type('input[placeholder="Enter your username or email"]', 'kishan');
  await page.type('input[placeholder="••••••••"]', 'Kishan@123');
  await page.click('button[type="submit"]');

  // 3. Wait for authenticated view navbar
  await page.waitForSelector('button[aria-label="User Account Menu"]', { timeout: 10000 });
  console.log('Logged in successfully!');

  // 4. Click User Account Menu Dropdown
  await page.click('button[aria-label="User Account Menu"]');
  await new Promise(r => setTimeout(r, 600));

  // 5. Capture screenshot of authenticated view with open dropdown
  await page.screenshot({ path: '/tmp/desktop-user-dropdown.png' });
  console.log('Saved /tmp/desktop-user-dropdown.png');

  // 6. Inspect dropdown text items
  const menuText = await page.evaluate(() => {
    const btn = document.querySelector('button[aria-label="User Account Menu"]');
    return btn?.parentElement?.innerText || '';
  });
  console.log('--- User Dropdown Content ---');
  console.log(menuText);
  console.log('-----------------------------');

  await browser.close();
})();
