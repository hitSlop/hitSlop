/** Black-box package coverage: no template selectors, actions, or business logic. */
import { strict as assert } from "node:assert";
import { cp, mkdir, mkdtemp, readdir, rm, writeFile } from "node:fs/promises";
import { join, resolve } from "node:path";
import { tmpdir } from "node:os";
import { builtTemplates } from "./templates";
import { digest } from "./runtime-artifacts";
import { nativeFixtureSlugs, prepareNativeFixtures } from "./native-fixtures";

const helper = resolve("apps/apple/Packages/HitSlopApple/.build/debug/hitslop-native");
const evidence = resolve(".hitslop/evidence/render");
const parent = await mkdtemp(join(tmpdir(), "hitslop-native-smoke-"));
// Everyday CI renders the native fixtures; releases render every bundled template.
const fixtures = process.argv.includes("--fixtures");
if (fixtures) await prepareNativeFixtures();
const packages = fixtures
  ? nativeFixtureSlugs.map((slug) => ({
      name: `fixture-${slug}`,
      source: resolve("generated/native-fixtures", `${slug}.slop`),
    }))
  : (await builtTemplates()).templates
      .filter((t) => t.bundled)
      .map((t) => ({
        name: `bundled-${t.slug}`,
        source: resolve("generated/templates", `${t.slug}.slop`),
      }));
for (const name of (await readdir("tests/fixtures")).sort())
  packages.push({ name: `fixture-${name}`, source: resolve("tests/fixtures", name, "document") });
type Stage = "initialRead" | "png" | "pdf" | "finalRead";
type RenderResult = {
  name: string;
  sha256: string;
  passed: boolean;
  seconds: Partial<Record<Stage | "total", number>>;
};
async function run(args: string[], result: RenderResult, stage: Stage) {
  const started = performance.now();
  const child = Bun.spawn([helper, ...args], { stdout: "pipe", stderr: "pipe" });
  const timeout = setTimeout(() => child.kill(), 60_000);
  try {
    const [out, error, code] = await Promise.all([
      new Response(child.stdout).text(),
      new Response(child.stderr).text(),
      child.exited,
    ]);
    assert.equal(
      code,
      0,
      `${args.join(" ")} (${((performance.now() - started) / 1000).toFixed(1)}s): ${error}`,
    );
    return out;
  } finally {
    clearTimeout(timeout);
    result.seconds[stage] = (performance.now() - started) / 1000;
  }
}
const results: RenderResult[] = [];
const sweepStarted = performance.now();
let failure: string | undefined;
try {
  await mkdir(evidence, { recursive: true });
  for (const { name, source } of packages) {
    const started = performance.now();
    console.log(`Checking ${name}`);
    const root = join(parent, `${name}.slop`);
    const before = await digest(source);
    const result: RenderResult = { name, sha256: before, passed: false, seconds: {} };
    results.push(result);
    try {
      await cp(source, root, { recursive: true });
      const state = JSON.parse(await run(["get", root], result, "initialRead"));
      for (const format of ["png", "pdf"] as const) {
        const output = join(evidence, `${name}.${format}`);
        await run(["export", root, "--format", format, "--output", output], result, format);
        const bytes = await Bun.file(output).bytes();
        assert(bytes.length > 100, `Empty ${format}: ${name}`);
        assert.equal(
          Buffer.from(bytes.subarray(0, format === "png" ? 8 : 4)).toString("hex"),
          format === "png" ? "89504e470d0a1a0a" : "25504446",
        );
      }
      assert.deepEqual(JSON.parse(await run(["get", root], result, "finalRead")), state);
      assert.equal(await digest(source), before, `Master changed: ${name}`);
      result.passed = true;
      console.log(
        `PASS ${name}: native open, authored render, PNG/PDF, reopen (${((performance.now() - started) / 1000).toFixed(1)}s)`,
      );
    } finally {
      result.seconds.total = (performance.now() - started) / 1000;
    }
  }
} catch (error) {
  failure = String(error);
  throw error;
} finally {
  const seconds = {
    initialRead: 0,
    png: 0,
    pdf: 0,
    finalRead: 0,
    total: (performance.now() - sweepStarted) / 1000,
  };
  for (const result of results)
    for (const stage of ["initialRead", "png", "pdf", "finalRead"] as const)
      seconds[stage] += result.seconds[stage] ?? 0;
  console.log(`Render timings (seconds): ${JSON.stringify(seconds)}`);
  await writeFile(
    join(evidence, "results.json"),
    JSON.stringify({ passed: !failure, seconds, results, error: failure }, null, 2) + "\n",
  );
  await rm(parent, { recursive: true, force: true });
}
