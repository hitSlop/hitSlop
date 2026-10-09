import { mkdir, mkdtemp, readFile, readdir, rename, rm, writeFile } from "node:fs/promises";
import { join, resolve } from "node:path";
import { homedir, tmpdir } from "node:os";
import { createHash, randomBytes } from "node:crypto";
import { BrowserHost, BrowserResourcePolicy, BrowserHostPolicy } from "../schema/constants";
import { cliRoot } from "./paths";
import { execute } from "./engine";
import { run } from "./process";

const origin = `http://127.0.0.1:${BrowserHost.port}`;
const auth = (token: string) => ({ Authorization: `Bearer ${token}` });
type Discovery = { protocol: number; port: number; pid: number; token: string; runtime: string };
type Session = { directory: string; file: string; expires: number; size: number };
const hostPolicy = BrowserHostPolicy;
const pagePolicy = BrowserResourcePolicy;

async function runtimeHash(root: string) {
  const hash = createHash("sha256");
  const files = (await readdir(root, { recursive: true, withFileTypes: true })).filter(entry => entry.isFile());
  const names = files.map(entry => join(entry.parentPath, entry.name).slice(root.length + 1)).sort();
  if (!names.includes("host.js")) throw new Error("Browser runtime is missing. Build it with bun scripts/build/browser.ts or reinstall the CLI.");
  for (const name of names) { hash.update(name); hash.update(await readFile(join(root, name))); }
  return hash.digest("hex");
}

/** Exported for the browser boundary tests. The production port is fixed: it owns OPFS. */
export async function startBrowserHost(runtimeRoot = join(cliRoot, "browser")) {
  const runtime = await runtimeHash(runtimeRoot);
  const token = randomBytes(32).toString("hex");
  const sessions = new Map<string, Session>();
  let uploads = 0;
  const closeSession = async (id: string) => {
    const session = sessions.get(id);
    if (session) { sessions.delete(id); await rm(session.directory, { recursive: true, force: true }); }
  };
  const discovery: Discovery = { port: BrowserHost.port, protocol: BrowserHost.protocol, pid: process.pid, token, runtime };
  const response = (body: BodyInit | null, status = 200, headers: HeadersInit = {}) => new Response(body, { status, headers: { "Cache-Control": "no-store", "X-Content-Type-Options": "nosniff", ...headers } });
  const server = Bun.serve({
    hostname: "127.0.0.1", port: BrowserHost.port, maxRequestBodySize: BrowserHost.fileBytes,
    async fetch(request) {
      try {
        const url = new URL(request.url);
        const trusted = url.host === `127.0.0.1:${BrowserHost.port}`;
        const frame = new RegExp(`^[0-9a-f-]{36}\\.localhost:${BrowserHost.port}$`).test(url.host);
        if (!trusted && !frame) return response("Unrecognized host", 403);
        if (frame) {
          if (request.method !== "GET") return response(null, 405);
          if (url.pathname === "/") return response('<!doctype html><html><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>Slop</title></head><body><script type="module" src="/frame.js"></script></body></html>', 200, { "Content-Type": "text/html", "Content-Security-Policy": pagePolicy });
          if (["/frame.js", "/service-worker.js"].includes(url.pathname)) return response(Bun.file(join(runtimeRoot, url.pathname.slice(1))), 200, { "Content-Type": "text/javascript", "Service-Worker-Allowed": "/" });
          return response("Resource unavailable", 404);
        }
        if (url.pathname.startsWith("/__host/")) {
          if (request.headers.has("Origin") || request.headers.get("Authorization") !== `Bearer ${token}`) return response(null, 403);
          if (url.pathname === "/__host/health" && request.method === "GET") return Response.json(discovery);
          if (url.pathname === "/__host/import" && request.method === "POST") {
            if (sessions.size + uploads >= 8) return response("Too many pending imports; finish one first", 429);
            const size = Number(request.headers.get("Content-Length"));
            if (!Number.isSafeInteger(size) || size < 100 || size > BrowserHost.fileBytes || !request.body) return response("Invalid snapshot size", 413);
            uploads++;
            try {
              const directory = await mkdtemp(join(tmpdir(), "hitslop-browser-import-"));
              const file = join(directory, "snapshot.slop");
              const id = randomBytes(32).toString("hex");
              let bytes = 0;
              try {
                const stream = request.body.pipeThrough(new TransformStream({ transform(chunk, controller) {
                  bytes += chunk.byteLength;
                  if (bytes > size) throw new Error("Snapshot exceeds declared size");
                  controller.enqueue(chunk);
                } }));
                await Bun.write(file, new Response(stream));
                if (bytes !== size) throw new Error("Incomplete snapshot upload");
                sessions.set(id, { directory, file, size, expires: Date.now() + 600_000 });
                return Response.json({ source: id });
              } catch (error) { await rm(directory, { recursive: true, force: true }); throw error; }
            } finally { uploads--; }
          }
          return response(null, 404);
        }
        if (url.pathname.startsWith("/__import/")) {
          if (request.headers.get("Origin") && request.headers.get("Origin") !== origin) return response(null, 403);
          const id = url.pathname.slice("/__import/".length);
          const session = sessions.get(id);
          if (!session || session.expires <= Date.now()) return response("Import expired", 410);
          if (request.method === "DELETE") { await closeSession(id); return response(null, 204); }
          if (request.method === "HEAD") return response(null, 200, { "Content-Length": String(session.size), "Accept-Ranges": "bytes" });
          if (request.method !== "GET") return response(null, 405);
          const range = /^bytes=(\d+)-(\d+)$/.exec(request.headers.get("Range") || "");
          if (!range) return response(null, 416);
          const start = Number(range[1]), end = Number(range[2]);
          if (start > end || end >= session.size || end - start + 1 > BrowserHost.chunkBytes) return response(null, 416);
          return response(Bun.file(session.file).slice(start, end + 1), 206, { "Content-Type": "application/octet-stream", "Content-Length": String(end - start + 1), "Content-Range": `bytes ${start}-${end}/${session.size}` });
        }
        if (request.method !== "GET") return response(null, 405);
        if (url.pathname === "/") return response(Bun.file(join(runtimeRoot, "index.html")), 200, { "Content-Type": "text/html", "Content-Security-Policy": hostPolicy });
        const name = url.pathname.slice("/browser/".length);
        if (!url.pathname.startsWith("/browser/") || !/^(host|worker|evaluator)\.js$|^(core|evaluator)\/hitslop_core_wasm(_bg\.wasm|\.js)$/.test(name)) return response(null, 404);
        return response(Bun.file(join(runtimeRoot, name)), 200, { "Content-Type": name.endsWith(".wasm") ? "application/wasm" : "text/javascript" });
      } catch (error) { return response(String(error), 500); }
    },
  });
  const expiry = setInterval(() => { for (const [id, session] of sessions) if (session.expires <= Date.now()) void closeSession(id); }, 30_000);
  return { discovery, async close() { clearInterval(expiry); server.stop(true); await Promise.all([...sessions.keys()].map(closeSession)); } };
}

export async function openBrowser(file?: string) {
  const runtimeRoot = join(cliRoot, "browser");
  const runtime = await runtimeHash(runtimeRoot);
  const folder = join(homedir(), ".hitslop");
  const discoveryPath = join(folder, "browser-host.json");
  let discovery: Discovery | undefined;
  let running: Awaited<ReturnType<typeof startBrowserHost>> | undefined;
  try {
    const saved = JSON.parse(await readFile(discoveryPath, "utf8")) as Discovery;
    const health = await fetch(`${origin}/__host/health`, { headers: auth(saved.token), signal: AbortSignal.timeout(2000) });
    if (health.ok) discovery = await health.json() as Discovery;
  } catch { /* A stale discovery file is harmless; binding the fixed port decides ownership. */ }
  if (discovery && (discovery.protocol !== BrowserHost.protocol || discovery.runtime !== runtime)) throw new Error("An older hitSlop browser host is running. Stop it in its terminal, then run this command again.");
  if (!discovery) {
    try { running = await startBrowserHost(runtimeRoot); }
    catch (error) { if ((error as { code?: string }).code === "EADDRINUSE") throw new Error(`Another process owns browser port ${BrowserHost.port}. Stop it first; changing ports would hide your saved copies.`); throw error; }
    discovery = running.discovery;
    try {
      await mkdir(folder, { recursive: true });
      const temporary = `${discoveryPath}.${process.pid}`;
      await writeFile(temporary, JSON.stringify(discovery), { mode: 0o600 });
      await rename(temporary, discoveryPath);
    } catch (error) { await running.close(); throw error; }
  }
  try {
    let url = origin;
    if (file) {
      const directory = await mkdtemp(join(tmpdir(), "hitslop-browser-snapshot-"));
      try {
        const output = join(directory, "snapshot.slop");
        await execute({ method: "copy", documentPath: resolve(file), output });
        const snapshot = Bun.file(output);
        const imported = await fetch(`${origin}/__host/import`, { method: "POST", headers: { ...auth(discovery.token), "Content-Length": String(snapshot.size) }, body: snapshot });
        if (!imported.ok) throw new Error(await imported.text());
        const { source } = await imported.json() as { source: string };
        url = `${origin}/?copy=${crypto.randomUUID()}#import=${source}`;
      } finally { await rm(directory, { recursive: true, force: true }); }
    }
    console.log(url);
    await run(process.platform === "darwin" ? ["open", "-a", "Google Chrome", url] : ["google-chrome", url]);
    if (running) {
      console.log("Local browser host is running. Keep this terminal open; Ctrl-C stops it.");
      await new Promise<void>(resolve => { process.once("SIGINT", resolve); process.once("SIGTERM", resolve); });
    }
  } finally {
    if (running) {
      // Withdraw discovery before releasing the port, so this process cannot
      // remove the next host's descriptor during shutdown.
      try {
        const saved = JSON.parse(await readFile(discoveryPath, "utf8")) as Discovery;
        if (saved.token === running.discovery.token) await rm(discoveryPath, { force: true });
      } finally { await running.close(); }
    }
  }
}
