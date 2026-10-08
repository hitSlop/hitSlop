// Measures Quick Checklist edits in real WebKit (Playwright) against the dev preview.
import { webkit, chromium } from "playwright";
const url = process.env.PREVIEW_URL ?? "http://127.0.0.1:5199/app.html";
const rows = Number(process.env.BENCH_ROWS ?? 5000);
const engine = process.env.ENGINE === "chromium" ? chromium : webkit;
const browser = await engine.launch();
const page = await browser.newPage({ viewport: { width: 480, height: 620 } });
const t0 = Date.now();
await page.goto(url);
await page.waitForFunction((n) => document.querySelectorAll(".checklist-row").length >= n, rows, { timeout: 120_000 });
await page.evaluate(() => new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r))));
const startup = Date.now() - t0;

const result = await page.evaluate(async (kind) => {
  const frame = () => new Promise((r) => requestAnimationFrame(() => setTimeout(r, 0)));
  const out = { js: [] as number[], layout: [] as number[], frame: [] as number[] };
  for (let i = 0; i < 20; i++) {
    await frame(); await frame();
    const row = document.querySelectorAll(".checklist-row")[2] as HTMLElement;
    const changed = new Promise<number>((resolve) => {
      const observer = new MutationObserver(() => { observer.disconnect(); resolve(performance.now()); });
      observer.observe(row, { attributes: true, subtree: true, childList: true, characterData: true });
    });
    const start = performance.now();
    (row.querySelector("input[type=checkbox]") as HTMLInputElement).click();
    const mutated = await changed;
    // Let the rest of the framework flush (microtasks), then force layout.
    await Promise.resolve(); await Promise.resolve();
    const js = performance.now();
    void document.documentElement.offsetHeight;
    const laidOut = performance.now();
    await frame();
    out.js.push(js - start);
    out.layout.push(laidOut - js);
    out.frame.push(performance.now() - laidOut);
    void mutated;
  }
  const med = (a: number[]) => a.sort((x, y) => x - y)[a.length >> 1];
  return { jsMS: med(out.js), forcedLayoutMS: med(out.layout), restOfFrameMS: med(out.frame) };
}, "checkbox");
const geometry = await page.evaluate(() => { const s = document.querySelector(".checklist-scroller") as HTMLElement; return { scrollerClient: s?.clientHeight, scrollerContent: s?.scrollHeight, rowHeight: (document.querySelector(".checklist-row") as HTMLElement)?.getBoundingClientRect().height }; });
console.log(JSON.stringify({ engine: engine.name(), rows, startupMS: startup, ...result, ...geometry }));
await browser.close();
