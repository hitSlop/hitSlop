import { chromium, type Browser, type CDPSession, type Page } from "playwright";
import { mkdir, rm, writeFile } from "node:fs/promises";
import { join, resolve } from "node:path";

export const repo = resolve(import.meta.dir, "../../..");
export const out = resolve(import.meta.dir, "../public/captures");
export const FPS = 30;
const chrome =
  process.env.PROMO_CHROME ??
  `${process.env.HOME}/Library/Caches/ms-playwright/chromium-1243/chrome-mac-arm64/Google Chrome for Testing.app/Contents/MacOS/Google Chrome for Testing`;
// SomaFM refuses the headless user agent.
const userAgent =
  "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/141.0.0.0 Safari/537.36";

export function launch() {
  return chromium.launch({
    executablePath: chrome,
    args: ["--use-gl=angle", "--mute-audio", "--autoplay-policy=no-user-gesture-required"],
  });
}

let nextPort = 5400;
/** Serves one example through `slop dev` and resolves once it answers. */
export async function serve(slug: string) {
  const port = nextPort++;
  const proc = Bun.spawn(["bun", "packages/hitslop/src/cli/cli.ts", "dev", `examples/slops/${slug}`, "--port", String(port)], {
    cwd: repo,
    stdout: "ignore",
    stderr: "inherit",
  });
  const url = `http://127.0.0.1:${port}/app.html`;
  for (let i = 0; i < 120; i++) {
    try {
      if ((await fetch(url)).ok) return { url, stop: () => proc.kill("SIGINT") };
    } catch {}
    await Bun.sleep(500);
  }
  proc.kill();
  throw new Error(`preview for ${slug} never started`);
}

export async function openSlop(browser: Browser, slug: string, scale = 2) {
  const manifest = await Bun.file(join(repo, "examples/slops", slug, "manifest.json")).json();
  const { width, height } = manifest.presentation;
  const server = await serve(slug);
  const page = await browser.newPage({ viewport: { width, height }, userAgent });
  // Playwright's deviceScaleFactor does not reach this Chromium build, so set it directly.
  const cdp = await page.context().newCDPSession(page);
  await cdp.send("Emulation.setDeviceMetricsOverride", { width, height, deviceScaleFactor: scale, mobile: false });
  await cdp.send("Emulation.setDefaultBackgroundColorOverride", { color: { r: 0, g: 0, b: 0, a: 0 } });
  page.on("pageerror", (e) => console.warn(`[${slug}]`, e.message));
  await page.goto(server.url);
  await page.waitForLoadState("networkidle").catch(() => {});
  await page.evaluate(() => document.fonts.ready);
  await page.waitForTimeout(800);
  return {
    page,
    cdp,
    width,
    height,
    async close() {
      await page.close();
      server.stop();
    },
  };
}

/**
 * Records a page as a constant-rate PNG sequence (alpha preserved) while `script` runs.
 * Frames are grabbed as fast as screenshots allow and resampled to FPS by holding the latest frame.
 * `mark(name)` records the output frame at which a scripted event happened.
 */
export async function record({ page, cdp }: { page: Page; cdp: CDPSession }, name: string, script: (mark: (label: string) => void) => Promise<void>) {
  const dir = join(out, name);
  await rm(dir, { recursive: true, force: true });
  await mkdir(dir, { recursive: true });
  const shots: { t: number; png: Buffer }[] = [];
  const marks: Record<string, number> = {};
  const start = performance.now();
  let running = true;
  const grabber = (async () => {
    while (running) {
      const t = performance.now() - start;
      const { data } = await cdp.send("Page.captureScreenshot", { format: "png", optimizeForSpeed: true });
      shots.push({ t, png: Buffer.from(data, "base64") });
    }
  })();
  await script((label) => (marks[label] = Math.round(((performance.now() - start) / 1000) * FPS)));
  running = false;
  await grabber;
  const total = Math.floor((shots.at(-1)!.t / 1000) * FPS);
  let j = 0;
  for (let f = 0; f <= total; f++) {
    while (j + 1 < shots.length && shots[j + 1].t <= (f / FPS) * 1000) j++;
    await writeFile(join(dir, `${String(f).padStart(4, "0")}.png`), shots[j].png);
  }
  const meta = { frames: total + 1, marks, width: 0, height: 0, grabbed: shots.length };
  const size = page.viewportSize()!;
  meta.width = size.width;
  meta.height = size.height;
  await writeFile(join(dir, "meta.json"), JSON.stringify(meta, null, 2));
  console.log(`${name}: ${meta.frames} frames from ${shots.length} grabs`, marks);
  return meta;
}

export async function still({ cdp }: { cdp: CDPSession }, name: string) {
  await mkdir(out, { recursive: true });
  const { data } = await cdp.send("Page.captureScreenshot", { format: "png" });
  await writeFile(join(out, `${name}.png`), Buffer.from(data, "base64"));
}
