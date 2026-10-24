import { defineConfig, chromium } from '@playwright/test';
export default defineConfig({
  testDir:'./e2e',testMatch:'**/*.spec.ts',fullyParallel:false,workers:1,timeout:60000,
  expect:{timeout:15000},
  reporter:process.env.CI ? 'github':'list',
  use:{baseURL:process.env.MDPDF_BASE_URL || 'http://127.0.0.1:3000',viewport:{width:1512,height:982},trace:'retain-on-failure',screenshot:'only-on-failure',launchOptions:{executablePath:process.env.CHROME_PATH || chromium.executablePath(),chromiumSandbox:true}},
  webServer:process.env.MDPDF_E2E_EXTERNAL ? undefined : {command:'bash scripts/e2e-server.sh',url:'http://127.0.0.1:3000/healthz',reuseExistingServer:!process.env.CI,timeout:120000},
});
