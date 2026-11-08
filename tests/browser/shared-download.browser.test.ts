import { afterAll, beforeAll, expect, test } from "bun:test";
import { chromium, type Page } from "playwright";
import { mkdtemp, rm } from "node:fs/promises";
import { join } from "node:path";
import { createHash } from "node:crypto";
import { brotliDecompressSync } from "node:zlib";
import { Database } from "bun:sqlite";
import { createFixture } from "../../packages/pop/tests/cli/dev-fixture";
import { buildTemplate } from "../../packages/pop/src/cli/template";
import { execute } from "../../packages/pop/src/cli/engine";
import { SharingHost } from "../../packages/pop/src/wire/constants.generated";

const origin = SharingHost.origin;
const hash = (bytes: Uint8Array) => createHash("sha256").update(bytes).digest("hex");
const attachment = Buffer.from("initial attachment received before any edits");
const attachmentID = hash(attachment);
let directory: string, seed: Uint8Array, documentID: string, app: string, worker: string, catalog: string;
let manifest: unknown;
beforeAll(async () => {
  directory = await mkdtemp(join(process.cwd(), ".hosted-download-test-"));
  const source = join(directory, "source");
  await createFixture(process.cwd(), source);
  const template = await buildTemplate(source, join(directory, "template.pop"), { renderViews: false });
  const file = join(directory, "seed.pop");
  await execute({ method: "create", from: template, output: file });
  await execute({ method: "batch", documentPath: file, attachments: [attachment.toString("base64")],
    batch: { intents: [{ type: "set", path: ["title"], value: attachmentID }] } });
  const inspected = await execute({ method: "inspect", file });
  if (!inspected.ok || inspected.method !== "inspect") throw new Error("Fixture inspection failed");
  app = inspected.info.appDigest;
  const db = new Database(file);
  try {
    documentID = (db.query("SELECT uuid FROM document").get() as { uuid: string }).uuid;
    const assets = db.query("SELECT key,media_type,encoding,bytes FROM assets WHERE key IN ('ui.js','ui.css') OR key LIKE 'media/%'").all() as { key: string; media_type: string; encoding: string; bytes: Uint8Array }[];
    manifest = { app, resources: assets.map(item => {
      const bytes = item.encoding === "br" ? brotliDecompressSync(item.bytes) : item.bytes;
      return { key: item.key, media_type: item.media_type, size: bytes.length, hash: hash(bytes) };
    }) };
    // Model the actual attachment-free wire artifact without changing its app or history.
    db.exec("DELETE FROM attachments; DELETE FROM artwork; VACUUM");
  } finally { db.close(); }
  seed = await Bun.file(file).bytes();
  const bundle = await Bun.build({ entrypoints: ["packages/pop/src/sharing/worker.ts"], target: "browser" });
  if (!bundle.success) throw new Error(bundle.logs.join("\n"));
  worker = await bundle.outputs[0]!.text();
  const catalogBundle = await Bun.build({ entrypoints: ["packages/pop/src/browser/copies.ts"], target: "browser" });
  if (!catalogBundle.success) throw new Error(catalogBundle.logs.join("\n"));
  catalog = await catalogBundle.outputs[0]!.text();
}, 120_000);
afterAll(async () => { if (directory) await rm(directory, { recursive: true, force: true }); });

async function start(page: Page, copy: string, fresh: boolean) {
  await page.goto(origin);
  await page.evaluate(async ({ copy, fresh, invitation }) => {
    const catalogURL = "/copy-catalog.js";
    const copies = await import(catalogURL);
    if (fresh) {
      // The host clears interrupted imports before starting a new join.
      await copies.cleanup();
      await copies.putCopy({ id: copy, title: "Importing…", created: Date.now(), state: "importing" });
    }
    const w = new Worker("/browser/worker.js", { type: "module" });
    (globalThis as any).worker = w;
    (globalThis as any).copyID = copy;
    (globalThis as any).events = [];
    w.onmessage = async ({ data }) => {
      if (data.type === "ready") await copies.updateCopy(copy, { title: data.title, state: "ready" });
      (globalThis as any).events.push(data);
    };
    w.onerror = error => (globalThis as any).events.push({ type: "workerError", error: error.message });
    w.postMessage({ type: "joinInvitation", invitation });
    w.postMessage({ type: "open", copy, source: fresh ? "0".repeat(64) : null });
  }, { copy, fresh, invitation: `${origin}/join#${documentID}.${"b".repeat(64)}` });
}
async function ready(page: Page) {
  await page.waitForFunction(() => (globalThis as any).events.some((e: any) => e.type === "ready" || e.type === "error" || e.type === "workerError"));
  const events = await page.evaluate(() => (globalThis as any).events);
  expect(events.filter((e: any) => e.type === "error" || e.type === "workerError")).toEqual([]);
  expect(events.some((e: any) => e.type === "ready")).toBe(true);
}

test("hosted initial download waits for verified attachments, persists them and reopens after a failed attempt", async () => {
  const browser = await chromium.launch({ channel: "chrome" });
  const context = await browser.newContext();
  let corrupt = true, requested = 0;
  let release: () => void = () => {};
  const gate = new Promise<void>(resolve => { release = resolve; });
  try {
    // No fixture credentials or socket traffic may reach the deployed relay.
    await context.routeWebSocket("**/*", socket => socket.close({ code: 1000, reason: "Initial-download fixture" }));
    await context.route("**/*", async route => {
      const url = new URL(route.request().url());
      const headers = { "Access-Control-Allow-Origin": "*", "Access-Control-Allow-Headers": "*", "Access-Control-Allow-Methods": "GET,POST,OPTIONS" };
      if (url.hostname === "identitytoolkit.googleapis.com" || url.hostname === "securetoken.googleapis.com") {
        await route.fulfill({ headers, json: { localId: "fixture-user", user_id: "fixture-user", refreshToken: "refresh", refresh_token: "refresh", idToken: "identity", id_token: "identity", expiresIn: "3600", expires_in: "3600" } }); return;
      }
      if (url.pathname.startsWith("/_resources/")) {
        await route.fulfill({ headers: { ...headers, "Access-Control-Allow-Origin": origin, "Access-Control-Allow-Credentials": "true" }, body: "ready" }); return;
      }
      if (url.origin !== origin) { await route.abort(); return; }
      const path = url.pathname;
      if (path === "/") { await route.fulfill({ contentType: "text/html", body: "<!doctype html><title>Trusted worker fixture</title>" }); return; }
      if (path === "/copy-catalog.js") { await route.fulfill({ contentType: "application/javascript", body: catalog }); return; }
      if (path === "/browser/worker.js") { await route.fulfill({ contentType: "application/javascript", body: worker }); return; }
      if (path.startsWith("/browser/core/")) { await route.fulfill({ contentType: path.endsWith(".wasm") ? "application/wasm" : "application/javascript", body: Buffer.from(await Bun.file("generated/browser/core/" + path.split("/").pop()).arrayBuffer()) }); return; }
      if (path === "/auth/config") { await route.fulfill({ json: { apiKey: "fixture", projectId: "fixture-project" } }); return; }
      if (path.endsWith("/members")) { await route.fulfill({ json: {} }); return; }
      if (path === `/v1/documents/${documentID}`) { await route.fulfill({ json: { document: documentID, phase: "live", seedHash: hash(seed), seedSize: seed.length, app, compact: false } }); return; }
      if (path.endsWith("/seed")) { await route.fulfill({ body: Buffer.from(seed) }); return; }
      if (path.endsWith("/resources")) { await route.fulfill({ json: manifest }); return; }
      if (path.endsWith("/blobs")) { await route.fulfill({ json: [{ key: attachmentID, hash: attachmentID, size: attachment.length, media_type: "application/octet-stream" }] }); return; }
      if (path.endsWith("/blob/" + attachmentID)) {
        requested++;
        if (corrupt) await gate;
        await route.fulfill({ body: corrupt ? Buffer.alloc(attachment.length) : attachment }); return;
      }
      await route.fulfill({ status: 503, body: "No live synchronization in this initial-download fixture" });
    });
    const page = await context.newPage();
    const copy = crypto.randomUUID();
    await start(page, copy, true);
    const deadline = Date.now() + 30_000;
    while (!requested && Date.now() < deadline) await new Promise(resolve => setTimeout(resolve, 20));
    if (!requested) throw new Error(JSON.stringify(await page.evaluate(() => (globalThis as any).events)));
    expect(await page.evaluate(() => (globalThis as any).events.some((e: any) => e.type === "ready"))).toBe(false);
    release();
    await page.waitForFunction(() => (globalThis as any).events.some((e: any) => e.type === "error"));
    expect(await page.evaluate(() => (globalThis as any).events.some((e: any) => e.type === "ready"))).toBe(false);
    corrupt = false;
    await start(page, copy, true);
    await ready(page);
    await start(page, copy, false);
    await ready(page);
    await page.evaluate(() => (globalThis as any).worker.postMessage({ type: "export", id: "saved-copy" }));
    await page.waitForFunction(() => (globalThis as any).events.some((e: any) => e.type === "exported"));
    const bytes = await page.evaluate(async () => {
      const event = (globalThis as any).events.find((e: any) => e.type === "exported");
      const root = await navigator.storage.getDirectory();
      const folder = await (await root.getDirectoryHandle("exports")).getDirectoryHandle((globalThis as any).copyID);
      return Array.from(new Uint8Array(await (await (await folder.getFileHandle(event.file)).getFile()).arrayBuffer()));
    });
    const output = join(directory, "downloaded.pop");
    await Bun.write(output, Buffer.from(bytes));
    const read = await execute({ method: "attachments.read", documentPath: output, attachmentID });
    if (!read.ok || read.method !== "attachments.read") throw new Error("Downloaded attachment missing");
    expect(Buffer.from(read.state.bytes, "base64")).toEqual(attachment);
  } finally { release(); await context.close(); await browser.close(); }
}, 60_000);
