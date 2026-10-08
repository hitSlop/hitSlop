import { readFile, writeFile } from "node:fs/promises";
import { resolve, join } from "node:path";

const out = resolve(import.meta.dir, "../../generated/manifest-validation-spike");
const report = JSON.parse(await readFile(join(out, "results.json"), "utf8"));
const sha256 = async (file: string) => new Bun.CryptoHasher("sha256").update(await readFile(file)).digest("hex");
const summary = {
  date: report.date,
  cases: report.cases,
  samplesPerScenario: report.samples,
  schemaSHA256: await sha256(join(out, "workspace/manifest.schema.json")),
  lockSHA256: await sha256(join(out, "workspace/Cargo.lock")),
  swift: report.swift,
  swiftVsTypeBox: report.swiftVsTypeBox,
  variants: {} as Record<string, unknown>,
};
for (const [mode, value] of Object.entries(report.variants) as [string, any][]) {
  const { results, samples, typeboxDifferences, completeDifferences, ...measurements } = value;
  const dependencies = (await readFile(join(out, mode, "dependencies.txt"), "utf8")).split("\n")
    .flatMap(line => { const match = line.match(/^(\S+) v(\S+)/); return match ? [`${match[1]} ${match[2]}`] : []; });
  summary.variants[mode] = {
    ...measurements,
    wasmSHA256: await sha256(join(out, mode, "wasm/hitslop_core_wasm_bg.wasm")),
    dependencies: [...new Set(dependencies)].sort(),
    ...(mode === "baseline" ? { note: "Control omits schema validation; not an acceptance candidate." } : { typeboxDifferences, completeDifferences }),
  };
}
await writeFile(join(import.meta.dir, "measurements.json"), JSON.stringify(summary, null, 2) + "\n");
console.log("Saved durable measurement summary");
