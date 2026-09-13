import AxeBuilder from "@axe-core/playwright";
import { expect, test } from "@playwright/test";

// Automated accessibility checks over the replay control room.
//
// axe is a floor, not a ceiling — it catches contrast, names, landmarks and
// structure, and it cannot tell you whether the screen makes sense. The
// keyboard and focus assertions below exist because those are the parts a
// scanner marks "manual" and a leadership audience actually depends on.
//
// Nothing here is allowed to pass by disabling a rule. If a violation is real,
// the interface changes.

/** WCAG 2.1 A and AA, which is the bar an internal tool should clear. */
const TAGS = ["wcag2a", "wcag2aa", "wcag21a", "wcag21aa"];

async function scan(page: import("@playwright/test").Page, selector?: string) {
  let builder = new AxeBuilder({ page }).withTags(TAGS);
  if (selector) builder = builder.include(selector);
  return builder.analyze();
}

function describeViolations(results: Awaited<ReturnType<typeof scan>>): string {
  return results.violations
    .map(
      (v) =>
        `${v.id} (${v.impact}): ${v.help}\n` +
        v.nodes.map((n) => `    ${n.target.join(" ")}\n      ${n.failureSummary}`).join("\n"),
    )
    .join("\n\n");
}

test.beforeEach(async ({ page }) => {
  await page.goto("/");
  await expect(page.locator(".banner-replay")).toBeVisible();
});

test("the loaded control room has no WCAG A or AA violations", async ({ page }) => {
  const results = await scan(page);
  expect(describeViolations(results)).toBe("");
});

test("colour contrast passes everywhere text appears", async ({ page }) => {
  // Called out separately because the palette carries meaning — OBSERVED green,
  // HUMAN RCA purple, UNVERIFIED amber — and a contrast failure there is a
  // failure of the classification system, not just of styling.
  const results = await new AxeBuilder({ page })
    .withRules(["color-contrast"])
    .analyze();
  expect(describeViolations(results)).toBe("");
});

test("landmarks and heading structure are navigable", async ({ page }) => {
  const results = await new AxeBuilder({ page })
    .withRules([
      "landmark-one-main",
      "landmark-unique",
      "page-has-heading-one",
      "heading-order",
      "region",
      "bypass",
    ])
    .analyze();
  expect(describeViolations(results)).toBe("");
});

test("every control has an accessible name", async ({ page }) => {
  const results = await new AxeBuilder({ page })
    .withRules(["button-name", "link-name", "label", "aria-command-name", "input-button-name"])
    .analyze();
  expect(describeViolations(results)).toBe("");
});

test("the timeline controls are accessible at every position", async ({ page }) => {
  // State-dependent panels can only be scanned in each state. The fleet panel
  // is empty at position zero and full later; both have to pass.
  for (const chapterName of ["FleetForge reports BLOCKED", "Second manual uncordon"]) {
    await page.locator(".chapter-button", { hasText: chapterName }).click();
    await expect(page.locator('.position-strip:not([data-stale="true"])')).toBeVisible();
    const results = await scan(page, "#timeline");
    expect(describeViolations(results), `at chapter: ${chapterName}`).toBe("");
  }
});

test("the evidence drawer is accessible once opened", async ({ page }) => {
  await page.locator("#evidence").scrollIntoViewIfNeeded();
  await page.locator("#evidence").getByRole("button", { name: "00-CONCLUSIONS.md" }).click();
  await expect(page.locator("#evidence-drawer .artifact")).toBeVisible({ timeout: 10_000 });
  const results = await scan(page, "#evidence");
  expect(describeViolations(results)).toBe("");
});

test("the investigation chain is accessible with a fact expanded", async ({ page }) => {
  await page.locator("#chain").scrollIntoViewIfNeeded();
  await page.locator(".chain-box", { hasText: "Disruption budget exhausted" }).click();
  const results = await scan(page, "#chain");
  expect(describeViolations(results)).toBe("");
});

test("the narrow viewport is accessible too", async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await page.reload();
  await expect(page.locator(".banner-replay")).toBeVisible();
  const results = await scan(page);
  expect(describeViolations(results)).toBe("");
});

test("the error screen is accessible", async ({ page }) => {
  await page.route("**/api/v1/environment", (route) => route.abort());
  await page.goto("/");
  await expect(page.getByText("FleetForge is not reachable")).toBeVisible();
  const results = await scan(page);
  expect(describeViolations(results)).toBe("");
});

/* ------------------------------------------------------- keyboard and focus */

test("tab order reaches the controls in the order they are read", async ({ page }) => {
  const seen: string[] = [];
  for (let i = 0; i < 14; i += 1) {
    await page.keyboard.press("Tab");
    seen.push(
      await page.evaluate(() => {
        const el = document.activeElement;
        if (!el) return "none";
        const label = (el.textContent ?? "").trim().slice(0, 28);
        return `${el.tagName.toLowerCase()}${label ? `:${label}` : ""}`;
      }),
    );
  }
  // The skip link first, before any cluster data.
  expect(seen[0]).toMatch(/^a:Skip to content/);
  // No control is skipped over into the void.
  expect(seen.filter((s) => s === "none")).toHaveLength(0);
});

test("the skip link moves focus to the main region", async ({ page }) => {
  await page.keyboard.press("Tab");
  await expect(page.locator(":focus")).toHaveText(/skip to content/i);
  await page.keyboard.press("Enter");
  expect(await page.evaluate(() => window.location.hash)).toBe("#content");
  await expect(page.locator("main#content")).toBeVisible();
});

test("focus is visible on every control, not merely present", async ({ page }) => {
  const controls = [
    page.getByRole("button", { name: "Play", exact: true }),
    page.getByRole("button", { name: "Restart" }),
    page.locator(".chapter-button").first(),
    page.locator(".scrub input"),
    page.locator(".chain-head").first(),
    page.locator("abbr.term").first(),
  ];
  for (const control of controls) {
    await control.scrollIntoViewIfNeeded();
    await control.focus();
    await expect(control).toBeFocused();
    // `getComputedStyle(el, ":focus-visible")` returns an empty declaration —
    // the second argument takes a pseudo-*element*, and :focus-visible is a
    // pseudo-*class*. An assertion written that way compares "" to "none",
    // passes for every element on the page, and proves nothing. Read the plain
    // computed style of the genuinely focused element instead.
    const ring = await control.evaluate((el) => {
      const s = getComputedStyle(el);
      return {
        focusVisible: el.matches(":focus-visible"),
        style: s.outlineStyle,
        width: parseFloat(s.outlineWidth),
      };
    });
    const where = await control.evaluate((e) => `${e.tagName}.${e.className}`);
    expect(ring.focusVisible, `${where} is focused but not focus-visible`).toBe(true);
    expect(ring.style, `${where} has no focus outline`).not.toBe("none");
    expect(ring.width, `${where} has a zero-width focus outline`).toBeGreaterThan(0);
  }
});

test("the mode banner is announced, not merely drawn", async ({ page }) => {
  // A screen-reader user must learn this is a replay before any cluster data.
  const banner = page.locator(".banner-replay");
  await expect(banner).toHaveAttribute("role", "note");
  await expect(banner).toContainText("REPLAY — CAPTURED FROM REAL EKS/BOTTLEROCKET EXECUTION");

  const order = await page.evaluate(() => {
    const b = document.querySelector(".banner-replay");
    const m = document.querySelector("main");
    if (!b || !m) return "missing";
    return b.compareDocumentPosition(m) & Node.DOCUMENT_POSITION_FOLLOWING
      ? "banner first"
      : "main first";
  });
  expect(order).toBe("banner first");
});
