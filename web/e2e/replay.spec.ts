import { expect, test } from "@playwright/test";

// The replay control room, in a real browser, against the real evidence bundle.
//
// These assertions are about what a person in the room would be able to
// conclude from the screen. The most important ones are negative: they fail if
// the interface ever presents replay data as live, presents a failed networking
// check as availability, or shows a claim without saying where it came from.

const chapter = (name: string) => `.chapter-button:has-text("${name}")`;

test.beforeEach(async ({ page }) => {
  await page.goto("/");
  await expect(page.locator(".banner-replay")).toBeVisible();
});

test("the mode badge and banner say REPLAY and cannot be dismissed", async ({ page }) => {
  await expect(page.locator(".mode-badge")).toHaveText("REPLAY");
  await expect(page.locator(".banner-replay strong")).toHaveText(
    "REPLAY — CAPTURED FROM REAL EKS/BOTTLEROCKET EXECUTION",
  );
  // No close control anywhere in the banner.
  await expect(page.locator(".banner-replay button")).toHaveCount(0);

  // And it survives scrolling to the bottom of the page, because a screenshot
  // of the fleet panel has to carry the label too.
  await page.locator("#limitations").scrollIntoViewIfNeeded();
  await expect(page.locator(".banner-replay")).toBeInViewport();
});

test("nothing on the page claims to be live", async ({ page }) => {
  const body = (await page.locator("body").innerText()).toLowerCase();
  expect(body).not.toContain("live stream connected");
  expect(body).not.toContain("mode: live");
  await expect(page.locator(".mode-live")).toHaveCount(0);
});

test("availability is UNKNOWN and the 30.2% figure is not presented as uptime", async ({
  page,
}) => {
  // Scoped by label, not by text: the traffic tile's own caption contains the
  // phrase "availability during the incident" — precisely because it is
  // disclaiming it.
  const tile = page.locator(".tile", {
    has: page.locator(".tile-label", { hasText: "Availability during the incident" }),
  });
  await expect(tile.locator(".tile-value")).toHaveText("UNKNOWN");
  await expect(tile).not.toContainText("30.2");

  const traffic = page.locator(".tile", {
    has: page.locator(".tile-label", { hasText: "Post-recovery networking check" }),
  });
  await expect(traffic.locator(".tile-value")).toContainText("FAILED");
  await expect(traffic).toContainText("must never be presented as uptime");
});

test("the summary states that FleetForge did not predict the deadlock", async ({ page }) => {
  const callout = page.locator(".callout");
  await expect(callout).toContainText("FleetForge did not predict this");
  await expect(callout).toContainText("14:08:15");
  await expect(callout).toContainText("14:15:47");
});

test("the causal chain is labelled HUMAN RCA, not presented as FleetForge's inference", async ({
  page,
}) => {
  const claim = page.locator(".claim", { hasText: "prevented the third web replica" });
  await expect(claim.locator(".basis")).toHaveText("HUMAN RCA");
  await expect(claim).toHaveClass(/claim-soft/);
});

test("every claim carries a basis tag", async ({ page }) => {
  const claims = page.locator("#summary .claim");
  const count = await claims.count();
  expect(count).toBeGreaterThan(5);
  await expect(page.locator("#summary .claim .basis")).toHaveCount(count);
});

test("chapters jump the timeline and the fleet follows", async ({ page }) => {
  await page.locator(chapter("FleetForge reports BLOCKED")).click();
  await expect(page.locator(".chapter-note")).toContainText("FleetForge reports BLOCKED");
  await expect(page.locator(".position-strip")).toContainText("2 cordoned");
  await expect(page.locator(".position-strip")).toContainText("1 Pending");

  await page.locator(chapter("Second manual uncordon")).click();
  await expect(page.locator(".position-strip")).toContainText("0 cordoned");
});

test("play advances the timeline and pause stops it", async ({ page }) => {
  const step = page.locator("#timeline header .count");
  await expect(step).toContainText("step 1 of");

  await page.getByRole("button", { name: "Play", exact: true }).click();
  await expect(step).not.toContainText("step 1 of", { timeout: 8_000 });

  await page.getByRole("button", { name: "Pause", exact: true }).click();
  const frozen = await step.textContent();
  await page.waitForTimeout(2_000);
  expect(await step.textContent()).toBe(frozen);
});

test("stepping and restarting are exact and reversible", async ({ page }) => {
  const step = page.locator("#timeline header .count");
  await page.getByRole("button", { name: "Step ▶" }).click();
  await expect(step).toContainText("step 2 of");
  await page.getByRole("button", { name: "◀ Step" }).click();
  await expect(step).toContainText("step 1 of");

  await page.locator(chapter("Capture ends")).click();
  await expect(step).toContainText("step 509 of 509");
  await page.getByRole("button", { name: "Restart" }).click();
  await expect(step).toContainText("step 1 of");
});

test("the same position always renders the same state", async ({ page }) => {
  await page.locator(chapter("First manual uncordon")).click();
  const first = await page.locator(".position-strip").innerText();

  await page.locator(chapter("Capture begins")).click();
  await page.locator(chapter("First manual uncordon")).click();
  expect(await page.locator(".position-strip").innerText()).toBe(first);
});

test("the PDB finding shows arithmetic, not just a verdict", async ({ page }) => {
  const finding = page.locator("#finding");
  await expect(finding).toContainText("disruptionsAllowed = currentHealthy - desiredHealthy");
  await expect(finding).toContainText(".status.disruptionsAllowed");
  await expect(finding).toContainText("What this finding does not tell you");
});

test("under-predictions are shown in their own class and counted up front", async ({ page }) => {
  const panel = page.locator("#predictions");
  await expect(panel.locator(".verdict-blocked")).toContainText("UNDER-PREDICTED");
  await expect(panel.locator("tr.pred-under_predicted")).toHaveCount(2);
  // Never relabelled as conservative.
  await expect(panel.locator("tr.pred-under_predicted .pill")).toHaveText([
    "under-predicted",
    "under-predicted",
  ]);
});

test("an artifact opens with its hash and is served through the API", async ({ page }) => {
  await page.locator("#evidence").scrollIntoViewIfNeeded();
  await page.getByRole("button", { name: "00-CONCLUSIONS.md" }).click();
  const drawer = page.locator("#evidence .drawer");
  await expect(drawer.locator(".artifact")).toContainText("Brupop", { timeout: 10_000 });
});

test("the limitations panel names the version-field bug rather than hiding it", async ({
  page,
}) => {
  const panel = page.locator("#limitations");
  await expect(panel).toContainText("version-field-bug");
  await expect(panel).toContainText("No mutation was issued by FleetForge");
  await expect(panel).toContainText("does not prove the recording was complete");
});

test("the recorded 2.0.0 version is shown as recorded and flagged as suspect", async ({
  page,
}) => {
  await page.locator(chapter("Nodes are already cordoned")).click();
  const suspect = page.locator(".value-suspect").first();
  await expect(suspect).toContainText("2.0.0");
  await expect(suspect).toHaveAttribute("title", /field-mapping bug/);
});

test("renders on a phone-width viewport without horizontal overflow", async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await page.goto("/");
  await expect(page.locator(".banner-replay")).toBeVisible();
  // The banner wraps rather than truncating: "REPLAY — CAPTURED FROM REAL
  // EKS/BOT…" has lost the half that says where the data came from.
  await expect(page.locator(".banner-replay strong")).toHaveText(
    "REPLAY — CAPTURED FROM REAL EKS/BOTTLEROCKET EXECUTION",
  );
  const overflow = await page.evaluate(
    () => document.documentElement.scrollWidth - document.documentElement.clientWidth,
  );
  expect(overflow).toBeLessThanOrEqual(1);
});

test("a backend that is not replaying produces an explanation, not an empty screen", async ({
  page,
}) => {
  // Simulate the backend disappearing between page loads.
  await page.route("**/api/v1/environment", (route) => route.abort());
  await page.goto("/");
  await expect(page.getByText("FleetForge is not reachable")).toBeVisible();
  await expect(page.getByText(/no honest thing to show/i)).toBeVisible();
});
