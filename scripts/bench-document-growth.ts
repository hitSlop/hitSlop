/** Opt-in native durability workload. Uses authored stroke geometry, never a second engine. */
import { mkdtemp, mkdir, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { defaultBrush, strokePath, StrokeSamples } from "../examples/slops/doodle-board/drawing";

const days = Number(process.env.HITSLOP_GROWTH_DAYS ?? 365);
if (!Number.isInteger(days) || days < 1 || days > 365) throw new Error("Days must be 1…365");
const input = await mkdtemp(join(tmpdir(), "hitslop-growth-input-"));
try {
  const git = Bun.spawn(["git", "rev-parse", "HEAD"], { stdout: "pipe" });
  const commit = (await new Response(git.stdout).text()).trim();
  if (await git.exited) throw new Error("Cannot identify benchmark commit");
  let seed = 120029;
  const random = () => ((seed = (Math.imul(seed, 1664525) + 1013904223) >>> 0) / 2 ** 32);
  await mkdir(join(input, "strokes"));
  for (let day = 1; day <= days; day++) {
    const strokes = Array.from({ length: 100 }, () => {
      const samples = new StrokeSamples();
      let x = 80 + random() * 500, y = 100 + random() * 400;
      for (let point = 0; point < 100; point++) {
        x = Math.max(20, Math.min(980, x + 3 + (random() - .5) * 8));
        y = Math.max(20, Math.min(680, y + (random() - .5) * 18));
        samples.add([x, y, .5]);
      }
      return strokePath(samples.points, defaultBrush, false);
    });
    await writeFile(join(input, "strokes", `${day}.json`), JSON.stringify(strokes));
  }
  const child = Bun.spawn([process.execPath, "run", "swift:test", "--filter", "DocumentGrowthTests"], {
    env: {
      ...process.env,
      HITSLOP_GROWTH_INPUT: input,
      HITSLOP_GROWTH_DAYS: String(days),
      HITSLOP_GROWTH_COMMIT: commit,
      HITSLOP_GROWTH_OUTPUT:
        process.env.HITSLOP_GROWTH_OUTPUT ??
        `${process.cwd()}/docs/evidence/document-growth-${new Date().toISOString().slice(0, 10)}.json`,
    },
    stdout: "inherit", stderr: "inherit",
  });
  process.exitCode = await child.exited;
} finally {
  await rm(input, { recursive: true, force: true });
}
