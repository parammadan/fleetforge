import { defineConfig, devices } from "@playwright/test";

// End-to-end tests against the replay control room.
//
// A separate config from the fixture suite because the backend is started
// differently — `--replay` rather than `--fixtures` — and because the two
// modes make different promises. Running them in one project would mean one
// `webServer`, and the mode assertions are the point of both suites.
export default defineConfig({
  testDir: "./e2e",
  testMatch: /(replay|a11y)\.spec\.ts/,
  fullyParallel: false,
  workers: 1,
  reporter: process.env.CI ? "list" : [["list"], ["html", { open: "never" }]],
  timeout: 45_000,

  use: {
    baseURL: "http://127.0.0.1:5173",
    trace: "retain-on-failure",
    screenshot: "only-on-failure",
  },

  // Three engines, because the interface is a leadership artefact that will be
  // opened on whatever the reader happens to use. Firefox and WebKit are where
  // CSS that only Chromium tolerates shows up.
  projects: [
    { name: "chromium", use: { ...devices["Desktop Chrome"] } },
    { name: "firefox", use: { ...devices["Desktop Firefox"] } },
    { name: "webkit", use: { ...devices["Desktop Safari"] } },
  ],

  webServer: [
    {
      command:
        "cd .. && ./target/debug/fleetforge --replay evidence/eks-recovery --bind 127.0.0.1:8080",
      url: "http://127.0.0.1:8080/readyz",
      reuseExistingServer: false,
      timeout: 60_000,
    },
    {
      command: "npm run dev",
      url: "http://127.0.0.1:5173",
      reuseExistingServer: false,
      timeout: 60_000,
    },
  ],
});
