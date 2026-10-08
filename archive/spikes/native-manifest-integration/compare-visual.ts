import { readFile, writeFile } from "node:fs/promises";
import { createHash } from "node:crypto";
import { resolve, join } from "node:path";
const out = resolve(import.meta.dir, "../../generated/native-manifest-integration");
const results = [];
for (const kind of ["washer", "washer-fallback"]) {
  for (const variant of ["baseline-visual", "visual"]) {
    const base = join(out, variant, kind);
    const child = Bun.spawn(
      [
        "/opt/homebrew/bin/pdftoppm",
        "-png",
        "-singlefile",
        "-scale-to",
        "640",
        `${base}.pdf`,
        `${base}-pdf`,
      ],
      { stdout: "inherit", stderr: "inherit" },
    );
    if (await child.exited) throw new Error(`Cannot render ${base}.pdf`);
  }
  for (const suffix of [".png", "-pdf.png"]) {
    const baseline = await readFile(join(out, "baseline-visual", kind + suffix));
    const candidate = await readFile(join(out, "visual", kind + suffix));
    results.push({
      file: kind + suffix,
      identical: baseline.equals(candidate),
      baselineSHA256: createHash("sha256").update(baseline).digest("hex"),
      candidateSHA256: createHash("sha256").update(candidate).digest("hex"),
    });
  }
}
await writeFile(join(out, "visual-comparison.json"), JSON.stringify(results, null, 2));
console.log(JSON.stringify(results, null, 2));
if (results.some((r) => !r.identical))
  throw new Error("Export comparison differs; inspect the images");
