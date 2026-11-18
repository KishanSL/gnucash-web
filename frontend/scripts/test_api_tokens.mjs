import puppeteer from 'puppeteer';
import { BASE_URL, requireIsolatedEnv } from './lib/e2e.mjs';

requireIsolatedEnv('test_api_tokens');

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

async function domClick(page, selector) {
  await page.waitForSelector(selector, { timeout: 10000 });
  await page.$eval(selector, (node) => node.click());
}

/**
 * Sets a controlled React input reliably: reset the value tracker, use the native setter and
 * dispatch a bubbling input event (see AGENTS.md "Controlled Inputs Tracker Reset").
 */
async function setInputValue(page, selector, value) {
  await page.waitForSelector(selector, { timeout: 10000 });
  await page.$eval(
    selector,
    (el, next) => {
      const input = el;
      const tracker = input._valueTracker;
      if (tracker) tracker.setValue('');
      const setter = Object.getOwnPropertyDescriptor(
        window.HTMLInputElement.prototype,
        'value'
      )?.set;
      setter?.call(input, next);
      input.dispatchEvent(new Event('input', { bubbles: true }));
    },
    value
  );
}

async function clickByText(page, selector, text) {
  const elements = await page.$$(selector);
  for (const el of elements) {
    const content = await page.evaluate((node) => node.textContent || '', el);
    if (content.includes(text)) {
      // Use a DOM click instead of a coordinate click: the user menu is an absolutely
      // positioned popover, and this avoids hitting an overlay at the computed point.
      await el.evaluate((node) => node.click());
      return true;
    }
  }
  return false;
}

(async () => {
  const browser = await puppeteer.launch({
    headless: true,
    args: ['--no-sandbox', '--disable-setuid-sandbox'],
  });

  try {
    const context = browser.defaultBrowserContext();
    await context.overridePermissions(BASE_URL, ['clipboard-read', 'clipboard-write']);

    const page = await browser.newPage();
    await page.setViewport({ width: 1440, height: 900 });
    page.on('pageerror', (err) => console.error('PAGE ERROR:', err));
    page.on('console', (msg) => {
      if (msg.type() === 'error') console.error('CONSOLE ERROR:', msg.text());
    });

    console.log(`1. Navigating to ${BASE_URL}...`);
    await page.goto(BASE_URL, { waitUntil: 'networkidle0' });

    // --- Login ---
    console.log('2. Logging in as kishan...');
    await page.type('input[placeholder="Enter your username or email"]', 'kishan');
    await page.type('input[placeholder="••••••••"]', 'Kishan@123');
    await page.click('button[type="submit"]');
    await page.waitForSelector('button[aria-label="User Account Menu"]', { timeout: 15000 });

    // --- Open the user menu: API Tokens sits between Change Password and Reset Entire Database ---
    console.log('3. Opening the user menu...');
    await page.click('button[aria-label="User Account Menu"]');
    await sleep(300);

    const menuText = await page.evaluate(() => {
      const btn = document.querySelector('button[aria-label="User Account Menu"]');
      return btn?.parentElement?.innerText || '';
    });
    const changeIdx = menuText.indexOf('Change Password');
    const apiIdx = menuText.indexOf('API Tokens');
    const resetIdx = menuText.indexOf('Reset Entire Database');
    if (changeIdx < 0 || apiIdx < 0 || resetIdx < 0) {
      throw new Error(`User menu is missing entries (UI-2): ${JSON.stringify(menuText)}`);
    }
    if (!(changeIdx < apiIdx && apiIdx < resetIdx)) {
      throw new Error('API Tokens is not between Change Password and Reset Entire Database (UI-2)');
    }
    console.log('✅ Confirmed: API Tokens is in the documented user-menu position (UI-2).');

    console.log('4. Opening the API Tokens modal...');
    if (!(await clickByText(page, 'button', 'API Tokens'))) {
      throw new Error('API Tokens menu entry not clickable');
    }
    await page.waitForSelector('[data-testid="api-tokens-modal"]', { timeout: 5000 });
    await page.waitForSelector('[data-testid="api-token-empty"], [data-testid="api-token-row"]', {
      timeout: 5000,
    });
    await page.screenshot({ path: '/tmp/api-tokens-modal.png' });
    console.log('Saved /tmp/api-tokens-modal.png');

    // --- Create a token ---
    console.log('5. Creating a token...');
    const tokenName = `e2e_service_${Date.now()}`;
    await setInputValue(page, '[data-testid="api-token-name"]', tokenName);
    await page.select('[data-testid="api-token-expiry"]', '30');
    const typedName = await page.$eval('[data-testid="api-token-name"]', (el) => el.value);
    if (typedName !== tokenName) {
      throw new Error(`Token name input did not accept typing: ${JSON.stringify(typedName)}`);
    }
    await domClick(page, '[data-testid="api-token-create"]');

    await page.waitForSelector('[data-testid="api-token-value"]', { timeout: 10000 });
    const token = await page.$eval('[data-testid="api-token-value"]', (el) => el.textContent.trim());
    if (!token.startsWith('gcw_') || token.length !== 68) {
      throw new Error(`Unexpected token format shown once: ${token}`);
    }
    const bodyText = await page.evaluate(() => document.body.innerText);
    if (!bodyText.includes('You will not see this token again')) {
      throw new Error('Missing show-once warning for a new token (UI-8)');
    }

    // The copy affordance must be present and usable. Headless Chrome blocks clipboard writes
    // (both execCommand and the async Clipboard API), so assert the button and that the plaintext
    // token stays visible instead of depending on the transient "Copied" label.
    const copyButton = await page.$eval('[data-testid="api-token-copy"]', (el) => ({
      label: el.textContent?.trim() || '',
      disabled: el.disabled,
    }));
    if (!copyButton.label) {
      throw new Error('Copy button has no label');
    }
    if (copyButton.disabled) {
      throw new Error('Copy button is disabled while the token is visible');
    }
    await domClick(page, '[data-testid="api-token-copy"]');
    const tokenAfterCopy = await page.$eval(
      '[data-testid="api-token-value"]',
      (el) => el.textContent?.trim() || ''
    );
    if (tokenAfterCopy !== token) {
      throw new Error('Plaintext token is no longer visible after clicking copy');
    }
    console.log('✅ Confirmed: token shown once with a copy control, warning present.');

    // --- The token authenticates as the user ---
    console.log('6. Using the token against the API...');
    const accountsRes = await fetch(`${BASE_URL}/api/accounts`, {
      headers: { Authorization: `Bearer ${token}` },
    });
    if (accountsRes.status !== 200) {
      throw new Error(`Token could not read accounts: HTTP ${accountsRes.status}`);
    }
    const accounts = await accountsRes.json();
    if (!Array.isArray(accounts) || accounts.length === 0) {
      throw new Error('Token returned no accounts for the seeded user');
    }
    // Session-only endpoints must refuse the token (SEC-18).
    const tokensRes = await fetch(`${BASE_URL}/api/auth/tokens`, {
      headers: { Authorization: `Bearer ${token}` },
    });
    if (tokensRes.status !== 403) {
      throw new Error(`Token managed tokens: expected 403, got ${tokensRes.status}`);
    }
    const resetRes = await fetch(`${BASE_URL}/api/reset-data`, {
      method: 'POST',
      headers: { Authorization: `Bearer ${token}` },
    });
    if (resetRes.status !== 403) {
      throw new Error(`Token reset data: expected 403, got ${resetRes.status}`);
    }
    console.log('✅ Confirmed: token works on the ledger API and gets 403 on protected endpoints.');

    // --- Revoke it, with confirmation ---
    console.log('7. Revoking the token...');
    const rowSelector = `[data-testid="api-token-row"][data-token-name="${tokenName}"]`;
    await page.waitForSelector(rowSelector, { timeout: 5000 });
    await domClick(page, `${rowSelector} [data-testid="api-token-revoke"]`);
    await page.waitForSelector(`${rowSelector} [data-testid="api-token-confirm-revoke"]`, {
      timeout: 5000,
    });
    await domClick(page, `${rowSelector} [data-testid="api-token-confirm-revoke"]`);

    await page.waitForFunction(
      (selector) =>
        document.querySelector(selector)?.getAttribute('data-token-status') === 'Revoked',
      { timeout: 10000 },
      rowSelector
    );
    await page.screenshot({ path: '/tmp/api-tokens-revoked.png' });
    console.log('Saved /tmp/api-tokens-revoked.png');

    const afterRevoke = await fetch(`${BASE_URL}/api/accounts`, {
      headers: { Authorization: `Bearer ${token}` },
    });
    if (afterRevoke.status !== 401) {
      throw new Error(`Revoked token still works: HTTP ${afterRevoke.status}`);
    }
    console.log('✅ Confirmed: revoked token is rejected with 401.');

    // --- Close and re-open: revoked state persists, plaintext is gone ---
    console.log('8. Re-opening the modal...');
    await domClick(page, '[data-testid="api-token-close"]');
    await sleep(300);
    await domClick(page, 'button[aria-label="User Account Menu"]');
    await sleep(300);
    if (!(await clickByText(page, 'button', 'API Tokens'))) {
      console.error(
        'USER MENU AFTER RE-OPEN:\n',
        await page.evaluate(() => document.body.innerText)
      );
      throw new Error('API Tokens menu entry not clickable on re-open');
    }
    await page.waitForSelector(rowSelector, { timeout: 5000 });
    const reopenedText = await page.evaluate(
      () => document.querySelector('[data-testid="api-tokens-modal"]')?.innerText || ''
    );
    if (reopenedText.includes(token)) {
      throw new Error('Plaintext token was fetched again after re-opening the modal');
    }
    const reopenedStatus = await page.$eval(rowSelector, (el) =>
      el.getAttribute('data-token-status')
    );
    if (reopenedStatus !== 'Revoked') {
      throw new Error(`Revoked token status did not persist: ${reopenedStatus}`);
    }
    console.log('✅ Confirmed: the plaintext token is never shown again and status persists.');

    console.log('\n🎉 API Tokens end-to-end verification passed.');
  } finally {
    await browser.close();
  }
})();
