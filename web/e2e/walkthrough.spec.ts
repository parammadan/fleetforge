import { expect, test } from "@playwright/test";

// A recorded walkthrough of the replay, for people who cannot run it.
//
// Playwright records the video; nothing is narrated, staged or re-timed beyond
// pausing long enough to read. Every frame is the real interface driven through
// the real backend against the real bundle — the same thing the tests drive.
//
// The REPLAY banner is asserted at every stop, because a recording circulates
// without its context and a clip of this that does not say REPLAY is precisely
// the artefact this project must not produce.

test.use({
  video: { mode: "on", size: { width: 1440, height: 900 } },
  viewport: { width: 1440, height: 900 },
});

/** Long enough for a viewer to read a panel, short enough to keep moving. */
const READ = 2600;
const BEAT = 1100;

test("walkthrough", async ({ page }) => {
  test.setTimeout(180_000);

  const chapter = (name: string) => page.locator(".chapter-button", { hasText: name });
  const settled = () =>
    expect(page.locator('.position-strip:not([data-stale="true"])')).toBeVisible();
  const show = async (selector: string) => {
    await page.locator(selector).evaluate((el) => {
      window.scrollTo({
        top: el.getBoundingClientRect().top + window.scrollY - 140,
        behavior: "smooth",
      });
    });
    await page.waitForTimeout(900);
  };
  const stillReplay = async () => {
    await expect(page.locator(".banner-replay strong")).toHaveText(
      "REPLAY — CAPTURED FROM REAL EKS/BOTTLEROCKET EXECUTION",
    );
    await expect(page.locator(".mode-badge")).toHaveText("REPLAY");
  };

  // 1 — Executive opening.
  await page.goto("/");
  await expect(page.locator(".banner-replay")).toBeVisible();
  await stillReplay();
  await expect(page.locator(".tile", { hasText: "Customer availability" })).toContainText(
    "UNKNOWN DURING INCIDENT",
  );
  await page.waitForTimeout(READ + 1200);

  await show("#summary");
  await page.waitForTimeout(READ);
  await stillReplay();

  // 2 — The blocker.
  await chapter("FleetForge reports BLOCKED").click();
  await settled();
  await show("#timeline");
  await expect(page.locator(".chapter-note")).toContainText("FleetForge reports BLOCKED");
  await expect(page.locator(".position-strip")).toContainText("2 cordoned");
  await page.waitForTimeout(READ);
  await stillReplay();

  // 3 — The causal investigation, with a fact opened.
  await show("#chain");
  await page.waitForTimeout(BEAT);
  await page.locator(".chain-box", { hasText: "Disruption budget exhausted" }).click();
  await page.waitForTimeout(READ);
  await expect(page.locator("#chain .chain-arrow .arrow-basis").first()).toHaveText("HUMAN RCA");
  await stillReplay();

  // 4 — The finding's arithmetic.
  await show("#finding");
  await expect(page.locator("#finding")).toContainText(
    "disruptionsAllowed = currentHealthy - desiredHealthy",
  );
  await page.waitForTimeout(READ);

  // 5 — First recovery.
  await show("#timeline");
  await chapter("First manual uncordon").click();
  await settled();
  await expect(page.locator(".position-strip")).toContainText("1 cordoned");
  await page.waitForTimeout(BEAT);
  await show("#fleet");
  await page.waitForTimeout(READ);
  await stillReplay();

  // 6 — The second stale cordon: updated to 1.64.0, still held out of service.
  await show("#timeline");
  await chapter("All nodes on 1.64.0").click();
  await settled();
  await expect(page.locator(".position-strip")).toContainText("1 cordoned");
  await show("#fleet");
  await page.waitForTimeout(READ);

  // 7 — Final recovery.
  await show("#timeline");
  await chapter("Second manual uncordon").click();
  await settled();
  await expect(page.locator(".position-strip")).toContainText("0 cordoned");
  await show("#fleet");
  await page.waitForTimeout(READ);
  await stillReplay();

  // 8 — Evidence drawer, opened from the chain.
  await show("#chain");
  await page.getByRole("button", { name: "04-pdb-before.json" }).first().click();
  await expect(page.locator("#evidence-drawer .artifact")).toBeVisible({ timeout: 10_000 });
  await show("#evidence-drawer");
  await page.waitForTimeout(READ + 900);
  await stillReplay();

  // 9 — Predicted versus actual.
  await show("#predictions");
  await expect(page.locator("#predictions .verdict-blocked")).toContainText("UNDER-PREDICTED");
  await page.waitForTimeout(READ);

  // 10 — Limitations.
  await show("#limitations");
  await expect(page.locator("#limitations .disclosures li")).toHaveCount(5);
  await page.waitForTimeout(READ + 900);
  await stillReplay();
});
