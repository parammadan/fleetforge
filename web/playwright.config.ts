import { defineConfig, devices } from "@playwright/test";

// End-to-end tests against a real browser.
//
// Runs against FIXTURE mode, not a live cluster: the assertions are about what
// the interface renders, and those must not change because a pod restarted
// mid-test. Fixture data is recorded real cluster state (ADR-0020), so the
// shapes are honest even though the values are frozen.
export default defineConfig({
  testDir: "./e2e",
  // The replay suite needs a backend started with `--replay` and lives in
  // playwright.replay.config.ts. Running it here would point it at fixture data
  // and fail for the right reason in the wrong place.
  testIgnore: /(replay|a11y)\.spec\.ts/,
  fullyParallel: false,
  workers: 1,
  reporter: process.env.CI ? "list" : [["list"], ["html", { open: "never" }]],
  timeout: 30_000,

  use: {
    baseURL: "http://127.0.0.1:5173",
    trace: "retain-on-failure",
    screenshot: "only-on-failure",
  },

  projects: [
    { name: "chromium", use: { ...devices["Desktop Chrome"] } },
  ],

  webServer: [
    {
      // The backend, in fixture mode. Nothing in these tests touches a cluster.
      command:
        "cd .. && ./target/debug/fleetforge --fixtures fixtures/captured --bind 127.0.0.1:8080",
      url: "http://127.0.0.1:8080/readyz",
      reuseExistingServer: false,
      timeout: 30_000,
    },
    {
      command: "npm run dev",
      url: "http://127.0.0.1:5173",
      reuseExistingServer: false,
      timeout: 60_000,
    },
  ],
});
