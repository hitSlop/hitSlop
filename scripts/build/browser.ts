import { cp, mkdtemp, rm } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { brotliCompressSync } from "node:zlib";
import { repository, publishFolder } from "../lib/artifacts";
import { buildBrowserWasm } from "./core";

/** Runtime served by the local CLI host; the app's native shell remains a separate artifact. */
export async function buildBrowser() {
  await buildBrowserWasm();
  const stage = await mkdtemp(join(tmpdir(), "hitslop-browser-build-"));
  try {
    const source = join(repository, "packages/hitslop/src/browser");
    const result = await Bun.build({
      entrypoints: ["host", "worker", "evaluator", "frame", "service-worker"].map(name => join(source, `${name}.ts`)),
      outdir: stage, target: "browser", minify: true, splitting: false,
    });
    if (!result.success) throw new Error(result.logs.join("\n"));
    await cp(join(source, "index.html"), join(stage, "index.html"));
    for (const kind of ["core", "evaluator"]) {
      await cp(join(repository, "generated/browser", kind), join(stage, kind), { recursive: true });
      const file = Bun.file(join(stage, kind, "hitslop_core_wasm_bg.wasm"));
      const bytes = await file.arrayBuffer();
      console.log(`Browser ${kind}: ${(bytes.byteLength / 1e6).toFixed(2)} MB raw, ${(brotliCompressSync(bytes).length / 1e6).toFixed(2)} MB Brotli`);
    }
    await publishFolder(join(repository, "packages/hitslop/browser"), ready => cp(stage, ready, { recursive: true }));
  } finally { await rm(stage, { recursive: true, force: true }); }
}
if (import.meta.main) await buildBrowser();
