import { copyFile, mkdir, readFile, writeFile } from "node:fs/promises";
import { resolve, join, dirname } from "node:path";
import { createHash } from "node:crypto";
import { parseManifest } from "../../packages/schema/src/manifest";
const root = resolve(import.meta.dir, "../..");
const out = join(root, "generated/native-manifest-integration");
const workspace = join(out, "workspace");
const json = async (path: string) => JSON.parse(await readFile(path, "utf8"));
const cases = await json(join(root, "generated/manifest-validation-spike/cases.json"));
const original = await import(join(root, "generated/core/wasm/hitslop_core_wasm.js"));
original.initSync({ module: new Uint8Array(await readFile(join(out, "baseline/core.wasm"))) });
const baseline = await json(join(out, "baseline/corpus.json"));
const candidate = await json(join(out, "candidate/corpus.json"));
// Existing schema/native-path disagreements established before the integration.
const unsafeSkins = new Set(["assets/../skin.png", "assets/a//skin.png", "assets/a/./skin.png"]);
const results = cases.map((item: any, i: number) => {
  const manifest = JSON.parse(item.input);
  let schema = true;
  try {
    parseManifest(manifest);
  } catch {
    schema = false;
  }
  let expected = schema;
  if (expected && manifest.presentation.skin === undefined) {
    const p = manifest.presentation;
    try {
      original.validateWindowShape(
        p.shape === undefined ? undefined : JSON.stringify(p.shape),
        p.width,
        p.height,
      );
    } catch {
      expected = false;
    }
  }
  if (baseline[i].name !== item.name || candidate[i].name !== item.name)
    throw new Error("Corpus order changed");
  const { package: _baselinePath, ...b } = baseline[i];
  const { package: _candidatePath, ...c } = candidate[i];
  return {
    name: item.name,
    typebox: schema,
    expectedNativeAcceptance: expected,
    expectedPackageAcceptance: expected && !unsafeSkins.has(manifest.presentation?.skin),
    baseline: b,
    candidate: c,
  };
});
const percentile = (values: number[], p: number) =>
  [...values].sort((a, b) => a - b)[Math.ceil(values.length * p) - 1];
const measurements: Record<string, any> = {};
for (const variant of ["baseline", "candidate"]) {
  const m = await json(join(out, variant, "measurements.json"));
  measurements[variant] = {
    ...m,
    summary: Object.fromEntries(
      Object.entries(m.timings).map(([name, samples]: [string, any]) => [
        name,
        Object.fromEntries(
          ["cold_ms", "warm_ms"].map((key) => [
            key,
            {
              p50: percentile(
                samples.map((s: any) => s[key]),
                0.5,
              ),
              p95: percentile(
                samples.map((s: any) => s[key]),
                0.95,
              ),
            },
          ]),
        ),
      ]),
    ),
  };
}
const changed: string[] = await json(join(out, "candidate-files.json"));
for (const path of changed) {
  await mkdir(dirname(join(out, "after", path)), { recursive: true });
  await copyFile(join(workspace, path), join(out, "after", path));
}
async function diff(extra: string[] = []) {
  const process = Bun.spawn(["git", "diff", "--no-index", ...extra, "--", "before", "after"], {
    cwd: out,
    stdout: "pipe",
    stderr: "inherit",
  });
  const output = await new Response(process.stdout).text();
  if ((await process.exited) > 1) throw new Error("Cannot create candidate patch");
  return output
    .replaceAll("a/before/", "a/")
    .replaceAll("b/after/", "b/")
    .replaceAll("a/after/", "a/");
}
const patch = await diff();
await writeFile(join(import.meta.dir, "candidate.patch"), patch);
const diffStats = await diff(["--numstat"]);
const wasmNormalTree = await readFile(join(out, "logs/wasm-normal-tree.stdout"), "utf8");
const wasmFeatureTree = await readFile(join(out, "logs/wasm-feature-tree.stdout"), "utf8");
const featureWasm = await readFile(join(out, "feature-wasm/hitslop_core_wasm_bg.wasm"));
const verification: Record<string, unknown> = {};
for (const label of [
  "baseline-regressions",
  "candidate-rust",
  "candidate-build",
  "candidate-check-final",
  "candidate-test",
  "candidate-swift",
  "candidate-core-final",
  "candidate-native",
  "candidate-restored",
  "candidate-visual",
  "wasm-feature-build",
]) {
  verification[label] = await json(join(out, `logs/${label}.json`));
}
const report = {
  measuredAt: new Date().toISOString(),
  methodology:
    "Same isolated workspace, Release helper, strip -S -x. Twenty fresh processes per scenario, first SlopPackage init timed then 100 repeated opens averaged. Warm filesystem caches; process launch and WebKit excluded.",
  measurements,
  verification,
  visualComparison: await json(join(out, "visual-comparison.json")),
  wasmFeatureCheck: {
    sameDependencyTree: wasmNormalTree === wasmFeatureTree,
    includesJsonschema: /jsonschema/.test(wasmNormalTree + wasmFeatureTree),
    featureBytes: featureWasm.length,
    featureSHA256: createHash("sha256").update(featureWasm).digest("hex"),
  },
  nativeParityDifferences: results.filter(
    (r: any) => r.expectedNativeAcceptance !== r.candidate.nativeAcceptance,
  ),
  baselineSchemaDifferences: results
    .filter((r: any) => r.typebox !== r.baseline.schemaAcceptance)
    .map((r: any) => r.name),
  packageParityDifferences: results.filter(
    (r: any) => r.expectedPackageAcceptance !== r.candidate.packageAcceptance,
  ),
  packageAcceptanceChanges: results
    .filter((r: any) => r.baseline.packageAcceptance !== r.candidate.packageAcceptance)
    .map((r: any) => r.name),
  nativeAcceptedButPackageRejected: results.filter(
    (r: any) => r.candidate.nativeAcceptance && !r.candidate.packageAcceptance,
  ),
  decodedTitleDifferences: results.filter(
    (r: any) =>
      r.candidate.packageAcceptance &&
      r.candidate.decodedTitle !==
        JSON.parse(cases.find((c: any) => c.name === r.name).input).title,
  ),
  diffStats,
  results,
};
await writeFile(join(import.meta.dir, "measurements.json"), JSON.stringify(report, null, 2) + "\n");
console.log(
  JSON.stringify(
    {
      ...report,
      measurements: Object.fromEntries(
        Object.entries(measurements).map(([k, m]) => [k, { ...m, timings: undefined }]),
      ),
      results: undefined,
    },
    null,
    2,
  ),
);
if (
  report.nativeParityDifferences.length ||
  report.packageParityDifferences.length ||
  report.decodedTitleDifferences.length
)
  throw new Error("Candidate parity failed");
if (
  !report.wasmFeatureCheck.sameDependencyTree ||
  report.wasmFeatureCheck.includesJsonschema ||
  featureWasm.length !== measurements.candidate.wasmBytes
)
  throw new Error("Native validator leaked into WASM");
