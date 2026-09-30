import { copyFile, cp, mkdir, readFile, writeFile, stat } from "node:fs/promises";
import { resolve, join } from "node:path";
const root = resolve(import.meta.dir, "../..");
const out = join(root, "generated/native-manifest-integration");
const workspace = join(out, "workspace");
const variant = process.argv[2];
if (!["baseline", "candidate"].includes(variant)) throw new Error("measure.ts baseline|candidate");
const directory = join(out, variant);
await mkdir(directory, { recursive: true });
async function run(command: string[]) {
  const child = Bun.spawn(command, { cwd: workspace, stdout: "pipe", stderr: "inherit" });
  const stdout = await new Response(child.stdout).text();
  if (await child.exited) throw new Error(command.join(" "));
  return stdout;
}
const release = join(workspace, "apps/apple/Packages/HitSlopApple/.build/release");
await cp(
  join(release, "HitSlopApple_HitSlopDocument.bundle"),
  join(directory, "HitSlopApple_HitSlopDocument.bundle"),
  { recursive: true },
);
const probe = join(directory, "manifest-integration-probe");
await copyFile(join(release, "manifest-integration-probe"), probe);
const helper = join(directory, "hitslop-native");
await copyFile(join(release, "hitslop-native"), helper);
await copyFile(helper, `${helper}.stripped`);
await run(["/usr/bin/strip", "-S", "-x", `${helper}.stripped`]);
const wasm = join(workspace, "generated/core/wasm/hitslop_core_wasm_bg.wasm");
await copyFile(wasm, join(directory, "core.wasm"));
const casesPath = join(root, "generated/manifest-validation-spike/cases.json");
const results = JSON.parse(await run([probe, casesPath, join(out, "packages")]));
await writeFile(join(directory, "corpus.json"), JSON.stringify(results, null, 2));
const scenarios = ["baseline", 'shape-"5px"', 'skin-"assets/skin.png"'];
const timings: Record<string, unknown[]> = {};
for (const name of scenarios) {
  const row = results.find((r: any) => r.name === name);
  if (!row?.packageAcceptance) throw new Error(`Cannot benchmark ${name}`);
  timings[name] = [];
  for (let sample = 0; sample < 20; sample++)
    timings[name].push(JSON.parse(await run([probe, "--time", row.package])));
}
const measurements = {
  variant,
  measuredAt: new Date().toISOString(),
  helperBytes: (await stat(helper)).size,
  helperStrippedBytes: (await stat(`${helper}.stripped`)).size,
  wasmBytes: (await stat(wasm)).size,
  timings,
};
await writeFile(join(directory, "measurements.json"), JSON.stringify(measurements, null, 2));
console.log(
  JSON.stringify(
    { ...measurements, timings: "20 fresh processes × (1 cold + 100 warm opens) per scenario" },
    null,
    2,
  ),
);
