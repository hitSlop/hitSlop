import { test, expect } from "bun:test";
import { webkit } from "playwright";
import { mkdtemp, readFile, writeFile, rm } from "node:fs/promises";
import { join } from "node:path";
import { startDev } from "../../src/cli/dev";
import { createFixture } from "./dev-fixture";
import { overrideSlop } from "./source-fixture";

/** A module slop.ts takes its initial values from: three tasks under `title`. */
const seed = (title: string) =>
  `export default ${JSON.stringify({ title, tasks: ["One", "Two", "Three"].map((text) => ({ text, done: false })) })};\n`;

test("HMR keeps one owner, accepted edits and row identity; metadata resets and recovers", async () => {
  const root = await mkdtemp(join(process.cwd(), ".dev-test-"));
  let dev: Awaited<ReturnType<typeof startDev>> | undefined;
  const browser = await webkit.launch();
  try {
    const source = join(root, "source");
    await createFixture(process.cwd(), source);
    // Metadata reloads when a module slop.ts imports changes.
    await writeFile(join(source, "seed.ts"), seed("A little room to think"));
    await overrideSlop(source, { initial: "seed" }, 'import seed from "./seed";');
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
    // slop.ts is build-only: the preview never serves it as an app module.
    const metadata = await fetch(new URL("/slop.ts", dev.url));
    expect(metadata.ok).toBe(false);
  } finally {
    await browser.close();
    await dev?.close();
    await rm(root, { recursive: true, force: true });
  }
}, 90000);

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
    const result = await frame.evaluate(async () => {
      const { doc, addTask, attachments } = (globalThis as any).__devProbe;
      const times: number[] = [];
      for (let i = 0; i < 40; i++) {
        const start = performance.now(); await addTask({ text: "Latency probe " + i });
        times.push(performance.now() - start);
      }
      const before = doc.current.tasks.length;
      let invalid = "";
      try { await addTask({ text: 3 }); } catch (e) { invalid = String(e); }
      const ref = await attachments.import(new File(["imported media"], "fake.png", { type: "image/png" }),
        (tx: any, ref: any) => tx.fields.title.set(ref.id));
      const url = attachments.url(ref.id);
      const whole = await fetch(url), range = await fetch(url, { headers: { Range: "bytes=0-3" } });
      const missing = await fetch(url.replace(ref.id, "a".repeat(64)));
      return { times, before, after: doc.current.tasks.length, invalid, url,
        type: whole.headers.get("content-type"), sandbox: whole.headers.get("content-security-policy"),
        nosniff: whole.headers.get("x-content-type-options"), bytes: await whole.text(),
        rangeStatus: range.status, range: await range.text(), missing: missing.status };
    });
    expect(result.before).toBe(result.after);
    expect(result.invalid).toContain("Invalid arguments");
    expect(result.type).toBe("application/octet-stream");
    expect(result.sandbox).toBe("sandbox"); expect(result.nosniff).toBe("nosniff");
    expect(result.bytes).toBe("imported media"); expect(result.rangeStatus).toBe(206); expect(result.range).toBe("impo");
    expect(result.missing).toBe(404);
    const sorted = result.times.toSorted((a, b) => a - b);
    console.log(`Native preview command latency (${sorted.length} fresh evaluations): p50=${sorted[Math.floor(sorted.length * .5)]}ms p95=${sorted[Math.ceil(sorted.length * .95)-1]}ms`);
    const [owner] = await dev.diagnostics(); expect(owner).toBeDefined();
    process.kill(owner!.pid, "SIGKILL");
    await ui.locator("#hitslop-preview-failure").waitFor();
    const refused = await frame.evaluate(async () => {
      try { await (globalThis as any).__devProbe.addTask({ text: "must not apply" }); return "accepted"; }
      catch { return "refused"; }
    });
    expect(refused).toBe("refused");
    expect((await fetch(result.url)).ok).toBe(false);
    await page.reload();
    await ui.getByRole("heading", { name: "Revision zero" }).waitFor();
    expect(await ui.locator("[data-count]").textContent()).toBe("3");
  } finally { await browser.close(); await dev?.close(); await rm(root, {recursive:true,force:true}); }
}, 60000);
