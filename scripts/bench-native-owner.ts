// Production helper measurement, without an app/release build or authored rendering.
import { cp, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { join, resolve } from "node:path";
import { tmpdir } from "node:os";
import { strict as assert } from "node:assert";
const binary = resolve("apps/apple/Packages/HitSlopApple/.build/debug/hitslop-native");
const folder = await mkdtemp(join(tmpdir(), "hitslop-native-owner-"));
const root = join(folder, "Measure.slop");
try {
  await cp("tests/fixtures/checklist/document", root, { recursive: true });
  const initial = JSON.parse(await readFile(join(root, "initial.json"), "utf8"));
  initial.rows = Array.from({ length: 1000 }, (_, i) => ({
    $id: String(i).padStart(26, "0"),
    text: `Task ${i}`,
    done: false,
  }));
  await writeFile(join(root, "initial.json"), JSON.stringify(initial));
  await writeFile(
    join(root, "assets/app.js"),
    "throw new Error('Authored code must not execute');",
  );
  async function run(args: string[]) {
    const start = performance.now();
    const child = Bun.spawn([binary, ...args], {
      stdout: "pipe",
      stderr: "pipe",
      env: { ...process.env, PATH: "/usr/bin:/bin" },
    });
    const [out, error, code] = await Promise.all([
      new Response(child.stdout).text(),
      new Response(child.stderr).text(),
      child.exited,
    ]);
    assert.equal(code, 0, error);
    return { value: JSON.parse(out), ms: performance.now() - start };
  }
  const edit = (by = 1) =>
    run(["apply", root, "--op", JSON.stringify({ type: "increment", path: ["hits"], by })]);
  await run(["get", root]);
  const samples: number[] = [];
  for (let i = 1; i <= 20; i++) {
    const result = await edit();
    assert.equal(result.value.hits, i);
    samples.push(result.ms);
  }
  // Both closed processes must serialize through flock; neither can bypass it.
  // Expected counter is independent of either process's returned snapshot.
  for (let i = 0; i < 10; i++) await Promise.all([edit(1), edit(2)]);
  assert.equal((await run(["get", root])).value.hits, 50);
  samples.sort((a, b) => a - b);
  const report = {
    rows: 1000,
    samples: 20,
    build: "debug helper",
    p50MS: samples[9],
    p95MS: samples[18],
    budgetMS: 150,
    withinBudget: samples[18]! <= 150,
    concurrentClosedPairs: 10,
    concurrentWritesExactlyOnce: true,
  };
  console.log(JSON.stringify(report, null, 2));
} finally {
  await rm(folder, { recursive: true, force: true });
}
