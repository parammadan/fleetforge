import { defineConfig, devices } from "@playwright/test";

// Records the walkthrough video. Separate from the test configs because this
// produces a deliverable rather than a verdict, and because recording video for
// every test would make the suites slow and enormous.
export default defineConfig({
  testDir: "./e2e",
  testMatch: "walkthrough.spec.ts",
  fullyParallel: false,
  workers: 1,
  reporter: [["list"]],
  timeout: 240_000,
  outputDir: "./walkthrough-output",

  use: {
    baseURL: "http://127.0.0.1:8080",
    // Deterministic playback speed: no animation shortcuts, nothing sped up.
    launchOptions: { args: ["--force-prefers-reduced-motion"] },
  },

  projects: [{ name: "chromium", use: { ...devices["Desktop Chrome"] } }],

  webServer: [
    {
      // The production binary serving the production interface — the same
      // thing `make demo` runs, not a development server.
      command:
        "cd .. && ./target/debug/fleetforge --replay evidence/eks-recovery --ui web/dist --bind 127.0.0.1:8080",
      url: "http://127.0.0.1:8080/readyz",
      reuseExistingServer: false,
      timeout: 60_000,
    },
  ],
});
