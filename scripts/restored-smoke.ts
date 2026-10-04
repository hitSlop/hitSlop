// Browser smoke for the active templates: each runs in `slop dev` (the WASM core) under
// Playwright WebKit and must render with no page errors; templates with a scripted check
// below must also leave the expected DOM.
// Usage: bun scripts/restored-smoke.ts [slug…]
import { webkit, type Page } from "playwright";
import assert from "node:assert/strict";
import { discoverTemplates } from "./templates";

const checks: Record<string, (page: Page) => Promise<void>> = {
  "small-expenses": async (page) => {
    await page.waitForSelector(".expenses-row");
    const rows = () => page.locator(".expenses-row").count();
    const before = await rows();
    await page.fill('input[aria-label="Merchant"]', "Bookshop");
    await page.fill('input[aria-label="Amount"]', "12.50");
    await page.fill('input[aria-label="Note"]', "gift");
    await page.click('button:has-text("Add")');
    await page.waitForFunction((n) => document.querySelectorAll(".expenses-row").length === n, before + 1);
    assert.equal(await page.locator(".expenses-note", { hasText: "gift" }).count(), 1);
    // Clearing an optional note removes it.
    await page.locator('.expenses-row:has-text("gift") button:has-text("Clear note")').click();
    await page.waitForFunction(() => ![...document.querySelectorAll(".expenses-note")].some((n) => n.textContent === "gift"));
    // The currency enum relabels totals.
    await page.click('[role="tab"]:has-text("USD")');
    await page.waitForFunction(() => document.querySelector("[data-total]")?.textContent?.includes("US$"));
  },
  "kanban-board": async (page) => {
    await page.waitForSelector(".board-lane");
    const limit = page.locator(".board-lane-limit input").first();
    await limit.fill("1");
    await limit.dispatchEvent("change");
    await page.waitForFunction(() => (document.querySelector(".board-lane-limit input") as HTMLInputElement).value === "1");
    await limit.fill("");
    await limit.dispatchEvent("change");
    await page.waitForFunction(() => (document.querySelector(".board-lane-limit input") as HTMLInputElement).value === "");
    // An out-of-range limit is clamped to the schema's maximum.
    await limit.fill("5000");
    await limit.dispatchEvent("change");
    await page.waitForFunction(() => (document.querySelector(".board-lane-limit input") as HTMLInputElement).value === "999");
  },
  recipe: async (page) => {
    const servings = page.locator('input[aria-label="Servings"]');
    await servings.waitFor();
    await servings.fill("6");
    await servings.dispatchEvent("change");
    await servings.blur();
    await page.waitForFunction(() => (document.querySelector('input[aria-label="Servings"]') as HTMLInputElement).value === "6");
    await servings.fill("");
    await servings.dispatchEvent("change");
    await servings.blur();
    await page.waitForFunction(() => (document.querySelector('input[aria-label="Servings"]') as HTMLInputElement).value === "");
  },
  "doodle-board": async (page) => {
    const board = page.locator("svg.doodle-canvas");
    await board.waitFor();
    const strokes = () => page.locator("svg.doodle-canvas path[data-stroke]").count();
    const before = await strokes();
    const box = (await board.boundingBox())!;
    await page.mouse.move(box.x + box.width * 0.3, box.y + box.height * 0.4);
    await page.mouse.down();
    for (let i = 1; i <= 12; i++) await page.mouse.move(box.x + box.width * (0.3 + i * 0.02), box.y + box.height * 0.4 + i * 3);
    await page.mouse.up();
    // Drawn locally, the stroke is written once, with its final path, when the gesture ends.
    await page.waitForFunction(
      (n) => document.querySelectorAll("svg.doodle-canvas path[data-stroke]").length > n,
      before,
      { timeout: 5000 },
    );
    const geometry = await page.locator("svg.doodle-canvas path[data-stroke]").last().getAttribute("d");
    assert.ok(geometry && geometry.length > 20, `stroke path: ${geometry}`);
  },
  // Record entry by date: a day's checkbox toggles a check-in.
  "habit-heatmap": async (page) => {
    const day = page.locator('button[role="checkbox"]:not([disabled])').last();
    await day.waitFor();
    await day.click();
    await page.waitForFunction(() => {
      const days = [...document.querySelectorAll('button[role="checkbox"]:not([disabled])')];
      return days.at(-1)?.getAttribute("data-state") === "checked";
    });
  },
  // Scalar list insert: a new attendee pill appears.
  "meeting-notes": async (page) => {
    const input = page.locator('input[aria-label="Add attendee"]');
    await input.waitFor();
    await input.fill("Robin");
    await input.press("Enter");
    await page.waitForSelector('[aria-label="Remove Robin"]');
  },
  // Scalar list insert at the end of the presets.
  "metronome-tapper": async (page) => {
    const save = page.locator('button[aria-label="Save current tempo"]');
    await save.waitFor();
    await save.click();
    await page.waitForTimeout(300);
  },
  // Scalar list preview while painting, then set on release.
  "pixel-art": async (page) => {
    const canvas = page.locator('[aria-label="16 by 16 pixel canvas"]');
    await canvas.waitFor();
    const box = (await canvas.boundingBox())!;
    await page.mouse.move(box.x + box.width * 0.2, box.y + box.height * 0.2);
    await page.mouse.down();
    await page.mouse.move(box.x + box.width * 0.4, box.y + box.height * 0.2);
    await page.mouse.up();
    await page.waitForTimeout(300);
  },
  // Record entry of objects: a cell's contents are written through the formula bar.
  "pocket-sheet": async (page) => {
    const bar = page.locator('input[aria-label^="Contents of"]');
    await bar.waitFor();
    await bar.fill("42");
    await bar.press("Enter");
    await page.waitForFunction(() => document.querySelector('[role="grid"]')?.textContent?.includes("42"));
  },
  // Record entry text created by an effect and edited with bindText.
  "morning-pages": async (page) => {
    const area = page.locator('[aria-label="Morning Pages writing area"]');
    await area.waitFor();
    await area.click();
    await page.keyboard.type("Three pages, starting now.");
    await page.waitForTimeout(300);
    assert.ok((await area.inputValue()).includes("Three pages"));
  },
  // Scalar list replace: a set's checkbox marks it complete.
  "workout-planner": async (page) => {
    const set = page.locator('[aria-label^="Set 1 of"]').first();
    await set.waitFor();
    await set.click();
    await page.waitForFunction(() => document.querySelector('[aria-label^="Set 1 of"]')?.getAttribute("data-state") === "checked");
  },
};
/** Slops without a targeted check still must load and render without page errors. */
const settle = async (page: Page) => {
  await page.waitForLoadState("networkidle").catch(() => {});
  await page.waitForTimeout(1000);
};
// Every active template by default, so a restored example is covered without an edit here.
const slugs = process.argv.slice(2).length ? process.argv.slice(2) : (await discoverTemplates()).map((template) => template.slug);
const browser = await webkit.launch();
let failed = 0;
for (const [index, slug] of slugs.entries()) {
  const port = 5301 + index;
  const server = Bun.spawn(["bun", "packages/cli/src/cli.ts", "dev", `examples/slops/${slug}`, "--port", String(port)], {
    stdout: "pipe",
    stderr: "pipe",
  });
  const page = await browser.newPage({ viewport: { width: 900, height: 900 } });
  const errors: string[] = [];
  page.on("pageerror", (error) => errors.push(String(error)));
  page.on("console", (message) => {
    // Network loads (streams, remote media) are outside the document; app errors are not.
    if (message.type() === "error" && !/Failed to load resource|NotSupportedError|NotAllowedError/.test(message.text()))
      errors.push(message.text());
  });
  try {
    const url = `http://127.0.0.1:${port}/app.html`;
    for (let attempt = 0; ; attempt++) {
      try {
        await page.goto(url, { timeout: 5000 });
        break;
      } catch (error) {
        if (attempt > 60) throw error;
        await Bun.sleep(500);
      }
    }
    errors.length = 0; // connection refusals while the dev server started
    await (checks[slug] ?? settle)(page);
    assert.deepEqual(errors, [], `page errors: ${errors.join("\n")}`);
    console.log(`PASS ${slug}`);
  } catch (error) {
    failed++;
    console.log(`FAIL ${slug}: ${error instanceof Error ? error.message : error}`);
    await page.screenshot({ path: `.hitslop/evidence/restored-${slug}.png` }).catch(() => {});
  } finally {
    await page.close();
    server.kill();
  }
}
await browser.close();
if (failed) process.exit(1);
