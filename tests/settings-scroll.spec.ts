import { test, expect } from "@playwright/test";
import { loadApp, openSettings } from "./helpers.js";

test.describe("Settings Panel scrolling", () => {
  test("keeps rows readable and uses vertical scrolling without horizontal overflow", async ({
    page,
  }) => {
    await page.setViewportSize({ width: 620, height: 480 });
    await loadApp(page);
    await openSettings(page);

    await page.getByRole("button", { name: /General|通用/i }).click();
    await page.getByRole("button", { name: "中文" }).click();
    await page.getByRole("button", { name: /Advanced|高级/i }).click();

    const content = page.locator(".content");
    await expect(content).toBeVisible();

    const defaultMetrics = await content.evaluate((node) => {
      const rows = Array.from(node.querySelectorAll<HTMLElement>(".row"));
      return {
        minRowHeight: Math.min(...rows.map((row) => row.getBoundingClientRect().height)),
        scrollWidth: node.scrollWidth,
        clientWidth: node.clientWidth,
      };
    });

    expect(defaultMetrics.minRowHeight).toBeGreaterThanOrEqual(48);
    expect(defaultMetrics.scrollWidth).toBeLessThanOrEqual(defaultMetrics.clientWidth + 1);

    await page.setViewportSize({ width: 620, height: 360 });

    const smallerMetrics = await content.evaluate((node) => ({
      clientHeight: node.clientHeight,
      scrollHeight: node.scrollHeight,
      scrollTop: node.scrollTop,
    }));

    expect(smallerMetrics.scrollHeight).toBeGreaterThan(smallerMetrics.clientHeight);

    await content.evaluate((node) => {
      node.scrollTop = node.scrollHeight;
    });

    await expect.poll(async () => content.evaluate((node) => node.scrollTop)).toBeGreaterThan(0);
    await expect(page.locator(".titlebar")).toBeVisible();
  });
});
