// What the interface actually renders, in a real browser.
//
// Component tests assert that a function returns the right element. These
// assert that a person looking at the screen is told the truth — which is a
// different claim, and the one that matters for a tool whose entire value is
// not misleading an operator.

import { expect, test } from "@playwright/test";

test.describe("data mode chrome", () => {
  test("fixture mode is labelled, prominently and without a dismiss control", async ({ page }) => {
    await page.goto("/");
    const badge = page.locator(".mode-badge");
    await expect(badge).toHaveText("FIXTURE");
    await expect(badge).toBeVisible();

    // No close button, anywhere near it. The label is chrome, not a
    // notification (ADR-0003).
    await expect(badge.locator("button")).toHaveCount(0);

    // And the banner spells out what it means.
    await expect(page.getByText(/recorded data loaded from disk/i)).toBeVisible();
    await expect(page.getByText(/does not make it live/i)).toBeVisible();
  });

  test("the badge survives scrolling, because the header is sticky", async ({ page }) => {
    await page.goto("/");
    await page.mouse.wheel(0, 4000);
    await expect(page.locator(".mode-badge")).toBeInViewport();
  });
});

test.describe("fleet overview", () => {
  test("renders real nodes with their resourceVersions", async ({ page }) => {
    await page.goto("/");
    const panel = page.locator("section.panel", { hasText: "Fleet overview" });
    await expect(panel).toBeVisible();

    // The captured fixture is the three-node kind cluster.
    await expect(panel.getByRole("row")).toHaveCount(4); // header + 3 nodes
    await expect(panel.getByText("fleetforge-dev-control-plane")).toBeVisible();

    // Provenance is on screen, not buried in a tooltip.
    await expect(panel.locator("th", { hasText: "rv" })).toBeVisible();
  });

  test("shows the client/server version skew rather than hiding it", async ({ page }) => {
    await page.goto("/");
    const panel = page.locator("section.panel", { hasText: "Environment & permissions" });
    await expect(panel.getByText("Client bindings target")).toBeVisible();
  });
});

test.describe("preflight workspace", () => {
  test("computes a verdict when a node is selected", async ({ page }) => {
    await page.goto("/");
    const workspace = page.locator("section.panel", { hasText: "Preflight workspace" });
    await expect(workspace.getByText("No nodes selected")).toBeVisible();

    await workspace.getByRole("checkbox").first().check();

    const verdict = workspace.locator(".verdict");
    await expect(verdict).toBeVisible({ timeout: 15_000 });
    await expect(verdict).toHaveText(/SAFE|BLOCKED/);

    // A preflight result is a calculation, and says so.
    await expect(workspace.locator(".mode-what_if")).toHaveText("WHAT-IF");
  });

  test("the evidence drawer shows the field, the value, and the arithmetic", async ({ page }) => {
    await page.goto("/");
    const workspace = page.locator("section.panel", { hasText: "Preflight workspace" });
    await workspace.getByRole("checkbox").first().check();
    await expect(workspace.locator(".verdict")).toBeVisible({ timeout: 15_000 });

    const finding = workspace.locator(".finding").first();
    await expect(finding).toBeVisible();

    const header = finding.locator(".finding-header");
    if ((await header.getAttribute("aria-expanded")) === "false") {
      await header.click();
    }

    const drawer = finding.locator(".drawer");
    await expect(drawer).toBeVisible();
    await expect(drawer.getByRole("heading", { name: /Limitations/i })).toBeVisible();
    await expect(drawer.getByText(/does not prove/i)).toBeVisible();
  });

  test("every finding states its limitations on screen", async ({ page }) => {
    // The rule that keeps the product honest, checked where a human would see
    // it rather than only in the JSON.
    await page.goto("/");
    const workspace = page.locator("section.panel", { hasText: "Preflight workspace" });
    await workspace.getByRole("checkbox").first().check();
    await expect(workspace.locator(".verdict")).toBeVisible({ timeout: 15_000 });

    const findings = workspace.locator(".finding");
    const count = await findings.count();
    expect(count).toBeGreaterThan(0);

    for (let i = 0; i < count; i++) {
      const finding = findings.nth(i);
      const header = finding.locator(".finding-header");
      if ((await header.getAttribute("aria-expanded")) === "false") {
        await header.click();
      }
      await expect(finding.locator(".limitations li").first()).toBeVisible();
    }
  });

  test("concurrency is adjustable and recomputes", async ({ page }) => {
    await page.goto("/");
    const workspace = page.locator("section.panel", { hasText: "Preflight workspace" });
    const boxes = workspace.getByRole("checkbox");
    await boxes.nth(0).check();
    await boxes.nth(1).check();

    await expect(workspace.locator(".verdict")).toBeVisible({ timeout: 15_000 });

    const slider = workspace.locator('input[type="range"]');
    await expect(slider).toBeEnabled();
    await slider.fill("2");
    await expect(workspace.locator("output")).toHaveText("2");
    await expect(workspace.locator(".verdict")).toBeVisible({ timeout: 15_000 });
  });
});

test.describe("accessibility", () => {
  test("the page is navigable by keyboard and focus is visible", async ({ page }) => {
    await page.goto("/");
    await page.keyboard.press("Tab");
    const focused = page.locator(":focus");
    await expect(focused).toBeVisible();

    // The skip link is the first stop.
    await expect(focused).toHaveText(/skip to content/i);
  });

  test("tables use header cells, so a screen reader can navigate them", async ({ page }) => {
    await page.goto("/");
    const panel = page.locator("section.panel", { hasText: "Fleet overview" });
    await expect(panel.locator("th[scope='col']").first()).toBeVisible();
    await expect(panel.locator("th[scope='row']").first()).toBeVisible();
  });

  test("renders on a narrow viewport without horizontal overflow", async ({ page }) => {
    await page.setViewportSize({ width: 390, height: 844 });
    await page.goto("/");
    await expect(page.locator(".mode-badge")).toBeVisible();

    const overflow = await page.evaluate(
      () => document.documentElement.scrollWidth - document.documentElement.clientWidth,
    );
    expect(overflow).toBeLessThanOrEqual(1);
  });
});

test.describe("screenshots", () => {
  // Not assertions. These exist so a human can look at the thing.
  test("capture the interface", async ({ page }) => {
    await page.setViewportSize({ width: 1440, height: 1000 });
    await page.goto("/");
    await page.waitForTimeout(1000);
    await page.screenshot({ path: "e2e/screenshots/overview.png", fullPage: false });

    const workspace = page.locator("section.panel", { hasText: "Preflight workspace" });
    await workspace.scrollIntoViewIfNeeded();
    await workspace.getByRole("checkbox").first().check();
    await expect(workspace.locator(".verdict")).toBeVisible({ timeout: 15_000 });
    await page.waitForTimeout(500);
    await page.screenshot({ path: "e2e/screenshots/preflight.png", fullPage: false });

    const finding = workspace.locator(".finding").first();
    const header = finding.locator(".finding-header");
    if ((await header.getAttribute("aria-expanded")) === "false") {
      await header.click();
    }
    await finding.scrollIntoViewIfNeeded();
    await page.waitForTimeout(300);
    await page.screenshot({ path: "e2e/screenshots/evidence.png", fullPage: false });
  });
});
