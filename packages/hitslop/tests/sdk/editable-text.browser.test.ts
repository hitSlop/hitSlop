// SDK component contracts through a real native owner and WebKit, independent of examples.
import { expect, test } from "bun:test";
import { webkit, type Page } from "playwright";
import { mkdtemp, rm, cp } from "node:fs/promises";
import { join } from "node:path";
import { startDev } from "../../src/cli/dev";

async function preview(run: (page: Page) => Promise<void>) {
  const root = await mkdtemp(join(process.cwd(), ".dev-test-editable-"));
  let dev: Awaited<ReturnType<typeof startDev>> | undefined;
  let browser: Awaited<ReturnType<typeof webkit.launch>> | undefined;
  try {
    const source = join(root, "source");
    await cp(join(process.cwd(), "tests/apps/editable-text"), source, { recursive: true });
    dev = await startDev(source);
    browser = await webkit.launch();
    const page = await browser.newPage();
    page.setDefaultTimeout(15000);
    await page.goto(dev.url);
    await page.frameLocator("iframe").getByRole("textbox", { name: "First text" }).waitFor();
    await run(page);
  } finally {
    await browser?.close();
    await dev?.close();
    await rm(root, { recursive: true, force: true });
  }
}
const app = (page: Page) => page.frames().find(frame => frame.url().includes("/app.html"))!;
const focused = (page: Page) => app(page).evaluate(() => document.activeElement?.getAttribute("aria-label"));

// An IME's Enter commits the composition; only Enter after it finishes editing the field.
test("Enter while composing keeps the text editor; Enter after composing moves to Focus destination", () =>
  preview(async (page) => {
    const frame = page.frameLocator("iframe");
    await frame.getByRole("textbox", { name: "First text" }).click();
    const editor = frame.locator('textarea[aria-label="First text"]');
    await editor.waitFor();
    await editor.dispatchEvent("keydown", { key: "Enter", isComposing: true, bubbles: true, cancelable: true });
    expect(await focused(page)).toBe("First text");
    await editor.press("Enter");
    expect(await focused(page)).toBe("Focus destination");
  }), 60000);

test("Escape preserves typed text and returns focus; composition keeps editing", () =>
  preview(async (page) => {
    const frame = page.frameLocator("iframe");
    await frame.getByRole("textbox", { name: "First text" }).click();
    const editor = frame.locator('textarea[aria-label="First text"]');
    await editor.fill("Keep 日本😀 after Escape");
    await editor.dispatchEvent("keydown", { key: "Escape", isComposing: true, bubbles: true, cancelable: true });
    expect(await editor.count()).toBe(1);
    await editor.press("Escape");
    await editor.waitFor({ state: "detached" });
    expect(await focused(page)).toBe("First text");
    // The detached binding drains asynchronously; the display reads accepted snapshots.
    await app(page).waitForFunction(() =>
      document.querySelector('[role="textbox"][aria-label="First text"]')?.textContent === "Keep 日本😀 after Escape");
    expect(await frame.getByRole("textbox", { name: "First text" }).textContent()).toBe("Keep 日本😀 after Escape");
  }), 60000);

// A row is text until edited: the caret lands where the person clicked, the field grows
// with its text, and leaving it shows text again.
test("text edits in place at the clicked caret, grows, and returns to text", () =>
  preview(async (page) => {
    const frame = page.frameLocator("iframe");
    const position = await app(page).evaluate(() => {
      const text = document.querySelector('[role="textbox"][aria-label="Second text"]')!;
      const range = document.createRange();
      range.setStart(text.firstChild!, 5);
      range.setEnd(text.firstChild!, 6);
      const character = range.getBoundingClientRect(), box = text.getBoundingClientRect();
      return { x: character.left - box.left + 1, y: character.top - box.top + character.height / 2 };
    });
    await frame.getByRole("textbox", { name: "Second text" }).click({ position });
    const field = frame.locator('textarea[aria-label="Second text"]');
    await field.waitFor();
    expect(await field.evaluate((element: HTMLTextAreaElement) => element.selectionStart)).toBe(5);
    const height = () => field.evaluate((element) => element.getBoundingClientRect().height);
    const before = await height();
    await field.press("End");
    await field.pressSequentially(" and then along the river, past the bakery, over the bridge and back home again before dark");
    expect(await height()).toBeGreaterThan(before);
    await frame.getByRole("textbox", { name: "Focus destination" }).click();
    await expect(field.count()).resolves.toBe(0);
    expect(await frame.getByRole("textbox", { name: "Second text" }).textContent()).toContain("back home again");
  }), 60000);

// Programmatic owner updates must keep the DOM-derived sizing mirror in step.
test("accepted external text updates resize an open editor", () =>
  preview(async (page) => {
    const frame = page.frameLocator("iframe");
    await frame.getByRole("textbox", { name: "First text" }).click();
    const editor = frame.locator('textarea[aria-label="First text"]');
    await editor.waitFor();
    const before = await editor.evaluate(element => element.getBoundingClientRect().height);
    await app(page).evaluate(async () => {
      await (globalThis as any).setFixtureText("First line\nSecond line 日本😀\nThird line\nFourth line");
    });
    await app(page).waitForFunction(() => {
      const textarea = document.querySelector('textarea[aria-label="First text"]') as HTMLTextAreaElement;
      return textarea?.value.includes("Fourth line");
    });
    expect(await editor.evaluate(element => element.getBoundingClientRect().height)).toBeGreaterThan(before * 2);
    expect(await focused(page)).toBe("First text");
  }), 60000);

