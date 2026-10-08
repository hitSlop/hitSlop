// Use the frozen baseline's existing WASM validator to check the rejected adapter's corpus.
import { join } from "node:path";
import { writeFile, mkdir } from "node:fs/promises";
const { validateWindowShape } = await import(join(process.cwd(), "packages/cli/src/core.ts"));
const sources = ["M,0 0", "M0 0,", "M0 0,Z", "M0 0Z 1 1", "M0 0A-20 -10 30 1 0 40 40Z", "M0 0A10 10 0 0 1 0 0Z", "M0 0L1e309 0", "M0 0A1e309 1 0 0 1 0 0"];
for (const radius of [50, 16384, 1e12, 1e22]) sources.push(`M0 0A${radius} ${radius} 0 0 1 ${radius * 2} 0`);
const results = [];
for (const source of sources) {
  let accepted = true;
  try { await validateWindowShape({ width: 100, height: 100, shape: { path: source } }); }
  catch { accepted = false; }
  results.push({ source, accepted });
}
await mkdir(".hitslop", { recursive: true });
await writeFile(".hitslop/boundary-shape-baseline.json", JSON.stringify(results, null, 2));
console.log(JSON.stringify(results, null, 2));
