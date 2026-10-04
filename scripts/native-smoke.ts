/** Black-box template coverage: no template selectors, actions, or business logic. */
import { strict as assert } from "node:assert";
import { mkdir, mkdtemp, readdir, readFile, rm, stat, writeFile } from "node:fs/promises";
import { join, resolve } from "node:path";
import { tmpdir } from "node:os";
import { builtTemplates } from "./templates";
import { digest, fileDigest, useTestRegistry } from "./runtime-artifacts";
import { createDocument, documentFromStage, nativeRequest } from "./fixture-documents";
import { nativeFixtureSlugs, prepareNativeFixtures } from "./native-fixtures";
useTestRegistry();

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
/** A built template file, or a fixture's build stage. */
const checksum = async (source: string) =>
  (await stat(source)).isDirectory() ? digest(source) : fileDigest(source);
type Stage = "initialRead" | "png" | "pdf" | "finalRead";
type RenderResult = {
  name: string;
  sha256: string;
  passed: boolean;
  seconds: Partial<Record<Stage | "total", number>>;
};
/** One helper request, timed as `stage`. */
async function run(body: Record<string, unknown>, result: RenderResult, stage: Stage) {
  const started = performance.now();
  try {
    const reply = await nativeRequest(helper, body);
    assert.ok(reply.ok, `${body.method} (${((performance.now() - started) / 1000).toFixed(1)}s): ${reply.error}`);
    return reply;
  } finally {
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
    const before = await checksum(source);
    const result: RenderResult = { name, sha256: before, passed: false, seconds: {} };
    results.push(result);
    try {
      if ((await stat(source)).isDirectory()) await documentFromStage(source, root, helper);
      else await createDocument(helper, source, root);
      const state = (await run({ method: "get", documentPath: root }, result, "initialRead")).state;
      for (const format of ["png", "pdf"] as const) {
        const output = join(evidence, `${name}.${format}`);
        await run({ method: "export", documentPath: root, format, output }, result, format);
        const bytes = await Bun.file(output).bytes();
        assert(bytes.length > 100, `Empty ${format}: ${name}`);
        assert.equal(
          Buffer.from(bytes.subarray(0, format === "png" ? 8 : 4)).toString("hex"),
          format === "png" ? "89504e470d0a1a0a" : "25504446",
        );
      }
      assert.deepEqual((await run({ method: "get", documentPath: root }, result, "finalRead")).state, state);
      assert.equal(await checksum(source), before, `Master changed: ${name}`);
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
