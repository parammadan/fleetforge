import { defineConfig, devices } from "@playwright/test";

// The Phase C bundle: a prevented failure rather than an incident.
//
// Its own config because the backend serves a different directory, and because
// the two captures make close to opposite claims — the assertions that matter
// are the ones checking each says only its own.
export default defineConfig({
  testDir: "./e2e",
  testMatch: "prevented.spec.ts",
  fullyParallel: false,
  workers: 1,
  reporter: process.env.CI ? "list" : [["list"], ["html", { open: "never" }]],
  timeout: 45_000,
  use: { baseURL: "http://127.0.0.1:8080", trace: "retain-on-failure", screenshot: "only-on-failure" },
  projects: [{ name: "chromium", use: { ...devices["Desktop Chrome"] } }],
  webServer: [
    {
      command:
        "cd .. && ./target/release/fleetforge --replay evidence/eks-live --ui web/dist --bind 127.0.0.1:8080",
      url: "http://127.0.0.1:8080/readyz",
      reuseExistingServer: false,
      timeout: 60_000,
    },
  ],
});
