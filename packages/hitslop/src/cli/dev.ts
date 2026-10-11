import { createServer, type Plugin, type ViteDevServer } from "vite";
import { readFile, mkdtemp, rm, realpath } from "node:fs/promises";
import { dirname, join, resolve } from "node:path";
import { tmpdir } from "node:os";
import { createHash } from "node:crypto";
import { roleName } from "./app-vite";
import { appConfig, virtualEntry } from "./vite";
import { previewFrame } from "./preview";
import { previewResizeScript } from "./preview-resize";
import { cliRoot, shellDirectory } from "./paths";
import { stageWorker } from "./build";
import { execute } from "./engine";
import { PreviewOwners } from "./preview-owner";
import { previewClient } from "./preview-client";
import type { BuildInput, Capability, WindowDefinition } from "../wire/app.generated";
import {
  appContentSecurityPolicy,
  capabilityPermissionsPolicy,
  previewHostPolicy,
} from "../schema/policy";
import { AttachmentIdPattern } from "../schema/constants";

export async function startDev(source: string, port = 0, signal?: AbortSignal) {
  source = await realpath(resolve(source));
  const temporary = await mkdtemp(join(tmpdir(), "hitslop-preview-"));
  let stage = "",
    template = "",
    input: BuildInput,
    window: WindowDefinition,
    capabilities: Capability[] = [];
  const hosts = new PreviewOwners(temporary, () => template);
  let dependencies = new Set([join(source, "slop.ts")]);
  let closed = false,
    failed = false;
  let server: ViteDevServer | undefined, child: { kill(): void } | undefined;
  let rebuilding: Promise<void> | undefined,
    wanted = false;
  const build = async () => {
    await hosts.invalidate();
    const next = await mkdtemp(join(temporary, "build-"));
    try {
      const worker = stageWorker(source, join(next, "stage"), "Definition build failed");
      child = worker;
      const built = await worker.done;
      await execute({
        method: "pack",
        app: built.input,
        stage: join(next, "stage"),
        file: join(next, "preview.slop"),
      });
      const old = stage;
      input = built.input;
      window = built.window;
      capabilities = built.capabilities;
      stage = next;
      template = join(next, "preview.slop");
      dependencies = new Set(
        built.dependencies.definition.filter((path) => !/\.(?:svelte|css)$/.test(path)),
      );
      server?.watcher.add([...dependencies]);
      failed = false;
      hosts.activate();
      if (old) await rm(old, { recursive: true, force: true });
    } catch (error) {
      failed = true;
      await rm(next, { recursive: true, force: true });
      throw error;
    } finally {
      child = undefined;
    }
  };
  const rebuild = () => {
    wanted = true;
    rebuilding ??= (async () => {
      while (wanted && !closed) {
        wanted = false;
        try {
          await build();
          if (!closed) server?.ws.send({ type: "full-reload", path: "*" });
        } catch (error) {
          if (!closed)
            server?.ws.send({
              type: "error",
              err: { message: String(error), stack: "", plugin: "hitslop-definition" },
            });
        }
      }
      rebuilding = undefined;
    })();
  };
  let closing: Promise<void> | undefined;
  const close = () =>
    (closing ??= (async () => {
      closed = true;
      signal?.removeEventListener("abort", onAbort);
      child?.kill();
      await hosts.close();
      await server?.close();
      await rebuilding;
      await rm(temporary, { recursive: true, force: true });
    })());
  const onAbort = () => {
    void close().catch(console.error);
  };
  signal?.addEventListener("abort", onAbort, { once: true });
  try {
    signal?.throwIfAborted();
    await build();
    signal?.throwIfAborted();
    const host: Plugin = {
      name: "hitslop-preview-host",
      configureServer(current) {
        hosts.attach(current);
        current.watcher.add([source, ...dependencies]);
        current.watcher.on("all", (event, file) => {
          const rootRole = dirname(file) === source && roleName(file);
          const inventoryChanged = !!rootRole && event !== "change";
          if (
            !closed &&
            ["add", "unlink", "change"].includes(event) &&
            (inventoryChanged ||
              dependencies.has(file) ||
              (failed && file.startsWith(source + "/")))
          ) {
            const entry = current.moduleGraph.getModuleById(virtualEntry);
            if (entry) current.moduleGraph.invalidateModule(entry);
            rebuild();
          }
        });
        current.middlewares.use(async (req, res, next) => {
          let path = "";
          try {
            path = decodeURIComponent(new URL(req.url!, "http://localhost").pathname);
            const origin = new URL(current.resolvedUrls!.local[0]!).origin;
            let content: string | Buffer | undefined,
              type = "text/javascript";
            const routes: [boolean, () => void | Promise<void>][] = [
              [
                path === "/",
                () => {
                  content = previewFrame({
                    title: input.declaration.metadata.title,
                    window,
                    capabilities,
                  });
                  type = "text/html";
                },
              ],
              [
                path === "/app.html",
                () => {
                  type = "text/html";
                  content = `<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"></head><body><script type="module" src="/__preview__/native.js?token=${hosts.token}"></script></body></html>`;
                },
              ],
              [
                path.startsWith("/__preview__/"),
                () => {
                  if (path === "/__preview__/resize.js") content = previewResizeScript;
                  if (path === "/__preview__/native.js") content = previewClient;
                },
              ],
              [
                path.startsWith("/__shell__/"),
                async () => {
                  const file = resolve(shellDirectory, path.slice(11));
                  if (!file.startsWith(shellDirectory + "/") || !file.endsWith(".js"))
                    throw new Error("Resource not exposed");
                  content = await readFile(file);
                },
              ],
              [
                path.startsWith("/assets/"),
                async () => {
                  const key = path.slice(8),
                    resource = input.resources.find((r) => r.kind === "app" && r.key === key);
                  if (!resource) {
                    res.statusCode = 404;
                    res.end();
                    return;
                  }
                  content = await readFile(join(stage, "stage", resource.path));
                  type = resource.mediaType;
                },
              ],
              [
                path.startsWith("/attachments/"),
                async () => {
                  const [, , token, id, ...rest] = path.split("/");
                  if (rest.length || !token || !new RegExp(AttachmentIdPattern).test(id ?? ""))
                    throw new Error("Invalid attachment URL");
                  const metadata = await hosts.resource(
                    token,
                    id!,
                    0,
                    0,
                    req.headers.range ?? null,
                  );
                  const plan = metadata.plan;
                  if (!plan || metadata.error) {
                    res.statusCode = 404;
                    res.end();
                    return;
                  }
                  res.statusCode = plan.status;
                  for (const [name, value] of Object.entries(plan.headers))
                    res.setHeader(name, value);
                  if (!plan.span || req.method === "HEAD") {
                    res.end();
                    return;
                  }
                  const data = await hosts.resource(token, id!, plan.span.offset, plan.span.length);
                  if (data.error || !data.bytes)
                    throw new Error(data.error ?? "Missing attachment");
                  res.end(Buffer.from(data.bytes, "base64"));
                },
              ],
            ];
            await routes.find(([matches]) => matches)?.[1]();
            if (res.writableEnded) return;
            if (content === undefined) {
              next();
              return;
            }
            res.setHeader("Content-Type", type);
            res.setHeader("Cache-Control", "no-store");
            res.setHeader("X-Content-Type-Options", "nosniff");
            res.setHeader(
              "Content-Security-Policy",
              path === "/"
                ? previewHostPolicy(origin)
                : appContentSecurityPolicy("browser", origin),
            );
            // Devices follow the declaration, as the native host admits them.
            if (path === "/" || path === "/app.html")
              res.setHeader("Permissions-Policy", capabilityPermissionsPolicy(capabilities));
            res.end(req.method === "HEAD" ? undefined : content);
          } catch (error) {
            if (path.startsWith("/attachments/")) {
              res.statusCode = 404;
              res.end();
            } else next(error as Error);
          }
        });
      },
      handleHotUpdate(ctx) {
        if (rebuilding || failed || dependencies.has(ctx.file)) return [];
      },
    };
    const config = await appConfig(source);
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
        fs: { strict: true, allow: [source, cliRoot] },
      },
    });
    signal?.throwIfAborted();
    await server.listen();
    signal?.throwIfAborted();
    const origin = new URL(server.resolvedUrls!.local[0]!).origin;
    return { url: origin + "/", close, diagnostics: () => hosts.diagnostics() };
  } catch (error) {
    await close();
    throw error;
  }
}
