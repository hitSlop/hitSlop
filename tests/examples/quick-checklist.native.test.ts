// Quick Checklist's editing and capture behavior, in WebKit through the dev preview. Kept
// outside the example, so a copied example stays self-contained.
import { expect, test } from "bun:test";
import { webkit, type Page } from "playwright";
import { mkdtemp, rm, cp } from "node:fs/promises";
import { join } from "node:path";
import { startDev } from "../../packages/cli/src/dev";

async function preview(run: (page: Page) => Promise<void>) {
  const root = await mkdtemp(join(process.cwd(), ".dev-test-"));
  const source = join(root, "quick-checklist");
  await cp(join(import.meta.dir, "../../examples/slops/quick-checklist"), source, { recursive: true });
  const dev = await startDev(source);
  const browser = await webkit.launch();
  try {
    const page = await browser.newPage();
    page.setDefaultTimeout(15000);
    await page.goto(dev.url);
    await page.frameLocator("iframe").getByRole("textbox", { name: "New task" }).waitFor();
    await run(page);
  } finally {
    await browser.close();
    await dev.close();
    await rm(root, { recursive: true, force: true });
  }
}
const app = (page: Page) => page.frames().find((frame) => frame.url().includes("/app.html"))!;
const focused = (page: Page) => app(page).evaluate(() => document.activeElement?.getAttribute("aria-label"));

// An IME's Enter commits the composition; only Enter after it finishes editing the task.
test("Enter while composing keeps the task editor; Enter after composing moves to New task", () =>
  preview(async (page) => {
    const frame = page.frameLocator("iframe");
    await frame.getByRole("textbox", { name: "Task 1" }).click();
    const editor = frame.locator('textarea[aria-label="Task 1"]');
    await editor.waitFor();
    await editor.dispatchEvent("keydown", { key: "Enter", isComposing: true, bubbles: true, cancelable: true });
    expect(await focused(page)).toBe("Task 1");
    await editor.press("Enter");
    expect(await focused(page)).toBe("New task");
  }), 60000);

// A fresh renderer starts with the default local view for both preview and export.
test("fresh preview and export show the default active tasks", () =>
  preview(async (page) => {
    const capture = (mode: "preview" | "export") =>
      app(page).evaluate(async (mode) => {
        const { capture } = (globalThis as any).__slop;
        const token = crypto.randomUUID();
        await capture.begin(token, mode);
        try {
          return document.querySelector('[aria-label="Exported checklist"]')?.textContent ?? "";
        } finally {
          await capture.restore(token);
        }
      }, mode);
    const preview = await capture("preview");
    expect(preview).toContain("Send the first draft");
    expect(preview).not.toContain("No filed tasks yet");
    expect(await capture("export")).toContain("Send the first draft");
  }), 60000);
