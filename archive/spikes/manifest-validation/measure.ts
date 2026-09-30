import { readFile, stat, writeFile } from "node:fs/promises";
import { join, resolve } from "node:path";
import { gzipSync } from "node:zlib";

const root = resolve(import.meta.dir, "../..");
const out = join(root, "generated/manifest-validation-spike");
const cases: {name: string; input: string; typebox: boolean}[] = JSON.parse(await readFile(join(out, "cases.json"), "utf8"));
if (process.argv[2] === "--wasm-child") {
  const dir = join(out, process.argv[3]!, "wasm");
  const wasm = new Uint8Array(await readFile(join(dir, "hitslop_core_wasm_bg.wasm")));
  const start = performance.now();
  const module = await import(join(dir, "hitslop_core_wasm.js"));
  module.initSync({ module: wasm });
  const init_ms = performance.now() - start;
  const coldStart = performance.now();
  const input = cases[Number(process.argv[4] ?? 0)]!.input;
  const cold = module.manifestProbe(input);
  const cold_us = (performance.now() - coldStart) * 1000;
  const warmStart = performance.now();
  for (let i = 0; i < 1000; i++) module.manifestProbe(input);
  const warm_us = (performance.now() - warmStart);
  const results = cases.map(c => ({ name: c.name, result: JSON.parse(module.manifestProbe(c.input)) }));
  // Keep the production adapter present and exercise its existing descriptor path.
  module.validate('{"kind":"object","properties":{"done":{"kind":"boolean"}}}', '{"done":false}');
  console.log(JSON.stringify({ init_ms, cold_us, warm_us, cold_result: cold, results }));
  process.exit(0);
}
async function json(args: string[]) {
  const child = Bun.spawn(args, { stdout: "pipe", stderr: "pipe" });
  const [output, error, code] = await Promise.all([new Response(child.stdout).text(), new Response(child.stderr).text(), child.exited]);
  if (code) throw new Error(`${args.join(" ")}\n${error}`);
  return JSON.parse(output);
}
function summary(values: number[]) {
  const sorted = [...values].sort((a,b) => a-b);
  return { p50: sorted[Math.floor(sorted.length / 2)], p95: sorted[Math.ceil(sorted.length * .95)-1], min: sorted[0], max: sorted.at(-1) };
}
const report: Record<string, any> = { date: new Date().toISOString(), samples: 20, cases: cases.length, variants: {} };
const swiftSamples = [];
for (let i = 0; i < report.samples; i++) swiftSamples.push(await json([join(out, "swift-probe"), join(out, "workspace/manifest.schema.json"), join(out, "cases.json")]));
const swift = swiftSamples[0].results;
report.swift = { cold_us: summary(swiftSamples.map(s=>s.cold_us)), warm_us: summary(swiftSamples.map(s=>s.warm_us)) };
report.swiftVsTypeBox = cases.flatMap((c, i) => swift[i].valid === c.typebox ? [] : [{name:c.name, typebox:c.typebox, swift:swift[i].valid}]);
const original = await import(join(root, "generated/core/wasm/hitslop_core_wasm.js"));
original.initSync({ module: new Uint8Array(await readFile(join(root, "generated/core/wasm/hitslop_core_wasm_bg.wasm"))) });
const expected = cases.map(c => {
  if (!c.typebox) return false;
  const { presentation:p } = JSON.parse(c.input);
  if (p.skin !== undefined) return true;
  try { original.validateWindowShape(p.shape === undefined ? undefined : JSON.stringify(p.shape), p.width, p.height); return true; }
  catch { return false; }
});
for (const mode of ["baseline", "runtime", "compiled"]) {
  const dir = join(out, mode);
  const nativeSamples = [], wasmSamples = [];
  for (let i = 0; i < report.samples; i++) {
    nativeSamples.push(await json([join(dir, "manifest-bench"), join(out, "cases.json")]));
    wasmSamples.push(await json([process.execPath, import.meta.path, "--wasm-child", mode]));
  }
  const wasmBytes = await readFile(join(dir, "wasm/hitslop_core_wasm_bg.wasm"));
  const first = nativeSamples[0];
  const nativeWasmDifferences = cases.flatMap((c,i) => {
    const n = first.results[i].result, w = wasmSamples[0].results[i].result;
    return n.schemaValid === w.schemaValid && n.valid === w.valid ? [] : [{name:c.name, native:n, wasm:w}];
  });
  report.variants[mode] = {
    wasmBytes: wasmBytes.length, wasmGzipBytes: gzipSync(wasmBytes).length,
    nativeDylibBytes: (await stat(join(dir, "libhitslop_core_ffi.dylib"))).size,
    native: { cold_us: summary(nativeSamples.map(s=>s.cold_us)), warm_us: summary(nativeSamples.map(s=>s.warm_us)) },
    wasm: { init_ms: summary(wasmSamples.map(s=>s.init_ms)), cold_us: summary(wasmSamples.map(s=>s.cold_us)), warm_us: summary(wasmSamples.map(s=>s.warm_us)) },
    nativeWasmDifferences,
    typeboxDifferences: cases.flatMap((c,i) => first.results[i].result.schemaValid === c.typebox ? [] : [{name:c.name, typebox:c.typebox, rust:first.results[i].result}]),
    completeDifferences: cases.flatMap((c,i) => first.results[i].result.valid === expected[i] ? [] : [{name:c.name, expected:expected[i], rust:first.results[i].result}]),
    results: first.results,
    samples: { native: nativeSamples.map(({results,...sample})=>sample), wasm: wasmSamples.map(({results,...sample})=>sample) },
  };
  report.variants[mode].additionalColdCases = {};
  for (const [scenario, name] of [["radius", 'shape-"5px"'], ["skin", 'skin-"assets/skin.png"']]) {
    const index = cases.findIndex(c => c.name === name);
    if (index < 0) throw new Error(`Missing timing case ${name}`);
    const file = join(out, `timing-${scenario}.json`);
    await writeFile(file, JSON.stringify([cases[index]]));
    const native = [], wasm = [];
    for (let i = 0; i < report.samples; i++) {
      native.push(await json([join(dir, "manifest-bench"), file]));
      wasm.push(await json([process.execPath, import.meta.path, "--wasm-child", mode, String(index)]));
    }
    report.variants[mode].additionalColdCases[scenario] = {
      native_us: summary(native.map(s=>s.cold_us)), wasm_us: summary(wasm.map(s=>s.cold_us)),
    };
  }
  console.log(`${mode}: ${wasmBytes.length} WASM bytes; ${report.variants[mode].typeboxDifferences.length} schema differences; ${nativeWasmDifferences.length} native/WASM differences`);
}
await writeFile(join(out, "results.json"), JSON.stringify(report, null, 2) + "\n");
console.log(`Results: ${join(out, "results.json")}`);
await import("./summarize");
