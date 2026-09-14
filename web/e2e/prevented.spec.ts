import { expect, test } from "@playwright/test";

// The Phase C capture, in a browser.
//
// The incident and this run make close to opposite claims, and the failure mode
// worth guarding against is one leaking into the other: an incident that claims
// prevention, or a prevented run that borrows the incident's excuses.

async function openTab(page: import("@playwright/test").Page, label: string) {
  await page.locator(".tab", { hasText: label }).click();
  await expect(page.locator(".tab-active .tab-label")).toHaveText(label);
}

test.beforeEach(async ({ page }) => {
  await page.goto("/");
  await expect(page.locator(".banner-replay")).toBeVisible();
});

test("it is still unmistakably REPLAY", async ({ page }) => {
  await expect(page.locator(".mode-badge")).toHaveText("REPLAY");
  await expect(page.locator(".banner-replay strong")).toHaveText(
    "REPLAY — CAPTURED FROM REAL EKS/BOTTLEROCKET EXECUTION",
  );
  await expect(page.locator(".mode-live")).toHaveCount(0);
});

test("the landing view tells the prevention story, not the incident's", async ({ page }) => {
  await expect(page.locator(".overview-headline")).toContainText(/caught before it happened/i);
  await expect(page.locator(".brand")).toContainText("prevented-failure replay");

  const overview = await page.locator(".overview").innerText();
  // The incident's defining admission must not appear here.
  expect(overview).not.toContain("did not predict this incident");
  expect(overview).toContain("prevented rather than explained");
});

test("the prevention claim shows the ordering that earns it", async ({ page }) => {
  const d = page.locator(".overview-disclaimer");
  // The claim is stronger than the incident's, so the timestamps that justify
  // it are on the same screen rather than behind a tab.
  await expect(d).toContainText("22:40:19");
  await expect(d).toContainText("22:44:02");
  await expect(d).toContainText(/worth checking/i);
});

test("the landing view carries the prediction failure, not just the win", async ({ page }) => {
  await expect(page.locator(".outcome")).toContainText("prediction was wrong");
  await expect(page.locator(".outcome")).toContainText("not a fair one");
});

test("chapters are in causal order: blocked, safe, then the executor", async ({ page }) => {
  await page.getByRole("button", { name: /Explore the experiment/ }).click();
  const titles = await page.locator(".chapter-button").allTextContents();
  const idx = (needle: string) => titles.findIndex((t) => t.includes(needle));
  expect(idx("BLOCKED")).toBeGreaterThanOrEqual(0);
  // Brupop arriving after both preflights is the whole claim. If this ordering
  // ever inverts, the interface is asserting something the log does not show.
  expect(idx("BLOCKED")).toBeLessThan(idx("SAFE"));
  expect(idx("SAFE")).toBeLessThan(idx("Brupop begins"));
  expect(idx("Brupop begins")).toBeLessThan(idx("First node reboots"));
});

test("every chain link is evidence and none is a counterfactual", async ({ page }) => {
  await page.getByRole("button", { name: /Explore the experiment/ }).click();
  await openTab(page, "Investigation");
  const tags = await page.locator("#chain .chain-box .basis").allTextContents();
  expect(tags.length).toBe(6);
  for (const t of tags) expect(["OBSERVED", "DERIVED"]).toContain(t);
  await expect(page.locator("#chain")).toContainText("counterfactual");
});

test("the caveats are this run's, not the incident's", async ({ page }) => {
  await page.getByRole("button", { name: /Explore the experiment/ }).click();
  await openTab(page, "Limits");
  const limits = await page.locator("#limitations").innerText();
  expect(limits).toContain("deliberate-condition");
  expect(limits).toContain("unfair-prediction-window");
  // False here: one run_started, and the capture ended before teardown.
  expect(limits).not.toContain("restarts");
  expect(limits).not.toContain("teardown-tail");
});

test("the passing traffic sample is not presented as uptime", async ({ page }) => {
  await page.getByRole("button", { name: /Explore the experiment/ }).click();
  await openTab(page, "Limits");
  const traffic = page.locator("#traffic");
  await expect(traffic).toContainText(/NOT an availability or uptime measurement/i);
  await expect(traffic).toContainText(/one sampler/i);
});
