// Shared configuration for the Puppeteer scripts.
//
// E2E_BASE_URL  frontend to test (default: the live app on http://localhost:8081)
// E2E_ISOLATED  set to 1 by scripts/run_e2e.sh, which starts a throwaway backend
//               on the gnucash_e2e_test database and a preview build of the frontend

export const BASE_URL = (process.env.E2E_BASE_URL || 'http://localhost:8081').replace(/\/$/, '');

export const IS_ISOLATED = process.env.E2E_ISOLATED === '1';

/**
 * Call at the top of any script that creates, changes or deletes data.
 * Refuses to run unless the script was started by scripts/run_e2e.sh,
 * so the live database is never modified by a test.
 */
export function requireIsolatedEnv(scriptName) {
  if (!IS_ISOLATED) {
    console.error(
      `❌ ${scriptName} changes data, so it only runs in the isolated test environment.\n` +
        `   Run it with: npm run test:e2e -- ${scriptName}`
    );
    process.exit(1);
  }
}
