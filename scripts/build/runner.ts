import { writeIfChanged } from "../lib/artifacts";
import { readFile } from "node:fs/promises";
import { RuntimeABI } from "../../packages/hitslop/src/wire/constants.generated";
import { resolveCorpora, verifyPreludes } from "../compat/corpora";
import { releases } from "../compat/corpus";
const entries = { 1: "packages/hitslop/src/shell/abi/runner-1.ts" } satisfies Record<typeof RuntimeABI, string>;
// Explicit authoring tool. Ordinary builds embed the checked-in prelude unchanged.
// The compatibility gate verifies these bytes against every published corpus.
export async function buildRunner(check = false) {
  await resolveCorpora();
  const corpus = await releases();
  for (const { release } of corpus)
    if (release.frozen && release.producer.runner?.[RuntimeABI]) {
      if (check) { await verifyPreludes(corpus); return; }
      throw new Error(`ABI ${RuntimeABI} was released by ${release.release}; preserve its prelude and introduce a new ABI`);
    }
  const path = `crates/hitslop-runner/src/abi/${RuntimeABI}.generated.js`;
  const bundle = await Bun.build({ entrypoints: [entries[RuntimeABI]], target: "browser", format: "iife", minify: true, sourcemap: "none" });
  if (!bundle.success) throw new Error(bundle.logs.join("\n"));
  const output = await bundle.outputs[0]!.text();
  if (check) { if (await readFile(path, "utf8") !== output) throw new Error(`Generated drift: ${path}`); }
  else await writeIfChanged(path, output);
}
if (import.meta.main) await buildRunner(process.argv.includes("--check"));
