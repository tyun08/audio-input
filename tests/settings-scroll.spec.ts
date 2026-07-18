import { test, expect } from "@playwright/test";
import { loadApp, openSettings } from "./helpers.js";

const MIN_ROW_HEIGHT = 48;
const SCROLL_WIDTH_TOLERANCE = 1;

test.describe("Settings Panel scrolling", () => {
  test("keeps rows readable and uses vertical scrolling without horizontal overflow", async ({
    page,
  }) => {
    const now = Date.now();
    await page.setViewportSize({ width: 620, height: 480 });
    await loadApp(page);
    await page.evaluate((seedNow) => {
      const entries = Array.from({ length: 7 }, (_, index) => ({
        id: `session-${index + 1}`,
        createdAtMs: seedNow - index * 60_000,
        durationS: 12.5 + index,
        provider: "groq",
        rawText: "",
        polishedText: "",
        status: "failed",
        error:
          "这是一个用于验证设置滚动区域的较长失败消息，用来确保内容会换行显示，而不是横向裁切或压缩控件高度。",
        polishFailed: false,
      }));
      (window as any).__tauriSetResponse("list_history", entries);
      (window as any).__tauriSetResponse("get_max_history", 100);
    }, now);
    await openSettings(page);

    await page.getByRole("button", { name: /General|通用/i }).click();
    await page.getByRole("button", { name: "中文" }).click();
    await page.getByRole("button", { name: /History|历史/i }).click();

    const content = page.locator(".content");
    await expect(content).toBeVisible();
    await expect(page.locator(".history-item")).toHaveCount(7);

    const defaultMetrics = await content.evaluate((node) => {
      const rows = Array.from(node.querySelectorAll<HTMLElement>(".row"));
      return {
        minRowHeight: Math.min(...rows.map((row) => row.getBoundingClientRect().height)),
        scrollWidth: node.scrollWidth,
        clientWidth: node.clientWidth,
        scrollHeight: node.scrollHeight,
        clientHeight: node.clientHeight,
      };
    });

    expect(defaultMetrics.minRowHeight).toBeGreaterThanOrEqual(MIN_ROW_HEIGHT);
    expect(defaultMetrics.scrollWidth).toBeLessThanOrEqual(
      defaultMetrics.clientWidth + SCROLL_WIDTH_TOLERANCE
    );
    expect(defaultMetrics.scrollHeight).toBeGreaterThan(defaultMetrics.clientHeight);

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
