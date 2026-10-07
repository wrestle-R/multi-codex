import { defineConfig, devices } from "@playwright/test"
export default defineConfig({
  testDir: "./tests", fullyParallel: true, forbidOnly: Boolean(process.env.CI), retries: process.env.CI ? 1 : 0,
  reporter: process.env.CI ? "github" : "list",
  use: { baseURL: process.env.BASE_URL || "http://127.0.0.1:3100", trace: "retain-on-failure" },
  projects: [
    { name: "chromium", use: { ...devices["Desktop Chrome"] } },
    { name: "webkit", use: { ...devices["Desktop Safari"] } },
    { name: "mobile", use: { ...devices["Pixel 7"] } },
  ],
  webServer: process.env.BASE_URL ? undefined : { command: "npm run start -- --hostname 127.0.0.1 --port 3100", url: "http://127.0.0.1:3100", reuseExistingServer: !process.env.CI, timeout: 120_000 },
})
