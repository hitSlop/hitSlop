import { writeIfChanged, sha256 } from "../lib/artifacts";
import { readFile, readdir } from "node:fs/promises";
import { RuntimeABI } from "../../packages/hitslop/src/wire/constants.generated";
const entries = { 1: "packages/hitslop/src/shell/abi/runner-1.ts" } satisfies Record<typeof RuntimeABI, string>;
/** The prelude a frozen corpus release ran, by ABI. A released prelude's bytes are final:
 * it is verified, never rebuilt, so later SDK or bundler changes cannot alter what old
 * command programs run against. A change for new apps raises RuntimeABI. */
async function frozen(abi: number) {
  for (const release of await readdir("tests/compat")) {
    const record = JSON.parse(await readFile(`tests/compat/${release}/release.json`, "utf8").catch(() => "{}"));
    const digest = record.frozen ? record.producer?.runner?.[abi] : undefined;
    if (digest) return { release, digest: digest as string };
  }
}
export async function buildRunner(check = false) {
  const path = `crates/hitslop-runner/src/abi/${RuntimeABI}.generated.js`;
  const released = await frozen(RuntimeABI);
  if (released) {
    if (sha256(await readFile(path)) !== released.digest)
      throw new Error(`${path} is frozen by ${released.release}; restore it and raise RuntimeABI for the change`);
    return;
  }
  const bundle = await Bun.build({ entrypoints: [entries[RuntimeABI]], target: "browser", format: "iife", minify: true, sourcemap: "none" });
  if (!bundle.success) throw new Error(bundle.logs.join("\n"));
  const output = await bundle.outputs[0]!.text();
  if (check) { if (await readFile(path, "utf8") !== output) throw new Error(`Generated drift: ${path}`); }
  else await writeIfChanged(path, output);
}
if (import.meta.main) await buildRunner();
