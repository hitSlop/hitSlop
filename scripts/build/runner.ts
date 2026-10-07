import { writeIfChanged } from "../lib/artifacts";
import { readFile } from "node:fs/promises";
import { RuntimeABI } from "../../packages/hitslop/src/wire/constants.generated";
const entries = { 1: "packages/hitslop/src/shell/abi/runner-1.ts" } satisfies Record<typeof RuntimeABI, string>;
export async function buildRunner(check = false) {
  const bundle = await Bun.build({ entrypoints: [entries[RuntimeABI]], target: "browser", format: "iife", minify: true, sourcemap: "none" });
  if (!bundle.success) throw new Error(bundle.logs.join("\n"));
  const path = `crates/hitslop-runner/src/abi/${RuntimeABI}.generated.js`, output = await bundle.outputs[0]!.text();
  if (check) { if (await readFile(path, "utf8") !== output) throw new Error(`Generated drift: ${path}`); }
  else await writeIfChanged(path, output);
}
if (import.meta.main) await buildRunner();
