import { cp, mkdir, readFile } from "node:fs/promises";
import { resolve, join } from "node:path";
const out = resolve(import.meta.dir, "../../generated/native-manifest-integration");
const workspace = join(out, "workspace");
const baseline = process.argv.includes("--baseline");
const evidence = join(out, baseline ? "baseline-visual" : "visual");
const helper = baseline
  ? join(out, "baseline/hitslop-native")
  : join(workspace, "apps/apple/Packages/HitSlopApple/.build/debug/hitslop-native");
await mkdir(evidence, { recursive: true });
for (const kind of ["washer", "washer-fallback"]) {
  const source = join(workspace, "generated/shape-lab", `${kind}.slop`);
  const document = join(evidence, `${kind}.slop`);
  await cp(source, document, { recursive: true, errorOnExist: true, force: false });
  for (const format of ["png", "pdf"]) {
    const output = join(evidence, `${kind}.${format}`);
    const child = Bun.spawn([helper, "export", document, "--format", format, "--output", output], {
      cwd: workspace,
      stdout: "inherit",
      stderr: "inherit",
    });
    if (await child.exited) throw new Error(`Export failed: ${kind} ${format}`);
    const bytes = await readFile(output);
    if (
      bytes.length < 100 ||
      bytes.subarray(0, format === "png" ? 8 : 4).toString("hex") !==
        (format === "png" ? "89504e470d0a1a0a" : "25504446")
    ) {
      throw new Error(`Invalid export: ${output}`);
    }
    console.log(output);
  }
}
