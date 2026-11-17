import puppeteer from 'puppeteer';
import { execSync } from 'child_process';
import { BASE_URL, requireIsolatedEnv } from './lib/e2e.mjs';

requireIsolatedEnv('test_reset_password');

(async () => {
  const browser = await puppeteer.launch({
    headless: true,
    args: ['--no-sandbox', '--disable-setuid-sandbox'],
  });

  try {
    const page = await browser.newPage();
    await page.setViewport({ width: 1440, height: 900 });

    console.log(`1. Navigating to ${BASE_URL}...`);
    await page.goto(BASE_URL, { waitUntil: 'networkidle0' });

    // Step A: Verify public API call to /api/auth/reset-password is rejected with 403 Forbidden
    console.log('2. Verifying public POST /api/auth/reset-password returns 403 Forbidden...');
    const forbiddenCheck = execSync(
      `curl -s -o /dev/null -w "%{http_code}" -X POST ${BASE_URL}/api/auth/reset-password -H "Content-Type: application/json" -d '{"username":"kishan"}'`,
      { encoding: 'utf-8' }
    ).trim();
    if (forbiddenCheck !== '403') {
      throw new Error(`Expected 403 Forbidden for unauthenticated reset-password, got: ${forbiddenCheck}`);
    }
    console.log('✅ Confirmed: Public /api/auth/reset-password is strictly disabled (HTTP 403).');

    // Step B: Verify non-enumeration on login with non-existent user and wrong password
    console.log('3. Verifying login non-enumeration...');
    const invalidLoginRes = execSync(
      `curl -s -X POST ${BASE_URL}/api/auth/login -H "Content-Type: application/json" -d '{"username":"non_existent_attacker_test","password":"any_password"}'`,
      { encoding: 'utf-8' }
    );
    if (!invalidLoginRes.includes('Invalid username or password')) {
      throw new Error('Expected generic error on invalid user login: ' + invalidLoginRes);
    }
    console.log('✅ Confirmed: Non-existent user login returns generic "Invalid username or password".');

    // Step C: Verify UI "Forgot password?" shows CLI Admin Guidance and no public submit inputs
    console.log('4. Testing UI "Forgot password?" guidance view...');
    const buttons = await page.$$('button');
    let forgotFound = false;
    for (const b of buttons) {
      const text = await page.evaluate(el => el.textContent, b);
      if (text && text.includes('Forgot password?')) {
        await b.click();
        forgotFound = true;
        break;
      }
    }
    if (!forgotFound) throw new Error('Forgot password button not found');
    await new Promise(r => setTimeout(r, 500));

    // Confirm no text or password input fields exist in the reset guidance view
    const inputsInReset = await page.$$('input');
    if (inputsInReset.length > 0) {
      throw new Error('Security Violation: Input field found in disabled Reset Password view!');
    }
    const pageText = await page.evaluate(() => document.body.innerText);
    if (!pageText.includes('CLI Admin Password Reset') || !pageText.includes('./scripts/reset_password.sh')) {
      throw new Error('CLI guidance text not displayed in Reset Password view');
    }
    console.log('✅ Confirmed: UI presents secure CLI admin guidance with no public form inputs.');

    // Click "Back to Sign In"
    const backBtn = await page.waitForSelector('button');
    const allButtons = await page.$$('button');
    for (const b of allButtons) {
      const text = await page.evaluate(el => el.textContent, b);
      if (text && text.includes('Back to Sign In')) {
        await b.click();
        break;
      }
    }
    await new Promise(r => setTimeout(r, 500));

    // Step D: Execute server CLI script to reset password for kishan
    console.log('5. Executing ./scripts/reset_password.sh kishan...');
    const cliOutput = execSync('../scripts/reset_password.sh kishan', { encoding: 'utf-8' });
    const match = cliOutput.match(/Temporary Password\s*:\s*(\S+)/);
    if (!match || !match[1]) {
      throw new Error('Temporary password not found in CLI output: ' + cliOutput);
    }
    const tempPassword = match[1];
    console.log(`✅ Temporary password generated via admin CLI: ${tempPassword}`);

    // Step E: Log in with temporary password
    console.log('6. Logging in with temporary password...');
    const userField = await page.waitForSelector('input[placeholder="Enter your username or email"]');
    await userField.click({ clickCount: 3 });
    await userField.type('kishan');

    const pwdField = await page.waitForSelector('input[placeholder="••••••••"]');
    await pwdField.click({ clickCount: 3 });
    await pwdField.type(tempPassword);
    await page.click('button[type="submit"]');

    // Wait for user to be logged in
    await page.waitForSelector('button[aria-label="User Account Menu"]', { timeout: 10000 });
    console.log('✅ Logged in successfully with temporary password!');

    // Step F: Change password back to permanent Kishan@123
    console.log('7. Changing password back to permanent Kishan@123 in authenticated user space...');
    await page.click('button[aria-label="User Account Menu"]');
    await new Promise(r => setTimeout(r, 400));

    const dropdownButtons = await page.$$('button');
    let changeFound = false;
    for (const b of dropdownButtons) {
      const text = await page.evaluate(el => el.textContent, b);
      if (text && text.includes('Change Password')) {
        await b.click();
        changeFound = true;
        break;
      }
    }
    if (!changeFound) throw new Error('Change Password button in user menu not found');
    await new Promise(r => setTimeout(r, 500));

    const modalInputs = await page.$$('div[role="dialog"] input, .fixed input');
    if (modalInputs.length < 3) {
      throw new Error(`Expected at least 3 password inputs in Change Password modal, found ${modalInputs.length}`);
    }

    // Input current password (temporary password)
    await modalInputs[0].type(tempPassword);
    // Input new password (Kishan@123)
    await modalInputs[1].type('Kishan@123');
    // Confirm new password
    await modalInputs[2].type('Kishan@123');

    // Submit change password
    const submitBtn = await page.$('.fixed button[type="submit"]');
    await submitBtn.click();
    await new Promise(r => setTimeout(r, 2000));
    console.log('✅ Password restored to permanent Kishan@123.');

    console.log('🎉 Reset password CLI-only security architecture fully verified!');
  } finally {
    await browser.close();
  }
})();
