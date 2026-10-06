import { writeIfChanged } from "../lib/artifacts";
import { readFile } from "node:fs/promises";
export async function buildRunner(check = false) {
  const bundle = await Bun.build({ entrypoints: ["packages/hitslop/src/shell/runner.ts"], target: "browser", format: "iife", minify: true, sourcemap: "none" });
  if (!bundle.success) throw new Error(bundle.logs.join("\n"));
  const path = "crates/slop-engine/src/runner/prelude.generated.js", output = await bundle.outputs[0]!.text();
  if (check) { if (await readFile(path, "utf8") !== output) throw new Error(`Generated drift: ${path}`); }
  else await writeIfChanged(path, output);
}
if (import.meta.main) await buildRunner();
