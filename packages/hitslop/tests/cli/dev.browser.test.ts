import { test, expect } from "bun:test";
import { webkit } from "playwright";
import { mkdtemp, readFile, writeFile, rename, rm } from "node:fs/promises";
import { join } from "node:path";
import { startDev } from "../../src/cli/dev";
import { WidgetViewports } from "../../src/schema/constants";
import { createFixture } from "./dev-fixture";
import { overrideSlop, overrideSchema } from "./source-fixture";

test("device presets and viewport changes update Svelte and CSS without replacing accepted state", async () => {
  const root = await mkdtemp(join(process.cwd(), ".dev-test-"));
  const browser = await webkit.launch();
  let dev: Awaited<ReturnType<typeof startDev>> | undefined;
  try {
    const source = join(root, "source"); await createFixture(process.cwd(), source);
    dev = await startDev(source);
    const page = await browser.newPage(); await page.goto(dev.url);
    const ui = page.frameLocator("iframe");
    const env = ui.locator("[data-environment]");
    await env.filter({ hasText: "unknown/unknown/compact" }).waitFor();
    await ui.getByRole("button", { name: "Add row", exact: true }).click();
    await ui.locator("[data-count]").filter({ hasText: "4" }).waitFor();
    const ids = await ui.locator("[data-row-ids]").textContent();
    const preset = page.getByRole("combobox", { name: "Device preset" });
    await preset.selectOption("iphone");
    await env.filter({ hasText: "ios/phone/compact" }).waitFor();
    const mobileFrame = page.frames().find(frame => frame.url().includes("/app.html"))!;
    // Mobile reflow can move the focused field outside a nested scroller.
    // Use a minimal nested scroller, independent of any shipped template.
    await mobileFrame.evaluate(async () => {
      const pane = document.createElement("section");
      pane.id = "mobile-editing-probe";
      pane.style.cssText = "position:fixed;inset:0;overflow:auto;overflow-anchor:none;background:white";
      const space = document.createElement("div");
      space.style.height = "20px";
      const input = document.createElement("input");
      input.setAttribute("aria-label", "Mobile focus probe");
      pane.append(space, input);
      document.body.append(pane);
      input.focus({ preventScroll: true });
      await new Promise(requestAnimationFrame);
      await new Promise(requestAnimationFrame);
      space.style.height = "1200px";
      window.dispatchEvent(new Event("resize"));
    });
    await mobileFrame.waitForFunction(() => {
      const input = document.querySelector<HTMLInputElement>('[aria-label="Mobile focus probe"]')!;
      const rect = input.getBoundingClientRect();
      return document.activeElement === input && rect.top >= 0 && rect.bottom <= innerHeight;
    }, undefined, { timeout: 3000 });
    await mobileFrame.evaluate(() => document.getElementById("mobile-editing-probe")!.remove());
    await preset.selectOption("ipad");
    await env.filter({ hasText: "ios/tablet/regular" }).waitFor();
    await preset.selectOption("mac");
    await env.filter({ hasText: "macos/desktop/compact" }).waitFor();
    expect(await env.evaluate(element => getComputedStyle(element).color)).toBe("rgb(11, 22, 33)");
    for (const width of [640, 639]) {
      await page.locator(".stage").evaluate((element, width) => (element as HTMLElement).style.setProperty("--editor-width", `${width}px`), width);
      await env.filter({ hasText: `macos/desktop/${width < 640 ? "compact" : "regular"}` }).waitFor();
    }
    expect(await ui.locator("[data-row-ids]").textContent()).toBe(ids);
    const app = page.frames().find(frame => frame.url().includes("/app.html"))!;
    // Wrong origin and wrong source cannot simulate a device.
    for (const [origin, source] of [["https://example.com", true], [new URL(dev.url).origin, false]] as const) {
      await app.evaluate(([origin, fromParent]) => dispatchEvent(new MessageEvent("message", {
        origin, source: fromParent ? parent : window, data: { type: "slop:environment", preset: "iphone" },
      })), [origin, source] as const);
    }
    expect(await env.textContent()).toBe("macos/desktop/compact");
    const composition = page.getByRole("combobox", { name: "Preview composition" });
    await composition.selectOption("icon");
    await ui.locator('svg[aria-label="Fixture icon"]').waitFor();
    expect(await ui.locator("html").getAttribute("data-slop-platform")).toBeNull();
    expect(await ui.locator("html").getAttribute("data-slop-device")).toBeNull();
    expect(await ui.locator("html").getAttribute("data-slop-layout")).toBeNull();
    await composition.selectOption("editor");
    await env.filter({ hasText: "macos/desktop/compact" }).waitFor();
    expect(await ui.locator("[data-count]").textContent()).toBe("4");
    expect(await ui.locator("[data-row-ids]").textContent()).toBe(ids);
    // A frame reload receives the selected simulated identity before editor mount.
    await app.goto(app.url());
    await env.filter({ hasText: "macos/desktop/compact" }).waitFor();
  } finally { await browser.close(); await dev?.close(); await rm(root, { recursive: true, force: true }); }
}, 60000);

/** A schema dependency supplies three tasks under `title`. */
const seed = (title: string) =>
  `export default ${JSON.stringify({ title, tasks: ["One", "Two", "Three"].map((text) => ({ text, done: false })) })};\n`;

test("HMR keeps one owner, accepted edits and row identity; definition resets and recovers", async () => {
  const root = await mkdtemp(join(process.cwd(), ".dev-test-"));
  let dev: Awaited<ReturnType<typeof startDev>> | undefined;
  const browser = await webkit.launch();
  try {
    const source = join(root, "source");
    await createFixture(process.cwd(), source);
    const commands = await readFile(join(source, "commands.ts"), "utf8");
    await rm(join(source, "commands.ts"));
    // A schema dependency change rebuilds the disposable document.
    await writeFile(join(source, "seed.ts"), seed("A little room to think"));
    await overrideSchema(source, "seed", 'import seed from "./seed";');
    dev = await startDev(source);
    const page = await browser.newPage();
    page.setDefaultTimeout(15000);
    await page.goto(dev.url);
    const frame = page.frameLocator("iframe");
    await frame.getByRole("heading", { name: "Revision zero" }).waitFor();
    await frame.getByRole("button", { name: "Add row", exact: true }).click();
    await frame.locator("[data-count]").filter({ hasText: "4" }).waitFor();
    const rows = await frame.locator("[data-row-ids]").textContent();
    await frame.getByRole("textbox", { name: "Document title" }).fill("Accepted title");
    await frame.locator("[data-title]").filter({ hasText: "Accepted title" }).waitFor();
    const app = await readFile(join(source, "App.svelte"), "utf8");
    await writeFile(join(source, "App.svelte"), app.replace("Revision zero", "Hot revision"));
    await frame.getByRole("heading", { name: "Hot revision" }).waitFor();
    expect(await frame.locator("[data-row-ids]").textContent()).toBe(rows);
    expect(await frame.locator("[data-title]").textContent()).toBe("Accepted title");
    expect(await frame.locator("[data-probe]").count()).toBe(1);
    await writeFile(
      join(source, "App.svelte"),
      app.replace("rgb(20, 40, 60)", "rgb(80, 100, 120)"),
    );
    await page
      .frames()
      .find((frame) => frame.url().includes("/app.html"))!
      .waitForFunction(
        () => getComputedStyle(document.querySelector("h1")!).color === "rgb(80, 100, 120)",
      );
    await writeFile(join(source, "App.svelte"), "<script>let = ;</script>");
    await frame.locator("vite-error-overlay").waitFor();
    await writeFile(join(source, "App.svelte"), app.replace("Revision zero", "Recovered"));
    await frame.getByRole("heading", { name: "Recovered" }).waitFor();
    expect(await frame.locator("[data-count]").textContent()).toBe("4");
    await writeFile(join(source, "seed.ts"), "export default {title: 42};");
    await frame.locator("vite-error-overlay").waitFor();
    await writeFile(join(source, "seed.ts"), seed("Reset seed"));
    await frame.locator("[data-title]").filter({ hasText: "Reset seed" }).waitFor();
    expect(await frame.locator("[data-count]").textContent()).toBe("3");
    expect(await frame.locator("[data-probe]").count()).toBe(1);
    // Imported CSS updates without replacing the owner.
    await writeFile(join(source, "styles.css"), "body { --entry-probe: ready; }");
    await page.frames().find((frame) => frame.url().includes("/app.html"))!
      .waitForFunction(() => getComputedStyle(document.body).getPropertyValue("--entry-probe").trim() === "ready");
    expect(await frame.locator("[data-probe]").count()).toBe(1);
    const response = await fetch(new URL("/__app__/@fs/etc/passwd", dev.url));
    expect(response.status).toBe(403);
    // Every inventory change must reset accepted state, not just leave the old page visible.
    const roleChange = async (change: () => Promise<unknown>) => {
      await frame.getByRole("textbox", { name: "Document title" }).fill("Before role change");
      await frame.locator("[data-title]").filter({ hasText: "Before role change" }).waitFor();
      await change();
      await frame.locator("[data-title]").filter({ hasText: "Reset seed" }).waitFor();
      expect(await frame.locator("[data-probe]").count()).toBe(1);
    };
    // A required capture role disappearing fences the preview until restored.
    await rm(join(source, "Preview.svelte"));
    await frame.locator("vite-error-overlay").waitFor();
    await writeFile(join(source, "Preview.svelte"), "<p>Capture role</p>");
    await frame.locator("vite-error-overlay").waitFor({ state: "detached" });
    await frame.locator("[data-title]").filter({ hasText: "Reset seed" }).waitFor();
    await rename(join(source, "Preview.svelte"), join(source, "preview.tmp"));
    await rename(join(source, "preview.tmp"), join(source, "preview.svelte"));
    await frame.locator("vite-error-overlay").waitFor();
    await rename(join(source, "preview.svelte"), join(source, "preview.tmp"));
    await rename(join(source, "preview.tmp"), join(source, "Preview.svelte"));
    await frame.locator("vite-error-overlay").waitFor({ state: "detached" });
    await frame.getByRole("heading", { name: "Recovered" }).waitFor();
    // commands.ts was absent at startup; adding, fixing and removing it must all rebuild.
    await roleChange(() => writeFile(join(source, "commands.ts"), commands));
    await writeFile(join(source, "commands.ts"), "export default undefined;");
    await frame.locator("vite-error-overlay").waitFor();
    await writeFile(join(source, "commands.ts"), commands);
    await frame.locator("vite-error-overlay").waitFor({ state: "detached" });
    await frame.getByRole("heading", { name: "Recovered" }).waitFor();
    await roleChange(() => rm(join(source, "commands.ts")));

  } finally {
    await browser.close();
    await dev?.close();
    await rm(root, { recursive: true, force: true });
  }
}, 120000);

test("cancelling startup stops a definition build without opening a preview", async () => {
  const root=await mkdtemp(join(process.cwd(),".dev-test-"));
  const controller=new AbortController();
  try {
    const source=join(root,"source"); await createFixture(process.cwd(),source);
    await overrideSlop(source,{},"for (;;) {}");
    const opening=startDev(source,0,controller.signal);
    const result=opening.then(server => { void server.close(); return "opened"; },()=>"cancelled");
    await Bun.sleep(50);controller.abort();
    expect(await result).toBe("cancelled");
  } finally {controller.abort();await rm(root,{recursive:true,force:true});}
},10000);

// The browser host must exercise native storage, preserve URL isolation and stop editing
// when its owner dies. A new page gets a new disposable document, never the dead session.
test("native preview serves sniffed attachments and fences disconnected owners", async () => {
  const root = await mkdtemp(join(process.cwd(), ".dev-test-"));
  const browser = await webkit.launch();
  let dev: Awaited<ReturnType<typeof startDev>> | undefined;
  try {
    const source = join(root, "source"); await createFixture(process.cwd(), source);
    dev = await startDev(source);
    const page = await browser.newPage(); await page.goto(dev.url);
    const ui = page.frameLocator("iframe");
    await ui.getByRole("heading", { name: "Revision zero" }).waitFor();
    const frame = page.frames().find(f => f.url().includes("/app.html"))!;
    const benchmark = process.env.HITSLOP_BENCH_EDITS === "1";
    const iterations = benchmark ? 40 : 1;
    const result = await frame.evaluate(async iterations => {
      const { doc, addRow, attachments } = (globalThis as any).__devProbe;
      const times: number[] = [];
      for (let i = 0; i < iterations; i++) {
        const start = performance.now(); await addRow({ text: "Latency probe " + i });
        times.push(performance.now() - start);
      }
      const before = doc.current.tasks.length;
      let invalid: { code?: string; reason?: string } | undefined;
      try { await addRow({ text: 3 }); } catch (e) {
        const error = e as { code?: string; reason?: string };
        invalid = { code: error.code, reason: error.reason };
      }
      const ref = await attachments.import(new File(["imported media"], "fake.png", { type: "image/png" }),
        (tx: any, ref: any) => tx.fields.title.set(ref.id));
      const url = attachments.url(ref.id);
      const whole = await fetch(url), range = await fetch(url, { headers: { Range: "bytes=0-3" } });
      const head = await fetch(url, { method: "HEAD", headers: { Range: "bytes=-5" } });
      const missing = await fetch(url.replace(ref.id, "a".repeat(64)));
      return { times, before, after: doc.current.tasks.length, invalid, url,
        type: whole.headers.get("content-type"), sandbox: whole.headers.get("content-security-policy"),
        nosniff: whole.headers.get("x-content-type-options"), bytes: await whole.text(),
        rangeStatus: range.status, range: await range.text(), missing: missing.status,
        head: [head.status, head.headers.get("content-length"), await head.text()] };
    }, iterations);
    expect(result.before).toBe(3 + iterations);
    expect(result.before).toBe(result.after);
    expect(result.invalid).toEqual({ code: "rejected", reason: "type_mismatch" });
    expect(result.type).toBe("application/octet-stream");
    expect(result.sandbox).toBe("sandbox"); expect(result.nosniff).toBe("nosniff");
    expect(result.bytes).toBe("imported media"); expect(result.rangeStatus).toBe(206); expect(result.range).toBe("impo");
    expect(result.missing).toBe(404);
    expect(result.head).toEqual([206, "5", ""]);
    const sorted = [...result.times].sort((a, b) => a - b);
    if (benchmark) console.log(`Native preview edit latency (${sorted.length} fresh evaluations): p50=${sorted[Math.floor(sorted.length * .5)]}ms p95=${sorted[Math.ceil(sorted.length * .95)-1]}ms`);
    const [owner] = await dev.diagnostics(); expect(owner).toBeDefined();
    process.kill(owner!.pid, "SIGKILL");
    await ui.locator("#hitslop-preview-failure").waitFor();
    const refused = await frame.evaluate(async () => {
      try { await (globalThis as any).__devProbe.addRow({ text: "must not apply" }); return "accepted"; }
      catch { return "refused"; }
    });
    expect(refused).toBe("refused");
    expect((await fetch(result.url)).ok).toBe(false);
    await page.reload();
    await ui.getByRole("heading", { name: "Revision zero" }).waitFor();
    expect(await ui.locator("[data-count]").textContent()).toBe("3");
  } finally { await browser.close(); await dev?.close(); await rm(root, {recursive:true,force:true}); }
}, 60000);


// Each view is the browser layout of one role: only it is mounted, from the live document, on a
// surface the page sizes to its canvas. Selections come only from that page, and survive updates.
test("views switch between the editor and static layouts, keep edits and survive updates", async () => {
  const root = await mkdtemp(join(process.cwd(), ".dev-test-"));
  const browser = await webkit.launch();
  let dev: Awaited<ReturnType<typeof startDev>> | undefined;
  try {
    const source = join(root, "source");
    await createFixture(process.cwd(), source);
    const widget = `<script lang="ts">
      import type {CaptureProps, WidgetProps} from 'hitslop/svelte';
      import type schema from './schema';
      let {snapshot, family, width, height}: CaptureProps<typeof schema> & WidgetProps = $props();
    </script><section data-widget style="width:100%;height:100%">Original {family} {width}×{height}: {snapshot.title}</section>`;
    await writeFile(join(source, "Widget.svelte"), widget);
    // A viewport-high preview is measured at the 480×620 window, as production lays it out.
    await writeFile(join(source, "Preview.svelte"), `<script lang="ts">
      import type {CaptureProps} from 'hitslop/svelte';
      import type schema from './schema';
      let {snapshot}: CaptureProps<typeof schema> = $props();
    </script><article data-export style="min-height:100vh"><h1>{snapshot.title}</h1></article><p style="margin:0;height:40px">Note</p>`);
    dev = await startDev(source);
    const page = await browser.newPage();
    page.setDefaultTimeout(15000);
    await page.goto(dev.url);
    const frame = page.frameLocator("iframe");
    const app = () => page.frames().find((frame) => frame.url().includes("/app.html"))!;
    const surface = (width: number, height?: number) =>
      page.waitForFunction(([width, height]) => {
        const box = document.querySelector("iframe")!.getBoundingClientRect();
        return box.width === width && (height === undefined || box.height === height);
      }, [width, height] as const);
    const mounted = () => app().evaluate(() => ({
      capture: document.documentElement.dataset.slopCapture ?? null,
      editor: document.querySelectorAll("[data-slop-root]").length,
      preview: document.querySelectorAll('[data-slop-capture-target="preview"] [data-export]').length,
      icon: document.querySelectorAll('[data-slop-capture-target="icon"] svg').length,
      widget: document.querySelectorAll("[data-widget]").length,
    }));
    const none = { editor: 0, preview: 0, icon: 0, widget: 0 };
    await frame.getByRole("textbox", {name: "Document title"}).fill("Saved preview edit");
    await frame.locator("[data-title]").filter({hasText: "Saved preview edit"}).waitFor();
    await page.getByText("slop build shows the final image").waitFor();
    const picker = page.getByRole("combobox", {name: "Preview composition"});
    await picker.selectOption("preview");
    await frame.locator('[data-slop-capture-target="preview"] h1').filter({hasText: "Saved preview edit"}).waitFor();
    await surface(480, 620 + 40);
    expect(await mounted()).toEqual({ ...none, capture: "static", preview: 1 });
    await picker.selectOption("icon");
    await frame.locator('svg[aria-label="Fixture icon"]').waitFor();
    await surface(512, 512);
    expect(await mounted()).toEqual({ ...none, capture: "icon", icon: 1 });
    for (const { family, width, height } of WidgetViewports) {
      await picker.selectOption(family);
      await frame.locator("[data-widget]").filter({hasText: `${family} ${width}×${height}: Saved preview edit`}).waitFor();
      await surface(width, height);
    }
    expect(await mounted()).toEqual({ ...none, capture: "widget", widget: 1 });
    // Another origin, or another window at this origin, cannot select a view; the page can.
    const select = (origin: string, fromParent: boolean) =>
      app().evaluate(async ([origin, fromParent]) => {
        const data = { type: "slop:view", view: "icon" };
        dispatchEvent(new MessageEvent("message", { data, origin, source: fromParent ? parent : window }));
        await new Promise((resolve) => setTimeout(resolve, 250));
      }, [origin, fromParent] as const);
    await select("http://example.com", true);
    await select(new URL(dev.url).origin, false);
    expect((await mounted()).widget).toBe(1);
    await select(new URL(dev.url).origin, true);
    expect(await mounted()).toEqual({ ...none, capture: "icon", icon: 1 });
    // Component updates keep the selected view.
    await picker.selectOption("large");
    await writeFile(join(source, "Widget.svelte"), widget.replace("Original", "Updated"));
    await frame.locator("[data-widget]").filter({hasText: "Updated large"}).waitFor();
    await writeFile(join(source, "Widget.svelte"), widget.replace('width:100%;height:100%', 'width:900px;height:900px'));
    await page.getByRole("status").filter({hasText: "will be clipped"}).waitFor();
    await picker.selectOption("editor");
    await frame.getByRole("textbox", {name: "Document title"}).waitFor();
    expect(await frame.getByRole("textbox", {name: "Document title"}).inputValue()).toBe("Saved preview edit");
    await surface(480, 620);
    expect(await mounted()).toEqual({ ...none, capture: null, editor: 1 });
    // A definition change reloads the frame with a fresh document; the page selects again.
    await picker.selectOption("small");
    await writeFile(join(source, "commands.ts"), (await readFile(join(source, "commands.ts"), "utf8")) + "\n");
    await frame.locator("[data-widget]").filter({hasText: "small"}).filter({hasText: "Fixture title"}).waitFor();
    await rm(join(source, "Widget.svelte"));
    await page.locator("optgroup[disabled]").waitFor({state: "attached"});
    await frame.getByRole("textbox", {name: "Document title"}).waitFor();
    expect(await picker.inputValue()).toBe("editor");
    await writeFile(join(source, "Widget.svelte"), widget);
    await page.locator("optgroup:not([disabled])").waitFor({state: "attached"});
    await picker.selectOption("small");
    await frame.locator("[data-widget]").filter({hasText: "small"}).waitFor();
  } finally { await browser.close(); await dev?.close(); await rm(root, {recursive: true, force: true}); }
}, 120000);
