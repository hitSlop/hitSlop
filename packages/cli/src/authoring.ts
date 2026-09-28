import { resolve, join } from "node:path";
import { mkdtemp, rm, mkdir, readFile } from "node:fs/promises";
import type { SlopManifest } from "@hitslop/schema";
import { tmpdir, homedir } from "node:os";
import { buildProject, shellDirectory } from "./build";
import { buildTemplate, prepareRenderer, installTemplate } from "./template";
export async function runAuthoring(
  command: "build" | "register" | "dev",
  target: string,
  port = 5173,
) {
  if (command === "build") {
    console.log(await buildTemplate(target, await prepareRenderer()));
  } else if (command === "register") {
    const output = await buildTemplate(target, await prepareRenderer());
    const manifest = JSON.parse(await readFile(join(output, "manifest.json"), "utf8"));
    const templates = join(homedir(), ".hitslop/templates");
    await mkdir(templates, { recursive: true });
    const destination = join(templates, manifest.slug + ".slop");
    await installTemplate(output, destination);
    console.log(destination);
  } else if (command === "dev") {
    if (!(await Bun.file(join(shellDirectory, "boot.js")).exists()))
      throw new Error("CLI page shell is missing. Reinstall @hitslop/cli.");
    const temporary = await mkdtemp(join(tmpdir(), "hitslop-preview-"));
    const out = await buildProject(target, join(temporary, "preview.slop"));
    const frame = previewFrame(JSON.parse(await readFile(join(out, "manifest.json"), "utf8")));
    const server = Bun.serve({
      hostname: "127.0.0.1",
      port,
      async fetch(request) {
        const url = new URL(request.url),
          relative = decodeURIComponent(url.pathname);
        if (relative === "/")
          return new Response(frame, {
            headers: {
              "Cache-Control": "no-store",
              "Content-Type": "text/html; charset=utf-8",
              "Content-Security-Policy":
                "default-src 'none'; style-src 'unsafe-inline'; img-src 'self'; frame-src 'self'",
            },
          });
        // The page shell owns the page, as in the native host (SchemeHandler.visiblePage).
        if (relative === "/app.html")
          return new Response(page, {
            headers: {
              "Cache-Control": "no-store",
              "Content-Type": "text/html; charset=utf-8",
              "Content-Security-Policy": appPolicy,
            },
          });
        const shell = relative.startsWith("/__shell__/");
        const base = shell ? shellDirectory : out;
        const path = resolve(base, relative.slice(shell ? "/__shell__/".length : 1));
        if (!path.startsWith(base + "/")) return new Response("Forbidden", { status: 403 });
        const file = Bun.file(path);
        if (!(await file.exists())) return new Response("Not found", { status: 404 });
        return new Response(file, {
          headers: { "Cache-Control": "no-store", "Content-Security-Policy": appPolicy },
        });
      },
    });
    console.log(`Disposable preview: ${server.url}`);
    process.on("SIGINT", async () => {
      server.stop();
      await rm(temporary, { recursive: true, force: true });
      process.exit(0);
    });
  }
}

const page =
  '<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>hitSlop</title><link rel="stylesheet" href="/assets/app.css"></head><body><script type="module" src="/__shell__/boot.js"></script></body></html>';
const appPolicy =
  "default-src 'none'; script-src 'self' 'wasm-unsafe-eval'; connect-src 'self' https: blob:; media-src 'self' https: blob:; style-src 'self' 'unsafe-inline'; img-src 'self' data: https: blob:; font-src 'self'";

/** Stands in for the native window: manifest size, shape mask, and backing or skin chrome. */
export function previewFrame(manifest: Pick<SlopManifest, "title" | "presentation">) {
  const presentation = manifest.presentation;
  const { width, height } = presentation;
  const window = [`width:${width}px`, `height:${height}px`];
  if ("skin" in presentation)
    window.push(
      ...["background", "-webkit-mask", "mask"].map(
        (property) => `${property}:url("/${presentation.skin}") 0 0/100% 100% no-repeat`,
      ),
    );
  else {
    const { shape = "rounded", background, resizable = true } = presentation;
    window.push(
      `border-radius:${shape === "ellipse" ? "50%" : shape === "capsule" ? "9999px" : "min(22px,50%)"}`,
    );
    if (background !== "transparent")
      window.push("background:Canvas", "box-shadow:0 18px 50px #0004,0 0 0 .5px #0003");
    if (resizable) window.push("resize:both", "min-width:240px", "min-height:180px");
    if (shape === "ellipse" && width === height) window.push("aspect-ratio:1");
  }
  const title = manifest.title.replace(/[&<>"]/g, (c) => `&#${c.charCodeAt(0)};`);
  return `<!doctype html><html lang="en"><head><meta charset="utf-8"><title>${title} preview</title><style>html,body{margin:0;height:100%}body{display:grid;place-items:center;background:#e4e4e4 repeating-conic-gradient(#d2d2d2 0 25%,#e4e4e4 0 50%) 0 0/24px 24px}.window{overflow:hidden;${window.join(";")}}iframe{display:block;width:100%;height:100%;border:0}</style></head><body><div class="window"><iframe src="/app.html" title="${title}"></iframe></div></body></html>`;
}
