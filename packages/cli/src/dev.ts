import { previewResizeScript } from "./preview-resize";
import { createServer, type Plugin, type ViteDevServer } from "vite";
import { readFile, mkdtemp, rm, realpath } from "node:fs/promises";
import { dirname, join, resolve } from "node:path";
import { tmpdir } from "node:os";
import { createHash } from "node:crypto";
import { appConfig, virtualEntry } from "./vite";
import { discoverEntry } from "./entry";
import { previewFrame } from "./preview";
import { cliRoot, shellDirectory } from "./paths";
import { metadataFiles, stageWorker } from "./build";

export async function startDev(source: string, port = 0, signal?: AbortSignal) {
  source = await realpath(resolve(source));
  const temporary = await mkdtemp(join(tmpdir(), "hitslop-preview-"));
  const out = join(temporary, "preview");
  const graph = join(temporary, "dependencies.json");
  let child: { kill(): void } | undefined;
  let closed = false;
  let server: ViteDevServer | undefined;
  let dependencies = new Set(
    metadataFiles.map((file) => join(source, file)),
  );
  let metadataFailed = false;
  const metadata = async () => {
    if (closed) return;
    const worker = stageWorker([source, out, "--deps", graph], "Metadata evaluation failed");
    child = worker;
    try {
      await worker.done;
      const next = new Set<string>(JSON.parse(await readFile(graph, "utf8")));
      // Vite watches the source itself; only files outside it are watched for metadata.
      const outside = (file: string) => !file.startsWith(source + "/");
      server?.watcher.unwatch([...dependencies].filter((file) => !next.has(file) && outside(file)));
      dependencies = next;
      server?.watcher.add([...dependencies]);
      metadataFailed = false;
    } catch (error) {
      metadataFailed = true;
      throw error;
    } finally {
      child = undefined;
    }
  };
  // One rebuild runs at a time; changes during it are coalesced into the next one.
  let wanted: { metadata: boolean; entry: boolean } | undefined;
  let rebuilding: Promise<void> | undefined;
  const rebuild = (current: ViteDevServer, change: { metadata: boolean; entry: boolean }) => {
    wanted = { metadata: !!wanted?.metadata || change.metadata, entry: !!wanted?.entry || change.entry };
    rebuilding ??= (async () => {
      while (wanted && !closed) {
        const next = wanted;
        wanted = undefined;
        try {
          if (next.metadata) await metadata();
          if (next.entry) {
            const entry = current.moduleGraph.getModuleById(virtualEntry);
            if (entry) current.moduleGraph.invalidateModule(entry);
            current.config.optimizeDeps.entries = (await discoverEntry(source)).files.map((file) =>
              join(source, file),
            );
          }
          if (!closed) current.ws.send({ type: "full-reload" });
        } catch (error) {
          if (!closed)
            current.ws.send({
              type: "error",
              err: { message: String(error), stack: "", plugin: "hitslop-metadata" },
            });
        }
      }
      rebuilding = undefined;
    })();
  };
  let closing: Promise<void> | undefined;
  const close = () => {
    if (closing) return closing;
    closed = true;
    signal?.removeEventListener("abort", onAbort);
    child?.kill();
    return (closing = (async () => {
      await server?.close();
      await rebuilding;
      await rm(temporary, { recursive: true, force: true });
    })());
  };
  const onAbort = () => {
    void close().catch((error) => console.error(error));
  };
  signal?.addEventListener("abort", onAbort, { once: true });
  try {
    signal?.throwIfAborted();
    await metadata();
    signal?.throwIfAborted();
    const config = await appConfig(source);
    signal?.throwIfAborted();
    const policy =
      "default-src 'none'; script-src 'self' 'wasm-unsafe-eval'; connect-src 'self' ws://127.0.0.1:* https: blob:; media-src 'self' https: blob:; frame-src https:; style-src 'self' 'unsafe-inline'; img-src 'self' data: https: blob:; font-src 'self'";
    const page =
      '<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><script type="module" src="/@vite/client"></script></head><body><script type="module" src="/__shell__/boot.js"></script></body></html>';
    const conventional = new Set(
      ["main.ts", "App.svelte", "styles.css", "Export.svelte", "Icon.svelte"].map((file) =>
        join(source, file),
      ),
    );
    const host: Plugin = {
      name: "hitslop-preview-host",
      configureServer(current) {
        current.watcher.add([...dependencies]);
        current.watcher.on("all", (event, file) => {
          if (closed || !["add", "unlink", "change"].includes(event)) return;
          const entryChanged = event !== "change" && conventional.has(file);
          const metadataChanged =
            dependencies.has(file) || (metadataFailed && /\.(ts|json)$/.test(file));
          if (!entryChanged && !metadataChanged) return;
          rebuild(current, { metadata: metadataChanged, entry: entryChanged });
        });
        current.middlewares.use(async (req, res, next) => {
          try {
            const path = decodeURIComponent(new URL(req.url!, "http://localhost").pathname);
            let content: string | Buffer | undefined;
            let type = "application/json";
            if (path === "/") {
              content = previewFrame(JSON.parse(await readFile(join(out, "app.json"), "utf8")).manifest);
              type = "text/html";
            } else if (path === "/__preview__/resize.js") {
              content = Buffer.from(previewResizeScript);
              type = "text/javascript";
            } else if (path === "/app.html") {
              content = page;
              type = "text/html";
            } else if (path.startsWith("/__shell__/")) {
              const file = resolve(shellDirectory, path.slice(11));
              if (!file.startsWith(shellDirectory + "/")) {
                res.statusCode = 403;
                res.end();
                return;
              }
              content = await readFile(file);
              type = file.endsWith(".wasm") ? "application/wasm" : "text/javascript";
            } else if (path === "/app.json") {
              content = await readFile(join(out, "app.json"));
            }
            if (content === undefined) {
              next();
              return;
            }
            res.setHeader("Content-Type", type);
            res.setHeader("Cache-Control", "no-store");
            res.setHeader(
              "Content-Security-Policy",
              path === "/"
                ? "default-src 'none'; script-src 'self'; style-src 'unsafe-inline'; img-src 'self'; frame-src 'self'"
                : policy,
            );
            res.end(content);
          } catch (error) {
            next(error as Error);
          }
        });
      },
      handleHotUpdate(ctx) {
        // Metadata changes deliberately replace the disposable owner after fresh evaluation.
        if (dependencies.has(ctx.file)) return [];
      },
    };
    server = await createServer({
      ...config,
      plugins: [host, ...(config.plugins ?? [])],
      appType: "custom",
      cacheDir: join(
        tmpdir(),
        "hitslop-vite-cache",
        createHash("sha256")
          .update(cliRoot + source)
          .digest("hex"),
      ),
      server: {
        host: "127.0.0.1",
        port,
        strictPort: port !== 0,
        fs: {
          strict: true,
          allow: [source, dirname(dirname(Bun.resolveSync("@hitslop/document", cliRoot)))],
        },
      },
    });
    signal?.throwIfAborted();
    await server.listen();
    signal?.throwIfAborted();
    return { url: server.resolvedUrls!.local[0]!, close };
  } catch (error) {
    await close();
    throw error;
  }
}
