import { defineConfig, devices } from "@playwright/test";

export default defineConfig({
  testDir: "./tests",
  /* Run tests in files in parallel */
  fullyParallel: true,
  /* Fail the build on CI if test.only is left accidentally */
  forbidOnly: !!process.env.CI,
  /* Retry on CI */
  retries: process.env.CI ? 1 : 0,
  /* Fewer workers on CI to stay within resource limits */
  workers: process.env.CI ? 2 : undefined,
  /* Reporter */
  reporter: process.env.CI ? [["github"], ["html", { open: "never" }]] : "list",
  /* Shared settings for all tests */
  use: {
    /* Base URL for all page.goto("/") calls */
    baseURL: "http://localhost:4173",
    /* Collect trace on first retry in CI */
    trace: process.env.CI ? "on-first-retry" : "off",
  },

  projects: [
    {
      name: "chromium",
      use: { ...devices["Desktop Chrome"] },
    },
  ],

  /* Build the app and start vite preview before running tests */
  webServer: {
    command: "npm run build && npx vite preview --port 4173",
    port: 4173,
    reuseExistingServer: !process.env.CI,
    timeout: 60_000,
  },
});
