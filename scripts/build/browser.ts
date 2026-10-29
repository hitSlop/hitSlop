import { cp, mkdtemp, rm } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { brotliCompressSync } from "node:zlib";
import { repository, publishFolder } from "../lib/artifacts";
import { buildBrowserWasm } from "./core";

/** Runtime served by the local CLI host; the app's native shell remains a separate artifact. */
export async function buildBrowser() {
  await buildBrowserWasm();
  const stage = await mkdtemp(join(tmpdir(), "tinys-browser-build-"));
  try {
    const source = join(repository, "packages/tinys/src/browser");
    const result = await Bun.build({
      entrypoints: ["host", "worker", "frame", "service-worker"].map(name => join(source, `${name}.ts`)),
      outdir: stage, target: "browser", minify: true, splitting: false,
    });
    if (!result.success) throw new Error(result.logs.join("\n"));
    await cp(join(source, "index.html"), join(stage, "index.html"));
    await cp(join(repository, "generated/browser/core"), join(stage, "core"), { recursive: true });
    const bytes = await Bun.file(join(stage, "core/tinys_core_wasm_bg.wasm")).arrayBuffer();
    console.log(`Browser core: ${(bytes.byteLength / 1e6).toFixed(2)} MB raw, ${(brotliCompressSync(bytes).length / 1e6).toFixed(2)} MB Brotli`);
    await publishFolder(join(repository, "packages/tinys/browser"), ready => cp(stage, ready, { recursive: true }));
  } finally { await rm(stage, { recursive: true, force: true }); }
}
if (import.meta.main) await buildBrowser();
