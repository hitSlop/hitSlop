/** Refresh current native-owner previews; the real collaboration take is recorded separately. */
import { launch, openPop, record } from "./lib";
import { writeFile, readdir, readFile } from "node:fs/promises";
import { join } from "node:path";
const browser = await launch();
const names = process.argv.slice(2);
try {
  for (const slug of names.length
    ? names
    : [
        "pocket-pod",
        "flashcards",
        "focus-timer",
        "baby-journal",
        "codex-pet",
        "koi-pond",
        "doodle-board",
        "wordle",
        "pixel-art",
        "pocket-sheet",
      ]) {
    const s = await openPop(browser, slug);
    try {
      if (slug === "pocket-pod") {
        await s.page
          .getByLabel("Import local songs")
          .setInputFiles(join(import.meta.dir, "../public/music/new-york.mp3"));
        await s.page.waitForTimeout(1800);
        await s.page
          .getByText("Songs", { exact: true })
          .first()
          .evaluate((el) => (el as HTMLElement).click());
        await s.page.waitForTimeout(100);
        await s.page
          .getByText("Songs", { exact: true })
          .first()
          .evaluate((el) => (el as HTMLElement).click());
        await s.page
          .getByText(/new.york/i)
          .first()
          .evaluate((el) => (el as HTMLElement).click());
        await s.page.waitForTimeout(100);
        await s.page
          .getByText(/new.york/i)
          .first()
          .evaluate((el) => (el as HTMLElement).click());
        await s.page.locator("audio").evaluate((el) => {
          const audio = el as HTMLAudioElement;
          audio.pause();
          audio.currentTime = 89.14;
        });
        await s.page.waitForTimeout(4000);
      }
      await record(s, `launch-${slug}`, async (mark) => {
        await s.page.waitForTimeout(700);
        mark("action");
        if (slug === "flashcards")
          await s.page
            .getByRole("button", { name: /Reveal answer/ })
            .evaluate((el) => (el as HTMLElement).click());
        if (slug === "focus-timer") await s.page.getByRole("button", { name: /Start/ }).click();
        if (slug === "pocket-pod")
          await s.page
            .getByRole("button", { name: "Play local song", exact: true })
            .evaluate((el) => (el as HTMLElement).click());
        if (slug === "wordle") {
          await s.page.keyboard.type("CRANE", { delay: 100 });
          await s.page.keyboard.press("Enter");
        }
        if (slug === "doodle-board") {
          const rect = await s.page.locator("svg.doodle-canvas").boundingBox();
          if (!rect) throw new Error("Drawing board not visible");
          const points = Array.from({ length: 91 }, (_, i) => {
            const t = Math.PI + (2 * Math.PI * i) / 90;
            return {
              x: rect.x + (rect.width * (500 + 12 * 16 * Math.sin(t) ** 3)) / 1000,
              y:
                rect.y +
                (rect.height *
                  (340 -
                    12 *
                      (13 * Math.cos(t) -
                        5 * Math.cos(2 * t) -
                        2 * Math.cos(3 * t) -
                        Math.cos(4 * t)))) /
                  700,
            };
          });
          await s.page.mouse.move(points[0].x, points[0].y);
          await s.page.mouse.down();
          for (const point of points) {
            await s.page.mouse.move(point.x, point.y);
            await s.page.waitForTimeout(20);
          }
          await s.page.mouse.up();
        }
        await s.page.mouse.move(0, 0);
        await s.page.waitForTimeout(4500);
      });
      if (slug === "pocket-pod" || slug === "baby-journal") {
        await s.page.setViewportSize({ width: 390, height: 844 });
        await s.cdp.send("Emulation.setDeviceMetricsOverride", {
          width: 390,
          height: 844,
          deviceScaleFactor: 2,
          mobile: false,
        });
        await record(s, `launch-${slug}-phone`, () => s.page.waitForTimeout(4000));
      }
    } finally {
      await s.close();
    }
  }
} finally {
  await browser.close();
}
const captures = join(import.meta.dir, "../public/captures");
const metadata: Record<string, unknown> = {};
for (const dir of await readdir(captures))
  if (dir.startsWith("launch-")) {
    try {
      metadata[dir.slice(7)] = {
        ...JSON.parse(await readFile(join(captures, dir, "meta.json"), "utf8")),
        startFrame: 0,
      };
    } catch {}
  }
await writeFile(
  join(import.meta.dir, "../src/launch-media.json"),
  JSON.stringify(metadata, null, 2) + "\n",
);
