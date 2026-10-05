// Black-box template coverage: no template selectors, actions or business logic. Each
// template opens natively, renders its authored PNG and PDF, and reads the same afterwards,
// with its master unchanged. Everyday runs render the native fixtures; a release
// (HITSLOP_RENDER=all) renders every bundled template. Evidence: .hitslop/evidence/render.
import { afterAll, beforeAll, expect, test } from "bun:test";
import { mkdir, mkdtemp, readdir, rm, stat, writeFile } from "node:fs/promises";
import { join, resolve } from "node:path";
import { tmpdir } from "node:os";
import { builtTemplates } from "../../scripts/templates/discover";
import { digest, fileDigest, useTestRegistry } from "../../scripts/lib/artifacts";
import { assertExport, createDocument, documentFromStage, helperRequest } from "../../scripts/lib/helper";
import { nativeFixtureSlugs, prepareNativeFixtures } from "../../scripts/lib/native-fixtures";
useTestRegistry();

const evidence = resolve(".hitslop/evidence/render");
const all = process.env.HITSLOP_RENDER === "all";
const packages = all
  ? (await builtTemplates()).templates
      .filter((template) => template.bundled)
      .map((template) => ({ name: `bundled-${template.slug}`, source: resolve("generated/templates", `${template.slug}.slop`) }))
  : nativeFixtureSlugs.map((slug) => ({ name: `fixture-${slug}`, source: resolve("generated/native-fixtures", `${slug}.slop`) }));
for (const name of (await readdir("tests/fixtures")).sort())
  packages.push({ name: `fixture-${name}`, source: resolve("tests/fixtures", name, "document") });

type Stage = "initialRead" | "png" | "pdf" | "finalRead";
type RenderResult = { name: string; sha256: string; passed: boolean; seconds: Partial<Record<Stage | "total", number>> };
const results: RenderResult[] = [];
let parent: string;
const started = performance.now();

beforeAll(async () => {
  if (!all) await prepareNativeFixtures();
  parent = await mkdtemp(join(tmpdir(), "hitslop-native-smoke-"));
  // This run's evidence only: exports never replace a file.
  await rm(evidence, { recursive: true, force: true });
  await mkdir(evidence, { recursive: true });
}, 300_000);
afterAll(async () => {
  const seconds = { initialRead: 0, png: 0, pdf: 0, finalRead: 0, total: (performance.now() - started) / 1000 };
  for (const result of results)
    for (const stage of ["initialRead", "png", "pdf", "finalRead"] as const) seconds[stage] += result.seconds[stage] ?? 0;
  await writeFile(join(evidence, "results.json"), JSON.stringify({ passed: results.every((r) => r.passed), seconds, results }, null, 2) + "\n");
  await rm(parent, { recursive: true, force: true });
});

/** A built template file, or a fixture's build stage. */
const checksum = async (source: string) => ((await stat(source)).isDirectory() ? digest(source) : fileDigest(source));
async function timed<T>(result: RenderResult, stage: Stage, work: Promise<T>): Promise<T> {
  const begun = performance.now();
  try {
    return await work;
  } finally {
    result.seconds[stage] = (performance.now() - begun) / 1000;
  }
}

for (const { name, source } of packages)
  test(`${name} opens natively, renders PNG and PDF, and reads the same afterwards`, async () => {
    const begun = performance.now();
    const root = join(parent, `${name}.slop`);
    const before = await checksum(source);
    const result: RenderResult = { name, sha256: before, passed: false, seconds: {} };
    results.push(result);
    try {
      if ((await stat(source)).isDirectory()) await documentFromStage(source, root);
      else await createDocument(source, root);
      const state = (await timed(result, "initialRead", helperRequest({ method: "get", documentPath: root }))).state;
      for (const format of ["png", "pdf"] as const) {
        const output = join(evidence, `${name}.${format}`);
        await timed(result, format, helperRequest({ method: "export", documentPath: root, format, output }));
        await assertExport(output, format);
      }
      expect((await timed(result, "finalRead", helperRequest({ method: "get", documentPath: root }))).state).toEqual(state);
      expect(await checksum(source), `Master changed: ${name}`).toBe(before);
      result.passed = true;
    } finally {
      result.seconds.total = (performance.now() - begun) / 1000;
    }
  }, 120_000);
