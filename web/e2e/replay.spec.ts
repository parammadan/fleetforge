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
    has: page.locator(".tile-label", { hasText: "Customer availability" }),
  });
  // The spec's exact words. "UNKNOWN" alone reads as "unknown to this tool".
  await expect(tile.locator(".tile-value")).toHaveText("UNKNOWN DURING INCIDENT");
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
  const strip = page.locator(".position-strip");
  const settled = async () => {
    await expect(strip).not.toHaveAttribute("data-stale", "true");
    return strip.innerText();
  };

  await page.locator(chapter("First manual uncordon")).click();
  await expect(strip).toContainText("1 cordoned");
  const first = await settled();

  await page.locator(chapter("Capture begins")).click();
  await expect(strip).toContainText("Nothing observed yet");

  await page.locator(chapter("First manual uncordon")).click();
  await expect(strip).toContainText("1 cordoned");
  expect(await settled()).toBe(first);
});

test("state that has not caught up says so rather than showing the wrong moment", async ({
  page,
}) => {
  // Hold the state response so the gap between "the controls moved" and "the
  // fleet moved" is observable at all. Over loopback it is sub-millisecond.
  await page.route("**/api/v1/replay/state*", async (route) => {
    await new Promise((r) => setTimeout(r, 600));
    await route.continue();
  });
  await page.locator(chapter("Second manual uncordon")).click();
  await expect(page.locator(".position-strip")).toHaveAttribute("data-stale", "true");
  await expect(page.locator(".position-strip")).toContainText("Loading state for this position");
  // And it resolves rather than sticking.
  await expect(page.locator(".position-strip")).toContainText("0 cordoned", { timeout: 10_000 });
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
  // Scoped: claims elsewhere on the page link to the same artifact by name.
  await page.locator("#evidence").getByRole("button", { name: "00-CONCLUSIONS.md" }).click();
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

test("the investigation chain separates observations from the arrows joining them", async ({
  page,
}) => {
  const chain = page.locator("#chain");
  await chain.scrollIntoViewIfNeeded();

  // Five facts, four arrows. Every fact is evidence; every arrow is human RCA.
  await expect(chain.locator(".chain-box")).toHaveCount(5);
  await expect(chain.locator(".chain-arrow")).toHaveCount(4);
  await expect(chain.locator(".chain-arrow .arrow-basis")).toHaveText([
    "HUMAN RCA",
    "HUMAN RCA",
    "HUMAN RCA",
    "HUMAN RCA",
  ]);
  for (const tag of await chain.locator(".chain-box .basis").allTextContents()) {
    expect(["OBSERVED", "DERIVED"]).toContain(tag);
  }

  await expect(chain).toContainText("disruptionsAllowed = 0");
  await expect(chain).toContainText("did not produce the chain");
});

test("a chain fact opens the artifact it was read from", async ({ page }) => {
  await page.locator("#chain").scrollIntoViewIfNeeded();
  await page.locator(".chain-box", { hasText: "Disruption budget exhausted" }).click();
  await page.getByRole("button", { name: "04-pdb-before.json" }).first().click();

  const drawer = page.locator("#evidence-drawer");
  await expect(drawer).toBeVisible();
  await expect(drawer.locator("h4")).toHaveText("04-pdb-before.json");
  await expect(drawer.locator(".drawer-hash")).toContainText("sha256");
  await expect(drawer.locator(".artifact")).toContainText("disruptionsAllowed", {
    timeout: 10_000,
  });
});

test("a claim in the summary opens its own evidence", async ({ page }) => {
  await page
    .locator(".claim", { hasText: "detected an already-existing" })
    .getByRole("button", { name: "03-preflight-before.json" })
    .click();
  await expect(page.locator("#evidence-drawer h4")).toHaveText("03-preflight-before.json");
});

test("the provenance strip identifies the recording and the current position", async ({
  page,
}) => {
  const strip = page.locator(".provenance");
  await expect(strip).toContainText("2026-09-13 14:15:47–15:19:00 UTC");
  await expect(strip).toContainText("v1.36.4-eks-4cc7921");
  await expect(strip).toContainText("1.62.1 → 1.64.0");
  await expect(strip).toContainText("destroyed");

  // Position and snapshot identity track the timeline.
  await expect(strip).toContainText("step 1/509");
  await page.locator(".chapter-button", { hasText: "First manual uncordon" }).click();
  await expect(strip).toContainText("14:35:11");
  await expect(strip).not.toContainText("none yet");
});

test("the opening frame says nothing was observed rather than showing zeroes", async ({
  page,
}) => {
  // An empty cluster and a cluster not yet observed must never look the same.
  await expect(page.locator(".position-strip")).toContainText("Nothing observed yet");
  await expect(page.locator(".position-strip")).not.toContainText("0 nodes");
});

test("the five required disclosures are present and numbered", async ({ page }) => {
  const list = page.locator("#limitations .disclosures");
  await expect(list.locator("li")).toHaveCount(5);
  await expect(list).toContainText("Recording began after Brupop did");
  await expect(list).toContainText("No pre-update prediction exists");
  await expect(list).toContainText("Original availability cannot be determined");
  await expect(list).toContainText("causal chain includes human analysis");
  await expect(list).toContainText("networking root cause is unverified");
});

test("executive terms carry definitions, reachable without a mouse", async ({ page }) => {
  const pdb = page.locator("abbr.term", { hasText: "PodDisruptionBudget" }).first();
  await expect(pdb).toHaveAttribute("title", /how many of its copies may be taken offline/);
  await pdb.focus();
  await expect(pdb).toBeFocused();
});

test("the timeline is operable from the keyboard alone", async ({ page }) => {
  const step = page.locator("#timeline header .count");
  const forward = page.getByRole("button", { name: "Step ▶" });
  await forward.focus();
  await expect(forward).toBeFocused();
  await page.keyboard.press("Enter");
  await expect(step).toContainText("step 2 of");
  await page.keyboard.press("Enter");
  await expect(step).toContainText("step 3 of");

  // The scrubber is a native range input, so arrow keys move it.
  const scrub = page.locator(".scrub input");
  await scrub.focus();
  await page.keyboard.press("ArrowRight");
  await expect(step).toContainText("step 4 of");
});

test("focus is visible on every interactive control that matters", async ({ page }) => {
  for (const target of [
    page.getByRole("button", { name: "Play", exact: true }),
    page.locator(".chapter-button").first(),
    page.locator(".scrub input"),
  ]) {
    await target.focus();
    const outline = await target.evaluate(
      (el) => getComputedStyle(el, ":focus-visible").outlineStyle,
    );
    expect(outline).not.toBe("none");
  }
});

test("a failing state request reports the gap instead of showing a stale position", async ({
  page,
}) => {
  await page.route("**/api/v1/replay/state*", (route) => route.abort());
  await page.reload();
  await expect(page.getByText("State unavailable")).toBeVisible();
  await expect(page.getByText(/last position that loaded/)).toBeVisible();
});

test("a backend serving a mode other than replay is refused, not rendered", async ({ page }) => {
  // The browser's own guard. If backend and frontend ever disagree about the
  // mode, the screen must fail loudly rather than pick one.
  await page.route("**/api/v1/replay/context", async (route) => {
    const response = await route.fetch();
    const body = await response.json();
    body.mode = "live";
    body.mode_label = "LIVE";
    await route.fulfill({ json: body });
  });
  await page.goto("/");
  await expect(page.getByText(/refusing to render/)).toBeVisible();
  await expect(page.locator(".mode-live")).toHaveCount(0);
});

test("the evidence explorer leads with a description and keeps raw data secondary", async ({
  page,
}) => {
  await page.locator("#evidence").scrollIntoViewIfNeeded();
  await page.getByRole("button", { name: "09-brupop-controller.log" }).click();
  const drawer = page.locator("#evidence-drawer");
  await expect(drawer.locator(".drawer-explanation")).not.toBeEmpty();
  await expect(drawer.locator("details summary")).toContainText("Raw");
});

test("node cards state where each node is in its update, with evidence", async ({ page }) => {
  await page.locator(".chapter-button", { hasText: "Nodes are already cordoned" }).click();
  const stuck = page.locator(".node-card", { hasText: "cordoned" }).first();
  // A cordoned node with no Brupop shadow observed must say that, not shrug.
  await expect(stuck.locator(".transition")).toContainText(
    /cordoned — no Brupop state observed/i,
  );
  await expect(stuck.locator(".node-evidence")).toContainText("before recovery");
});
