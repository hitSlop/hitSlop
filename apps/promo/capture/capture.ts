/**
 * Records real tinys example apps (served by `tiny dev`) as alpha PNG sequences in public/captures.
 * Usage: bun capture/capture.ts [scene ...]   (defaults to every scene)
 */
import type { Locator, Page } from "playwright";
import { basename } from "node:path";
import { launch, openTiny, record, still } from "./lib";

const skin = process.env.PROMO_SKIN ?? `${process.env.HOME}/Desktop/tinys/Tenchi Muyo - Aeka.wsz`;
const pet = process.env.PROMO_PET ?? `${process.env.HOME}/Desktop/tinys/akitsuki-airi.codex-pet.zip`;

/** Drags a real file over the page (showing the app's drop affordance), then drops it. */
async function dropFile(page: Page, path: string, mime: string, hover = 600) {
  const bytes = Buffer.from(await Bun.file(path).arrayBuffer()).toString("base64");
  const transfer = await page.evaluateHandle(
    ({ bytes, name, mime }) => {
      const data = Uint8Array.from(atob(bytes), (c) => c.charCodeAt(0));
      const dt = new DataTransfer();
      dt.items.add(new File([data], name, { type: mime }));
      return dt;
    },
    { bytes, name: basename(path), mime },
  );
  const { width, height } = page.viewportSize()!;
  const at = { clientX: width / 2, clientY: height / 2, bubbles: true, cancelable: true };
  const target = await page.evaluateHandle(({ x, y }) => document.elementFromPoint(x, y), { x: at.clientX, y: at.clientY });
  for (const type of ["dragenter", "dragover"])
    await target.evaluate((el, [type, dt, at]) => el!.dispatchEvent(new DragEvent(type as string, { ...(at as object), dataTransfer: dt as DataTransfer })), [type, transfer, at] as const);
  await page.waitForTimeout(hover);
  await target.evaluate((el, [dt, at]) => el!.dispatchEvent(new DragEvent("drop", { ...(at as object), dataTransfer: dt as DataTransfer })), [transfer, at] as const);
}

// Activates a control directly; Playwright's hit-test refuses buttons that decorative overlays cover.
const press = (locator: Locator) => locator.evaluate((el) => (el as HTMLElement).click());

const scenes: Record<string, () => Promise<void>> = {
  async soma() {
    const s = await openTiny(browser, "soma-amp");
    s.page.on("response", (r) => r.status() >= 400 && console.warn("[soma]", r.status(), r.url()));
    await s.page.locator("#play").waitFor();
    await s.page.waitForTimeout(1200);
    await record(s, "soma", async (mark) => {
      await s.page.waitForTimeout(300);
      mark("play");
      await s.page.locator("#play").evaluate((el) => {
        for (const type of ["pointerdown", "mousedown", "pointerup", "mouseup", "click"])
          el.dispatchEvent(new (type.startsWith("pointer") ? PointerEvent : MouseEvent)(type, { bubbles: true, cancelable: true, view: window, button: 0 }));
      });
      await s.page.waitForTimeout(3800);
      mark("drag");
      await dropFile(s.page, skin, "application/zip", 700);
      mark("skin");
      await s.page.waitForTimeout(4000);
    });
    await s.close();
  },

  async pet() {
    const s = await openTiny(browser, "codex-pet");
    await record(s, "pet", async (mark) => {
      await s.page.waitForTimeout(2200);
      mark("drag");
      await dropFile(s.page, pet, "application/zip", 700);
      mark("pet");
      await s.page.waitForTimeout(5000);
    });
    await s.close();
  },

  // Mirrors the CLI edits shown in the terminal scene (see terminal.json): same values, applied live.
  async checklist() {
    const s = await openTiny(browser, "quick-checklist");
    await record(s, "checklist", async (mark) => {
      await s.page.waitForTimeout(1500);
      mark("title");
      await s.page.getByPlaceholder("Name your list").fill("Finals week");
      await s.page.getByPlaceholder("Name your list").blur();
      await s.page.waitForTimeout(1200);
      mark("insert");
      const add = s.page.getByPlaceholder("Add a little thing…");
      await add.fill("Review bio flashcards");
      await add.press("Enter");
      await add.blur();
      await s.page.waitForTimeout(3000);
    });
    await s.close();
  },

  async timer() {
    const s = await openTiny(browser, "focus-timer");
    await record(s, "timer", async (mark) => {
      await press(s.page.getByRole("button", { name: "Start" }));
      await s.page.mouse.move(0, 0);
      await s.page.waitForTimeout(2500);
      mark("theme");
      await s.page.evaluate(() => {
        const theme = { surface: "#3b6fd4", surfaceLight: "#5b8ef0", surfaceDeep: "#284f9e", accent: "#2f5fc0" };
        for (const [k, v] of Object.entries(theme)) document.documentElement.style.setProperty(`--tiny-${k}`, v);
      });
      await s.page.waitForTimeout(3500);
    });
    await s.close();
  },

  async koi() {
    const s = await openTiny(browser, "koi-pond", 1);
    await record(s, "koi", () => s.page.waitForTimeout(4500));
    await s.close();
  },

  async doodle() {
    const s = await openTiny(browser, "doodle-board");
    await record(s, "doodle", async () => {
      await s.page.waitForTimeout(300);
      // One continuous heart; separate strokes get bridged by the brush smoothing.
      // The p5 canvas divides pointer positions by the emulated 2x DPR, so coordinates are doubled.
      const at = (x: number, y: number) => [x * 2, y * 2] as const;
      const points = Array.from({ length: 110 }, (_, i) => {
        const t = Math.PI + (i / 109) * Math.PI * 2;
        return [372 + 12 * 16 * Math.sin(t) ** 3, 290 - 12 * (13 * Math.cos(t) - 5 * Math.cos(2 * t) - 2 * Math.cos(3 * t) - Math.cos(4 * t))];
      });
      await s.page.mouse.move(...at(points[0][0], points[0][1]));
      await s.page.mouse.down();
      for (const [x, y] of points) {
        await s.page.mouse.move(...at(x, y));
        await s.page.waitForTimeout(12);
      }
      await s.page.mouse.up();
      await s.page.mouse.move(0, 0);
      await s.page.waitForTimeout(1500);
    });
    await s.close();
  },

  async flashcards() {
    const s = await openTiny(browser, "flashcards");
    await record(s, "flashcards", async (mark) => {
      await s.page.waitForTimeout(1200);
      mark("reveal");
      await press(s.page.getByRole("button", { name: /Reveal answer/ }));
      await s.page.mouse.move(0, 0);
      await s.page.waitForTimeout(2200);
    });
    await s.close();
  },

  async wordle() {
    const s = await openTiny(browser, "wordle");
    await record(s, "wordle", async () => {
      await s.page.waitForTimeout(400);
      for (const word of ["CRANE", "TINYE"]) {
        for (const letter of word) {
          await s.page.keyboard.press(letter);
          await s.page.waitForTimeout(90);
        }
        await s.page.keyboard.press("Enter");
        await s.page.waitForTimeout(1500);
      }
    });
    await s.close();
  },

  async stills() {
    for (const slug of ["school-schedule", "assignment-tracker", "kanban-board", "side-quest", "pixel-art", "habit-heatmap", "pocket-sheet", "semester-planner"]) {
      const s = await openTiny(browser, slug);
      await s.page.waitForTimeout(1200);
      await still(s, `still-${slug}`);
      await s.close();
    }
  },
};

const browser = await launch();
const wanted = process.argv.slice(2);
try {
  for (const name of wanted.length ? wanted : Object.keys(scenes)) await scenes[name]();
} finally {
  await browser.close();
}
