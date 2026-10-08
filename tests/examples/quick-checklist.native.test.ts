// Quick Checklist's editing and capture behavior, in WebKit through the dev preview. Kept
// outside the example, so a copied example stays self-contained.
import { expect, test } from "bun:test";
import { webkit, type Page } from "playwright";
import { mkdtemp, rm, cp } from "node:fs/promises";
import { join } from "node:path";
import { startDev } from "../../packages/hitslop/src/cli/dev";

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

test("tasks can be added, completed, filed, restored, and removed through their controls", () =>
  preview(async (page) => {
    const frame = page.frameLocator("iframe");
    const composer = frame.getByRole("textbox", { name: "New task" });
    await composer.fill("Added with Enter");
    await composer.press("Enter");
    const first = frame.getByRole("checkbox", { name: "Mark Added with Enter complete", exact: true });
    await first.waitFor();
    expect(await composer.inputValue()).toBe("");
    await composer.fill("Added with button");
    await frame.getByRole("button", { name: "Add task", exact: true }).click();
    await frame.getByRole("checkbox", { name: "Mark Added with button complete", exact: true }).waitFor();
    expect(await composer.inputValue()).toBe("");
    await first.check();
    await frame.getByRole("button", { name: "File finished (2)", exact: true }).click();
    await first.waitFor({ state: "detached" });
    await frame.getByRole("tab", { name: "Filed 2", exact: true }).click();
    const restore = frame.getByRole("button", { name: "Restore Added with Enter", exact: true });
    await restore.click();
    await restore.waitFor({ state: "detached" });
    await frame.getByRole("tab", { name: "To do 4", exact: true }).click();
    expect(await first.isChecked()).toBe(false);
    await frame.getByRole("button", { name: "Actions for Added with Enter", exact: true }).click();
    await frame.getByRole("menuitem", { name: "Remove task", exact: true }).click();
    await first.waitFor({ state: "detached" });
    expect(await frame.getByRole("checkbox").count()).toBe(3);
    expect(await frame.getByRole("status").textContent()).toBe("Task removed.");
  }), 60000);

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

// A row is text until edited: the caret lands where the person clicked, the field grows
// with its text, and leaving it shows text again.
test("a task edits in place at the clicked caret, grows, and returns to text", () =>
  preview(async (page) => {
    const frame = page.frameLocator("iframe");
    const position = await app(page).evaluate(() => {
      const text = document.querySelector('[role="textbox"][aria-label="Task 2"]')!;
      const range = document.createRange();
      range.setStart(text.firstChild!, 5);
      range.setEnd(text.firstChild!, 6);
      const character = range.getBoundingClientRect(), box = text.getBoundingClientRect();
      return { x: character.left - box.left + 1, y: character.top - box.top + character.height / 2 };
    });
    await frame.getByRole("textbox", { name: "Task 2" }).click({ position });
    const field = frame.locator('textarea[aria-label="Task 2"]');
    await field.waitFor();
    expect(await field.evaluate((element: HTMLTextAreaElement) => element.selectionStart)).toBe(5);
    const height = () => field.evaluate((element) => element.getBoundingClientRect().height);
    const before = await height();
    await field.press("End");
    await field.pressSequentially(" and then along the river, past the bakery, over the bridge and back home again before dark");
    expect(await height()).toBeGreaterThan(before);
    await frame.getByRole("textbox", { name: "New task" }).click();
    await expect(field.count()).resolves.toBe(0);
    expect(await frame.getByRole("textbox", { name: "Task 2" }).textContent()).toContain("back home again");
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
