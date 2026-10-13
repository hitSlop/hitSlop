import { afterAll, beforeAll, expect, test } from "bun:test";
import { chromium, type BrowserContext, type Frame, type Page, type Worker as PageWorker } from "playwright";
import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { startBrowserHost } from "../../packages/hitslop/src/cli/browser";
import { createFixture } from "../../packages/hitslop/tests/cli/dev-fixture";
import { buildTemplate } from "../../packages/hitslop/src/cli/template";
import { execute } from "../../packages/hitslop/src/cli/engine";
import { releases, documents } from "../../scripts/compat/corpus";
import { BrowserHost } from "../../packages/hitslop/src/schema/constants";
import { copyDeadline } from "../../packages/hitslop/src/browser/requests";
import { Database } from "bun:sqlite";

function documentIdentity(file: string) {
  const db = new Database(file, { readonly: true });
  try { return (db.query("SELECT uuid FROM document").get() as { uuid: string }).uuid; }
  finally { db.close(); }
}

function documentContents(file: string) {
  const db = new Database(file, { readonly: true });
  try {
    const tables = db.query("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name").all() as { name: string }[];
    return tables.map(({ name }) => ({ name, rows: db.query(`SELECT ${name === "document" ? "id" : "*"} FROM "${name}" ORDER BY rowid`).all() }));
  } finally { db.close(); }
}
async function saveCopy(page: Page, output: string) {
  await page.evaluate(async () => {
    const root = await navigator.storage.getDirectory();
    const handle = await root.getFileHandle(`test-save-${crypto.randomUUID()}`, { create: true });
    (globalThis as any).__saveTarget = handle;
    (window as any).showSaveFilePicker = async () => handle;
  });
  await page.getByRole("button", { name: "Download .slop", exact: true }).click();
  await page.waitForFunction(() => !document.querySelector<HTMLButtonElement>("#download")?.disabled);
  const encoded = await page.evaluate(async () => {
    const file = await (globalThis as any).__saveTarget.getFile();
    return await new Promise<string>((resolve, reject) => {
      const reader = new FileReader(); reader.onload = () => resolve(String(reader.result).split(",")[1]!); reader.onerror = () => reject(reader.error); reader.readAsDataURL(file);
    });
  });
  await Bun.write(output, Buffer.from(encoded, "base64"));
}

let directory: string;
let host: Awaited<ReturnType<typeof startBrowserHost>>;
let snapshot: string;
const origin = `http://127.0.0.1:${BrowserHost.port}`;
beforeAll(async () => {
  directory = await mkdtemp(join(process.cwd(), ".browser-host-test-"));
  const source = join(directory, "source");
  await createFixture(process.cwd(), source);
  const declaration = join(source, "slop.ts");
  await writeFile(declaration, (await readFile(declaration, "utf8")).replace('window: { kind: "standard",', 'window: { kind: "standard", fullscreenable: true,'));
  const template = await buildTemplate(source, join(directory, "fixture.slop"), { renderArtwork: false });
  const original = join(directory, "original.slop");
  await execute({ method: "create", from: template, output: original });
  await execute({ method: "batch", documentPath: original, batch: { intents: [{ type: "setTheme", values: { accent: "#123456" } }] } });
  snapshot = join(directory, "snapshot.slop");
  await execute({ method: "copy", documentPath: original, output: snapshot });
  host = await startBrowserHost();
}, 120_000);
afterAll(async () => { await host?.close(); if (directory) await rm(directory, { recursive: true, force: true }); });
async function imported(file = snapshot) {
  const bytes = Bun.file(file);
  const result = await fetch(`${origin}/__host/import`, { method: "POST", headers: { Authorization: `Bearer ${host.discovery.token}`, "Content-Length": String(bytes.size) }, body: bytes });
  expect(result.status).toBe(200);
  const { source } = await result.json() as { source: string };
  return `${origin}/?copy=${crypto.randomUUID()}#import=${source}`;
}

for (const [name, engine] of [["Chrome", chromium]] as const) {
  test(`${name}: real WASM edits, OPFS reload, tab lock, focused text download and native reopen`, async () => {
    const profile = await mkdtemp(join(directory, "chrome-profile-"));
    const context = await engine.launchPersistentContext(profile, { channel: "chrome", acceptDownloads: true });
    try {
      const page = await context.newPage();
      const errors: string[] = [];
      page.on("pageerror", error => errors.push(error.message));
      await page.goto(await imported());
      const app = page.frameLocator("iframe");
      await app.getByRole("button", { name: "Add row", exact: true }).waitFor({ timeout: 30_000 }).catch(async error => { throw new Error(`${await page.locator("#status").textContent()}; ${errors.join("; ")}`, { cause: error }); });
      const frame = page.frames().find(frame => frame.url().includes(".localhost:"))!;
      expect(await page.getByRole("button", { name: "Keep awake: off" }).isVisible()).toBe(true);
      const oldIDs = await app.locator("[data-row-ids]").textContent();
      await app.getByRole("button", { name: "Add row", exact: true }).click();
      await frame.waitForFunction(() => document.querySelector("[data-count]")?.textContent === "4", undefined, { timeout: 30_000 }).catch(async error => { throw new Error(`Command: ${await app.locator("[data-error]").textContent()}; status: ${await page.locator("#status").textContent()}`, { cause: error }); });
      expect(await app.locator("[data-error]").textContent()).toBe("");
      const attachment = await frame.evaluate(async () => {
        const { doc, attachments } = (globalThis as any).__devProbe;
        const ref = await attachments.import(new File(["portable attachment"], "note.txt"),
          (tx: any, ref: any) => tx.fields.tasks.insert({ text: ref.id }));
        await doc.flush();
        const response = await fetch(attachments.url(ref.id), { headers: { Range: "bytes=0-7" } });
        const suffix = await fetch(attachments.url(ref.id), { headers: { Range: "bytes=-10" } });
        const head = await fetch(attachments.url(ref.id), { method: "HEAD", headers: { Range: "bytes=0-7" } });
        return { id: ref.id, range: await response.text(), status: response.status, sandbox: response.headers.get("Content-Security-Policy"),
          suffix: await suffix.text(), head: [head.status, head.headers.get("Content-Length"), await head.text()] };

      });
      expect(attachment.suffix).toBe("attachment"); expect(attachment.head).toEqual([206, "8", ""]);
      expect(attachment.range).toBe("portable"); expect(attachment.status).toBe(206); expect(attachment.sandbox).toBe("sandbox");
      const cdp = await context.newCDPSession(page);
      await cdp.send("ServiceWorker.enable"); await cdp.send("ServiceWorker.stopAllWorkers");
      expect(await frame.evaluate(async id => {
        const { attachments } = (globalThis as any).__devProbe;
        return await (await fetch(attachments.url(id))).text();
      }, attachment.id)).toBe("portable attachment");
      await cdp.detach();
      const ids = await app.locator("[data-row-ids]").textContent();
      expect(ids!.startsWith(oldIDs!)).toBe(true);
      // Private UI state is not persisted; IDs and accepted document data are.
      await app.getByRole("button", { name: "Local 0", exact: true }).click();
      await page.reload();
      await app.getByRole("button", { name: "Local 0", exact: true }).waitFor({ timeout: 30_000 });
      expect(await app.locator("[data-row-ids]").textContent()).toBe(ids);
      const second = await context.newPage();
      await second.goto(page.url());
      await second.waitForFunction(() => document.querySelector("#status")?.textContent?.includes("another tab"));
      await second.close();
      // Download without blurring the field: the host must call the page's drain.
      await app.getByRole("textbox", { name: "Document title" }).fill("Typed just before download");
      const file = join(directory, `${name}-edited.slop`);
      await saveCopy(page, file);
      const reopened = await execute({ method: "get", documentPath: file });
      expect((reopened.state as any).value.title).toBe("Typed just before download");
      expect((reopened.state as any).value.tasks.map((row: any) => row.$id).join(",")).toBe(ids);
      expect((reopened.state as any).theme.accent).toBe("#123456");
      const media = await execute({ method: "attachments.read", documentPath: file, attachmentID: attachment.id });
      expect(Buffer.from(media.state.bytes, "base64").toString()).toBe("portable attachment");
      await page.getByRole("button", { name: "Fullscreen", exact: true }).click();
      expect(await page.evaluate(() => document.fullscreenElement?.id)).toBe("stage");
      await page.evaluate(() => document.exitFullscreen());
      // Kill the owning worker after a confirmed save, then reopen the same pool.
      const url = page.url();
      await page.workers().find(worker => worker.url().endsWith("/worker.js"))!.evaluate(() => close());
      await page.close();
      const recovered = await context.newPage();
      await recovered.goto(url);
      await recovered.frameLocator("iframe").getByRole("textbox", { name: "Document title" }).waitFor();
      expect(await recovered.frameLocator("iframe").getByRole("textbox").inputValue()).toBe("Typed just before download");
      await recovered.frameLocator("iframe").getByRole("textbox").fill("Typed before returning to copies");
      await recovered.getByRole("link", { name: "hitSlop", exact: true }).click();
      await recovered.getByRole("heading", { name: "Browser copies", exact: true }).waitFor();
      await recovered.goto(url);
      await recovered.frameLocator("iframe").getByRole("textbox").waitFor();
      expect(await recovered.frameLocator("iframe").getByRole("textbox").inputValue()).toBe("Typed before returning to copies");
      expect(errors).toEqual([]);
    } finally { await context.close(); }
  }, 90_000);

  test(`${name}: independent imports have isolated storage and unedited exports preserve document contents`, async () => {
    const profile = await mkdtemp(join(directory, "chrome-profile-"));
    const context = await engine.launchPersistentContext(profile, { channel: "chrome", acceptDownloads: true });
    try {
      const first = await context.newPage(); const second = await context.newPage();
      await first.goto(await imported()); await second.goto(await imported());
      await first.frameLocator("iframe").getByRole("textbox").waitFor();
      await second.frameLocator("iframe").getByRole("textbox").waitFor();
      const a = first.frames().find(frame => frame.url().includes(".localhost:"))!;
      const b = second.frames().find(frame => frame.url().includes(".localhost:"))!;
      expect(new URL(a.url()).origin).not.toBe(new URL(b.url()).origin);
      await a.evaluate(() => localStorage.setItem("isolation-probe", "first"));
      expect(await b.evaluate(() => localStorage.getItem("isolation-probe"))).toBeNull();
      const file = join(directory, `${name}-unchanged.slop`);
      await saveCopy(first, file);
      expect(documentIdentity(file)).not.toBe(documentIdentity(snapshot));
      expect(documentContents(file)).toEqual(documentContents(snapshot));
      const identity = documentIdentity(file);
      const other = join(directory, "other-import.slop"); await saveCopy(second, other);
      expect(documentIdentity(other)).not.toBe(identity);
      await first.reload(); await first.frameLocator("iframe").getByRole("textbox").waitFor();
      await saveCopy(first, file); expect(documentIdentity(file)).toBe(identity);
    } finally { await context.close(); }
  }, 60_000);
}

test("host rejects unauthenticated controls and arbitrary Host headers", async () => {
  expect((await fetch(`${origin}/__host/health`)).status).toBe(403);
  expect((await fetch(origin, { headers: { Host: "attacker.example" } })).status).toBe(403);
  expect((await fetch(`${origin}/__host/health`, { headers: { Authorization: `Bearer ${host.discovery.token}`, Origin: "https://attacker.example" } })).status).toBe(403);
});


test("Chrome refuses WAL and malformed snapshots without presenting a saved copy", async () => {
  const context = await chromium.launchPersistentContext(await mkdtemp(join(directory, "refusal-profile-")), { channel: "chrome" });
  try {
    for (const kind of ["wal", "malformed"]) {
      const bytes = new Uint8Array(await Bun.file(snapshot).arrayBuffer());
      if (kind === "wal") { bytes[18] = 2; bytes[19] = 2; } else bytes[0] = 0;
      const file = join(directory, `${kind}.slop`); await Bun.write(file, bytes);
      const page = await context.newPage(); await page.goto(await imported(file));
      await page.waitForFunction(() => document.querySelector("#status")?.textContent?.includes("rollback-journal"));
      expect(await page.getByRole("button", { name: "Download .slop" }).isDisabled()).toBe(true);
      expect(new Uint8Array(await Bun.file(file).arrayBuffer())).toEqual(bytes);
      await page.close();
    }
  } finally { await context.close(); }
}, 30_000);


test("Chrome keeps failed writes unsaved and retry persists the accepted edit", async () => {
  const context = await chromium.launchPersistentContext(await mkdtemp(join(directory, "failure-profile-")), { channel: "chrome" });
  try {
    const page = await context.newPage(); await page.goto(await imported());
    const field = page.frameLocator("iframe").getByRole("textbox", { name: "Document title" });
    await field.waitFor();
    const owner = page.workers().find(worker => worker.url().endsWith("/worker.js"))!;
    await owner.evaluate(() => {
      const proto = (globalThis as any).FileSystemSyncAccessHandle.prototype;
      (globalThis as any).__restoreWrite = proto.write;
      proto.write = () => { throw new DOMException("Simulated storage quota exhaustion", "QuotaExceededError"); };
    });
    await field.fill("Retain this failed save"); await field.press("Tab");
    await page.getByRole("button", { name: "Retry save" }).waitFor();
    expect(await page.locator("#status").textContent()).not.toBe("Saved in this browser");
    await owner.evaluate(() => { (globalThis as any).FileSystemSyncAccessHandle.prototype.write = (globalThis as any).__restoreWrite; });
    await page.getByRole("button", { name: "Retry save" }).click();
    await page.waitForFunction(() => document.querySelector("#status")?.textContent === "Saved in this browser", undefined, { timeout: 5000 }).catch(async error => { throw new Error(`Retry status: ${await page.locator("#status").textContent()}`, { cause: error }); });
    await page.reload(); await field.waitFor();
    expect(await field.inputValue()).toBe("Retain this failed save");
  } finally { await context.close(); }
}, 45_000);


// Artifact compatibility, independent of current example source or app workflows.
test("Chrome opens and exports every saved compatibility document without content changes", async () => {
  const context = await chromium.launchPersistentContext(await mkdtemp(join(directory, "corpus-profile-")), { channel: "chrome", acceptDownloads: true });
  try {
    for (const release of await releases()) for (const name of await documents(release.root)) {
      const original = join(release.root, "documents", `${name}.slop`);
      const page = await context.newPage();
      await page.goto(await imported(original));
      await page.waitForFunction(() => !document.querySelector<HTMLButtonElement>("#download")?.disabled);
      await page.frameLocator("iframe").locator("body").waitFor();
      const frame = page.frames().find(frame => frame.url().includes(".localhost:"))!;
      await frame.waitForFunction(() => typeof (globalThis as any).__slop?.flush === "function");
      const file = join(directory, "corpus-export.slop"); await saveCopy(page, file);
      expect(documentContents(file), `${release.name}/${name}`).toEqual(documentContents(original));
      expect(documentIdentity(file)).not.toBe(documentIdentity(original));
      await page.close();
    }
  } finally { await context.close(); }
}, 180_000);


test("Chrome imports snapshots across bounded ranges and downloads their complete SQLite bytes", async () => {
  const file = join(directory, "large.slop");
  await execute({ method: "copy", documentPath: snapshot, output: file });
  const bytes = new Uint8Array(10 * 1024 * 1024).fill(97);
  const id = new Bun.CryptoHasher("sha256").update(bytes).digest("hex");
  await execute({ method: "batch", documentPath: file, attachments: [Buffer.from(bytes).toString("base64")], batch: { intents: [{ type: "insert", path: ["tasks"], value: { text: id } }] } });
  const context = await chromium.launchPersistentContext(await mkdtemp(join(directory, "range-profile-")), { channel: "chrome", acceptDownloads: true });
  try {
    await context.addInitScript(() => {
      const original = MessagePort.prototype.postMessage;
      (globalThis as any).__resourceChunks = [];
      MessagePort.prototype.postMessage = function (data: any, ...args: any[]) {
        if (data?.type === "resource" && data.bytes) (globalThis as any).__resourceChunks.push(data.bytes.byteLength);
        return (original as any).call(this, data, ...args);
      };
    });
    const page = await context.newPage(); const ranges: string[] = [];
    context.on("request", request => { const range = request.headers()["range"]; if (request.url().includes("/__import/") && range) ranges.push(range); });
    await page.goto(await imported(file));
    await page.frameLocator("iframe").getByRole("textbox").waitFor();
    const frame = page.frames().find(frame => frame.url().includes(".localhost:"))!;
    const result = await frame.evaluate(async id => {
      const { attachments } = (globalThis as any).__devProbe;
      const url = attachments.url(id);
      const response = await fetch(url);
      const hash = await crypto.subtle.digest("SHA-256", await response.arrayBuffer());
      const seek = await fetch(url, { headers: { Range: "bytes=9437184-9437191" } });
      const invalid = await fetch(url, { headers: { Range: "bytes=999999999-" } });
      const reader = (await fetch(url)).body!.getReader(); await reader.read(); await reader.cancel();
      return { hash: [...new Uint8Array(hash)].map(b => b.toString(16).padStart(2, "0")).join(""), seek: await seek.text(), status: seek.status, invalid: invalid.status, chunks: (globalThis as any).__resourceChunks as number[] };
    }, id);
    expect(result.hash).toBe(id); expect(result.seek).toBe("aaaaaaaa"); expect(result.status).toBe(206); expect(result.invalid).toBe(416);
    expect(result.chunks.length).toBeGreaterThan(10);
    expect(Math.max(...result.chunks)).toBeLessThanOrEqual(BrowserHost.chunkBytes);
    const cdp = await context.newCDPSession(page); await cdp.send("ServiceWorker.enable"); await cdp.send("ServiceWorker.stopAllWorkers");
    expect(await frame.evaluate(async id => (await fetch((globalThis as any).__devProbe.attachments.url(id), { headers: { Range: "bytes=10485752-10485759" } })).text(), id)).toBe("aaaaaaaa");
    await cdp.detach();
    const output = join(directory, "large-export.slop"); await saveCopy(page, output);
    expect(ranges.length).toBeGreaterThan(8);
    for (const range of ranges) { const [start, end] = range.slice(6).split("-").map(Number); expect(end! - start! + 1).toBeLessThanOrEqual(BrowserHost.chunkBytes); }
    expect(documentContents(output)).toEqual(documentContents(file));
    const state = await execute({ method: "get", documentPath: output });
    expect((state.state.value as any).tasks.at(-1).text).toBe(id);
  } finally { await context.close(); }
}, 60_000);


test("Chrome recovers an atomic document after its worker is killed during SQLite writes", async () => {
  const context = await chromium.launchPersistentContext(await mkdtemp(join(directory, "crash-profile-")), { channel: "chrome" });
  try {
    for (const writeNumber of [2, 4, 6]) {
      const page = await context.newPage();
      await page.addInitScript(() => {
        const NativeWorker = Worker;
        (globalThis as any).Worker = class extends NativeWorker {
          constructor(url: string | URL, options?: WorkerOptions) {
            super(url, options);
            this.addEventListener("message", event => {
              if (event.data?.crashProbe === true) { this.terminate(); (globalThis as any).__killed = true; }
            });
          }
        };
      });
      await page.goto(await imported());
      await page.frameLocator("iframe").getByRole("textbox").waitFor();
      const url = page.url();
      const owner = page.workers().find(worker => worker.url().endsWith("/worker.js"))!;
      await owner.evaluate(writeNumber => {
        const proto = (globalThis as any).FileSystemSyncAccessHandle.prototype;
        const write = proto.write; let count = 0;
        proto.write = function (...args: any[]) {
          const result = write.apply(this, args);
          if (++count === writeNumber) {
            postMessage({ crashProbe: true });
            // Hold this real write boundary until the parent terminates the worker.
            const until = performance.now() + 2000;
            while (performance.now() < until) {}
          }
          return result;
        };
      }, writeNumber);
      const field = page.frameLocator("iframe").getByRole("textbox");
      await field.fill("Interrupted transaction"); await field.press("Tab");
      await page.waitForFunction(() => (globalThis as any).__killed === true);
      await page.close();
      const recovered = await context.newPage(); await recovered.goto(url);
      const recoveredField = recovered.frameLocator("iframe").getByRole("textbox");
      await recoveredField.waitFor();
      expect(["Fixture title", "Interrupted transaction"]).toContain(await recoveredField.inputValue());
      expect(await recovered.frameLocator("iframe").locator("[data-count]").textContent()).toBe("3");
      await recoveredField.fill("Recovered and saved"); await recoveredField.press("Tab");
      await recovered.frames().find(frame => frame.url().includes(".localhost:"))!.evaluate(async () => { await (globalThis as any).__devProbe.doc.flush(); });
      await recovered.waitForFunction(() => document.querySelector("#status")?.textContent === "Saved in this browser");
      await recovered.reload(); await recoveredField.waitFor();
      expect(await recoveredField.inputValue()).toBe("Recovered and saved");
      await recovered.close();
    }
  } finally { await context.close(); }
}, 60_000);


test("Chrome Rust export flushes accepted edits and resource reads reject oversized chunks", async () => {
  const context = await chromium.launchPersistentContext(await mkdtemp(join(directory, "barrier-profile-")), { channel: "chrome" });
  try {
    const page = await context.newPage(); await page.goto(await imported());
    await page.frameLocator("iframe").getByRole("textbox").waitFor();
    const worker = page.workers().find(worker => worker.url().endsWith("/worker.js"))!;
    const result = await worker.evaluate(async input => {
      const runtimeURL = "/browser/core/hitslop_core_wasm.js";
      const runtime = await import(runtimeURL);
      await runtime.default();
      const copy = await runtime.installCopy(crypto.randomUUID());
      copy.importBegin(input.length); copy.importWrite(new Uint8Array(input)); copy.importFinish(); copy.open();
      copy.page("edit", JSON.stringify({ method: "apply", batch: { intents: [{ type: "set", path: ["title"], value: "Rust barrier saved this" }] } }));
      copy.poll(0, Date.now());
      const chunks: number[] = [];
      copy.export((bytes: Uint8Array) => chunks.push(...bytes));
      let oversized = false;
      try { copy.resourceRange(false, "ui.js", 0, 1048577); } catch { oversized = true; }
      copy.page("failed-edit", JSON.stringify({ method: "apply", batch: { intents: [{ type: "set", path: ["title"], value: "Save after recovery" }] } }));
      copy.poll(1, Date.now());
      const prototype = (globalThis as any).FileSystemSyncAccessHandle.prototype;
      const original = prototype.write;
      let refused = false, began = false;
      prototype.write = () => { throw new DOMException("No space", "QuotaExceededError"); };
      try { copy.export(() => { began = true; }); } catch { refused = true; } finally { prototype.write = original; }
      await copy.recoverStorage();
      copy.export(() => {});
      return { chunks, oversized, refused, began };
    }, [...new Uint8Array(await Bun.file(snapshot).arrayBuffer())]);
    const file = join(directory, "rust-barrier.slop"); await Bun.write(file, new Uint8Array(result.chunks));
    const reopened = await execute({ method: "get", documentPath: file });
    expect((reopened.state as any).value.title).toBe("Rust barrier saved this");
    expect(result.oversized).toBe(true); expect(result.refused).toBe(true); expect(result.began).toBe(false);
  } finally { await context.close(); }
}, 30_000);

test("Chrome keep-awake reflects release and ignores a grant after switching off", async () => {
  const context = await chromium.launchPersistentContext(await mkdtemp(join(directory, "wake-profile-")), { channel: "chrome" });
  try {
    await context.addInitScript(() => {
      const probe = { sentinel: null as any, pending: null as any, delayed: false };
      (globalThis as any).__wakeProbe = probe;
      Object.defineProperty(navigator, "wakeLock", { value: { request: async () => {
        const sentinel = Object.assign(new EventTarget(), { released: false, async release() { sentinel.released = true; sentinel.dispatchEvent(new Event("release")); } });
        probe.sentinel = sentinel;
        if (probe.delayed) await new Promise(resolve => { probe.pending = resolve; });
        return sentinel;
      } } });
    });
    const page = await context.newPage(); await page.goto(await imported());
    await page.frameLocator("iframe").getByRole("textbox").waitFor();
    await page.getByRole("button", { name: "Keep awake: off" }).click();
    await page.getByRole("button", { name: "Keep awake: on" }).waitFor();
    await page.evaluate(() => (globalThis as any).__wakeProbe.sentinel.release());
    expect(await page.locator("#awake").textContent()).toBe("Keep awake: paused");
    await page.locator("#awake").click();
    await page.evaluate(() => { (globalThis as any).__wakeProbe.delayed = true; });
    await page.locator("#awake").click(); await page.locator("#awake").click();
    await page.evaluate(() => (globalThis as any).__wakeProbe.pending());
    await page.waitForFunction(() => (globalThis as any).__wakeProbe.sentinel.released);
    expect(await page.locator("#awake").textContent()).toBe("Keep awake: off");
  } finally { await context.close(); }
}, 30_000);

test("Chrome Save As cancellation and failed writes leave no completed temporary exports", async () => {
  const context = await chromium.launchPersistentContext(await mkdtemp(join(directory, "save-as-profile-")), { channel: "chrome" });
  try {
    const page = await context.newPage(); await page.goto(await imported());
    await page.frameLocator("iframe").getByRole("textbox").waitFor();
    const exportsRemaining = () => page.evaluate(async () => {
      const root = await navigator.storage.getDirectory();
      try {
        const folder = await (await root.getDirectoryHandle("exports")).getDirectoryHandle(new URL(location.href).searchParams.get("copy")!);
        return Array.fromAsync((folder as any).keys());
      } catch (error) { if ((error as DOMException).name === "NotFoundError") return []; throw error; }
    });
    await page.evaluate(() => { (window as any).showSaveFilePicker = async () => { throw new DOMException("Cancelled", "AbortError"); }; });
    await page.getByRole("button", { name: "Download .slop" }).click();
    await page.waitForFunction(() => !document.querySelector<HTMLButtonElement>("#download")?.disabled);
    expect(await page.getByRole("alert").count()).toBe(0);
    expect(await exportsRemaining()).toEqual([]);
    await page.evaluate(() => { (window as any).showSaveFilePicker = async () => ({ createWritable: async () => new WritableStream({ write() { throw new Error("Destination write failed"); } }) }); });
    await page.getByRole("button", { name: "Download .slop" }).click();
    await page.waitForFunction(() => !document.querySelector<HTMLButtonElement>("#download")?.disabled);
    expect(await page.getByRole("alert").textContent()).toContain("Destination write failed");
    expect(await exportsRemaining()).toEqual([]);
    const file = join(directory, "save-as-recovered.slop"); await saveCopy(page, file);
    expect(documentContents(file)).toEqual(documentContents(snapshot));
    expect(await exportsRemaining()).toEqual([]);
    await saveCopy(page, file);
    expect(documentContents(file)).toEqual(documentContents(snapshot));
    expect(await exportsRemaining()).toEqual([]);
  } finally { await context.close(); }
}, 30_000);

test("Chrome refuses unknown container formats without altering their files", async () => {
  const context = await chromium.launchPersistentContext(await mkdtemp(join(directory, "container-profile-")), { channel: "chrome" });
  try {
    const page = await context.newPage(); await page.goto(await imported());
    await page.frameLocator("iframe").getByRole("textbox").waitFor();
    const url = page.url(); const id = new URL(url).searchParams.get("copy")!;
    await page.evaluate(async format => {
      const folder = await (await (await navigator.storage.getDirectory()).getDirectoryHandle("copies")).getDirectoryHandle(new URL(location.href).searchParams.get("copy")!);
      const writable = await (await folder.getFileHandle("container.json")).createWritable();
      await writable.write(JSON.stringify({ format })); await writable.close();
    }, BrowserHost.containerFormat + 1);
    await page.close();
    const observer = await context.newPage(); await observer.goto(origin);
    const files = () => observer.evaluate(async id => {
      const copy = await (await (await navigator.storage.getDirectory()).getDirectoryHandle("copies")).getDirectoryHandle(id);
      const result: Record<string, string> = {};
      async function visit(folder: FileSystemDirectoryHandle, prefix: string) {
        for await (const [name, entry] of (folder as any).entries()) {
          if (entry.kind === "directory") await visit(entry, prefix + name + "/");
          else result[prefix + name] = [...new Uint8Array(await crypto.subtle.digest("SHA-256", await (await entry.getFile()).arrayBuffer()))].join(",");
        }
      }
      await visit(copy, ""); return result;
    }, id);
    const before = await files();
    const refused = await context.newPage(); await refused.goto(url);
    await refused.getByRole("alert").waitFor();
    expect(await refused.getByRole("alert").textContent()).toContain("Unsupported browser storage format");
    expect(await refused.getByRole("button", { name: "Download .slop" }).isDisabled()).toBe(true);
    expect(await files()).toEqual(before);
  } finally { await context.close(); }
}, 30_000);

test("Chrome reclaims an abandoned export only after the owning tab closes", async () => {
  const context = await chromium.launchPersistentContext(await mkdtemp(join(directory, "export-cleanup-profile-")), { channel: "chrome" });
  try {
    const page = await context.newPage(); await page.goto(await imported());
    await page.frameLocator("iframe").getByRole("textbox").waitFor();
    const id = new URL(page.url()).searchParams.get("copy")!;
    await page.evaluate(() => { (window as any).showSaveFilePicker = async () => ({ createWritable: async () => new WritableStream({ write() { return new Promise(() => {}); } }) }); });
    await page.getByRole("button", { name: "Download .slop" }).click();
    const observer = await context.newPage(); await observer.goto(origin);
    const hasExport = () => observer.evaluate(async id => {
      try { const folder = await (await (await navigator.storage.getDirectory()).getDirectoryHandle("exports")).getDirectoryHandle(id); return (await Array.fromAsync((folder as any).keys())).length > 0; }
      catch (error) { if ((error as DOMException).name === "NotFoundError") return false; throw error; }
    }, id);
    for (let tries = 0; tries < 100 && !await hasExport(); tries++) await new Promise(resolve => setTimeout(resolve, 20));
    expect(await hasExport()).toBe(true);
    await observer.reload(); await observer.getByRole("heading", { name: "Browser copies" }).waitFor();
    expect(await hasExport()).toBe(true);
    await page.close(); await observer.reload(); await observer.getByRole("heading", { name: "Browser copies" }).waitFor();
    expect(await hasExport()).toBe(false);
  } finally { await context.close(); }
}, 30_000);

// A document worker that fails, or stops answering, must not leave the page waiting. Its
// outstanding writes have an unknown outcome and fail together, later requests fail at
// once, and nothing is ever sent again: an uncertain mutation is never replayed.
type Outcome = { name: string; ok: boolean; code?: string; message?: string };
const wait = (ms: number) => new Promise(resolve => setTimeout(resolve, ms));

/** In each document of a context: the timers of the given lengths (the deadlines) still pending,
 * and the message ports closed so far. */
function watchDocuments(watched: number[]) {
  const root = globalThis as any;
  const live = new Map<unknown, number>();
  const { setTimeout: set, clearTimeout: clear } = root;
  root.setTimeout = (handler: unknown, delay?: number, ...args: unknown[]) => {
    const id = set(typeof handler === "function" ? (...rest: unknown[]) => { live.delete(id); return handler(...rest); } : handler, delay, ...args);
    if (delay !== undefined && watched.includes(delay)) live.set(id, delay);
    return id;
  };
  root.clearTimeout = (id: unknown) => { live.delete(id); clear(id); };
  root.__deadlines = () => [...live.values()].sort();
  let closed = 0;
  const close = MessagePort.prototype.close;
  MessagePort.prototype.close = function (this: MessagePort) { closed++; close.call(this); };
  root.__closedPorts = () => closed;
}
const deadlines = (where: Page | Frame) => where.evaluate(() => (globalThis as any).__deadlines() as number[]);
const closedPorts = (where: Page | Frame) => where.evaluate(() => (globalThis as any).__closedPorts() as number);
const asked = (worker: PageWorker) => worker.evaluate(() => (globalThis as any).__asked as string[]);

/** Opens a copy, then makes its worker record what it is sent and answer none of it. */
async function stalledCopy(context: BrowserContext) {
  const page = await context.newPage();
  const errors: string[] = []; page.on("pageerror", error => errors.push(error.message));
  await page.goto(await imported());
  await page.frameLocator("iframe").getByRole("textbox", { name: "Document title" }).waitFor();
  const frame = page.frames().find(frame => frame.url().includes(".localhost:"))!;
  const worker = page.workers().find(worker => worker.url().endsWith("/worker.js"))!;
  await worker.evaluate(() => {
    const received: string[] = [];
    (globalThis as any).__asked = received;
    (globalThis as any).onmessage = ({ data }: MessageEvent) => { received.push(data?.type); };
  });
  return { page, frame, worker, errors };
}
/** Three writes of two kinds: the first applies and the attachment is stored while the row waits its turn. `__outcomes` collects how each ends. */
function startWrites(frame: Frame) {
  return frame.evaluate(() => {
    const { doc, attachments } = (globalThis as any).__devProbe;
    const outcomes: Outcome[] = ((globalThis as any).__outcomes = []);
    const watch = (name: string, work: Promise<unknown>) => work.then(
      () => void outcomes.push({ name, ok: true }),
      (error: any) => void outcomes.push({ name, ok: false, code: error?.code, message: String(error?.message) }));
    void watch("title", doc.fields.title.set("Never confirmed"));
    void watch("row", doc.fields.tasks.insert({ text: "Never confirmed" }));
    void watch("attachment", attachments.import(new File(["never stored"], "note.txt"), (tx: any, ref: any) => tx.fields.tasks.insert({ text: ref.id })));
  });
}
async function untilAsked(worker: PageWorker, count: number) {
  for (let tries = 0; tries < 200 && (await asked(worker)).length < count; tries++) await wait(25);
  expect((await asked(worker)).length).toBeGreaterThanOrEqual(count);
}
/** The outcomes of the writes, once `count` of them ended; a write still waiting `ms` later is a hang. */
async function outcomesWithin(frame: Frame, count: number, ms: number) {
  const read = () => frame.evaluate(() => (globalThis as any).__outcomes as Outcome[]);
  const until = Date.now() + ms;
  let seen = await read();
  while (seen.length < count && Date.now() < until) { await wait(25); seen = await read(); }
  if (seen.length < count) throw new Error(`${count - seen.length} of ${count} page requests were still waiting ${ms} ms later`);
  return seen;
}
/** Once the connection is dead, a write fails at once and neither it nor a read of current state is sent. */
async function expectLaterRequestsFail(frame: Frame, worker: PageWorker, reason: RegExp) {
  const before = await asked(worker);
  const later = await frame.evaluate(async () => {
    const { doc } = (globalThis as any).__devProbe;
    const started = performance.now(); const failures: { code?: string; message: string }[] = [];
    for (const write of [() => doc.fields.title.set("After the failure"), () => doc.fields.tasks.insert({ text: "After the failure" })])
      await write().catch((error: any) => failures.push({ code: error?.code, message: String(error?.message) }));
    return { failures, milliseconds: performance.now() - started };
  });
  expect(later.failures.map(({ code }) => code)).toEqual(["unknown_outcome", "unknown_outcome"]);
  for (const { message } of later.failures) expect(message).toMatch(reason);
  expect(later.milliseconds).toBeLessThan(2000);
  // The worker is never asked again: a failed write is not replayed, nor is a later one sent.
  expect(await asked(worker)).toEqual(before);
}

test("Chrome rejects every outstanding and later page request when the document worker fails", async () => {
  const context = await chromium.launchPersistentContext(await mkdtemp(join(directory, "worker-failure-profile-")), { channel: "chrome" });
  try {
    await context.addInitScript(watchDocuments, [copyDeadline, 30_000]);
    const { page, frame, worker, errors } = await stalledCopy(context);
    await startWrites(frame);
    // Page requests reach the worker and wait on it, each under the page deadline.
    await untilAsked(worker, 2);
    await wait(200);
    const sent = await asked(worker);
    expect(sent.every(type => type === "page")).toBe(true);
    const waiting = await deadlines(frame);
    const closedBefore = [await closedPorts(page), await closedPorts(frame)];
    expect(await frame.evaluate(() => (globalThis as any).__outcomes)).toEqual([]);
    // The worker fails: an uncaught error reaches the host, as a crash in a timer would.
    await worker.evaluate(() => { setTimeout(() => { throw new Error("simulated worker failure"); }, 0); });
    const outcomes = await outcomesWithin(frame, 3, 5000);
    expect(outcomes.map(({ name }) => name).sort()).toEqual(["attachment", "row", "title"]);
    for (const outcome of outcomes) {
      expect(outcome.ok).toBe(false); expect(outcome.code).toBe("unknown_outcome");
      expect(outcome.message).toContain("The document worker stopped");
    }
    // Each request waited under a page deadline, now cleared. The person is told, and nothing that needs the worker stays enabled.
    expect(waiting).toEqual(sent.map(() => copyDeadline));
    expect(await page.getByRole("alert").textContent()).toContain("The document worker stopped");
    expect(await page.locator("#download").isDisabled()).toBe(true);
    // Every deadline timer is cleared, and the page connection is closed at both ends.
    expect(await deadlines(page)).toEqual([]); expect(await deadlines(frame)).toEqual([]);
    expect(await closedPorts(page)).toBeGreaterThan(closedBefore[0]!); expect(await closedPorts(frame)).toBeGreaterThan(closedBefore[1]!);
    await expectLaterRequestsFail(frame, worker, /The document worker stopped/);
    expect(await asked(worker)).toEqual(sent);
    // Nothing the failed worker posts afterwards is believed: the failure stays on screen.
    await worker.evaluate(() => postMessage({ type: "save", status: "saved", error: null }));
    await wait(300);
    expect(await page.locator("#status").textContent()).toContain("The document worker stopped");
    // A resource fetch through the Service Worker fails now, not after the 30 s resource deadline.
    const resource = await frame.evaluate(async () => {
      const started = performance.now(); const response = await fetch("/assets/ui.js");
      return { ok: response.ok, milliseconds: performance.now() - started };
    });
    expect(resource.ok).toBe(false); expect(resource.milliseconds).toBeLessThan(5000);
    expect(await deadlines(page)).toEqual([]); expect(await deadlines(frame)).toEqual([]);
    expect(await asked(worker)).toEqual(sent);
    // A frame that connects after the failure is told at once: it fails to open, it does not wait.
    errors.length = 0;
    await frame.evaluate(() => { setTimeout(() => location.reload(), 0); });
    for (let tries = 0; tries < 200 && !errors.some(message => message.includes("The document worker stopped")); tries++) await wait(25);
    expect(errors.some(message => message.includes("The document worker stopped"))).toBe(true);
    expect(await asked(worker)).toEqual(sent);
    // The person is not held on a dead copy: the catalog link leaves without the save that cannot happen.
    await page.getByRole("link", { name: "hitSlop", exact: true }).click();
    await page.getByRole("heading", { name: "Browser copies", exact: true }).waitFor({ timeout: 5000 });
  } finally { await context.close(); }
}, 60_000);

test("Chrome fails a page request when the worker throws outside request handling", async () => {
  const context = await chromium.launchPersistentContext(await mkdtemp(join(directory, "worker-poll-profile-")), { channel: "chrome" });
  try {
    await context.addInitScript(watchDocuments, [copyDeadline, 30_000]);
    const page = await context.newPage(); await page.goto(await imported());
    const field = page.frameLocator("iframe").getByRole("textbox", { name: "Document title" });
    await field.waitFor();
    // A saved, idle copy has no timer left in its worker: the next poll belongs to a request.
    await field.fill("Saved before the failure"); await field.press("Tab");
    await page.waitForFunction(() => document.querySelector("#status")?.textContent === "Saved in this browser");
    await wait(1500);
    const frame = page.frames().find(frame => frame.url().includes(".localhost:"))!;
    const worker = page.workers().find(worker => worker.url().endsWith("/worker.js"))!;
    await worker.evaluate(async () => {
      const runtimeURL = "/browser/core/hitslop_core_wasm.js";
      const runtime = await import(runtimeURL);
      runtime.BrowserCopy.prototype.poll = function () { throw new Error("simulated poll failure"); };
    });
    await startWrites(frame);
    const outcomes = await outcomesWithin(frame, 3, 5000);
    for (const outcome of outcomes) { expect(outcome.ok).toBe(false); expect(outcome.code).toBe("unknown_outcome"); }
    expect(await page.getByRole("alert").textContent()).toContain("The document worker stopped");
    expect(await deadlines(page)).toEqual([]); expect(await deadlines(frame)).toEqual([]);
  } finally { await context.close(); }
}, 60_000);

test("Chrome fails every page request for good when the document worker stops answering", async () => {
  const context = await chromium.launchPersistentContext(await mkdtemp(join(directory, "worker-silence-profile-")), { channel: "chrome" });
  try {
    // The worker never reports anything, so only the page's own deadline can end the wait.
    await context.clock.install();
    await context.addInitScript(watchDocuments, [copyDeadline, 30_000]);
    const { frame, worker } = await stalledCopy(context);
    await startWrites(frame);
    await untilAsked(worker, 2);
    await wait(200);
    const sent = await asked(worker);
    const waiting = await deadlines(frame);
    // Just short of the deadline every write is still waiting; just past it, all have failed.
    await context.clock.fastForward(copyDeadline - 1000);
    expect(await frame.evaluate(() => (globalThis as any).__outcomes)).toEqual([]);
    await context.clock.fastForward(2000);
    const outcomes = await outcomesWithin(frame, 3, 5000);
    expect(waiting).toEqual(sent.map(() => copyDeadline));
    for (const outcome of outcomes) {
      expect(outcome.ok).toBe(false); expect(outcome.code).toBe("unknown_outcome");
      expect(outcome.message).toContain("did not respond");
    }
    // A timed-out write has an unknown outcome, so the whole connection fails and nothing is retried.
    expect(await deadlines(frame)).toEqual([]);
    await expectLaterRequestsFail(frame, worker, /did not respond/);
    expect(await asked(worker)).toEqual(sent);
  } finally { await context.close(); }
}, 60_000);
