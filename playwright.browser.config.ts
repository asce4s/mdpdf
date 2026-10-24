import {defineConfig,chromium} from '@playwright/test';
export default defineConfig({
  testDir:'./e2e',testMatch:'browser-only.spec.ts',workers:1,timeout:60000,
  expect:{timeout:15000},reporter:process.env.CI?'github':'list',
  use:{baseURL:process.env.MDPDF_BROWSER_URL||'http://127.0.0.1:4173/mdpdf/',viewport:{width:1512,height:982},trace:'retain-on-failure',screenshot:'only-on-failure',launchOptions:{executablePath:process.env.CHROME_PATH||chromium.executablePath(),chromiumSandbox:true}},
  webServer:process.env.MDPDF_E2E_EXTERNAL?undefined:[{command:'node scripts/serve_browser.mjs --base /mdpdf/',url:'http://127.0.0.1:4173/mdpdf/',reuseExistingServer:!process.env.CI,timeout:10000},{command:'node scripts/serve_browser.mjs --port 4174',url:'http://127.0.0.1:4174/',reuseExistingServer:!process.env.CI,timeout:10000}],
});
