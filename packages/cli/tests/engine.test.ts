import { afterEach, expect, test } from "bun:test";
import { chmod, mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { findEngine } from "../src/engine";

const roots: string[] = [];
const saved = process.env.HITSLOP_ENGINE;
afterEach(async () => {
  if (saved === undefined) delete process.env.HITSLOP_ENGINE;
  else process.env.HITSLOP_ENGINE = saved;
  for (const root of roots.splice(0)) await rm(root, { recursive: true, force: true });
});

test("HITSLOP_ENGINE names the engine, and must be executable", async () => {
  const root = await mkdtemp(join(tmpdir(), "hitslop-engine-"));
  roots.push(root);
  const chosen = join(root, "chosen-engine");
  await writeFile(chosen, "#!/bin/sh\n");
  process.env.HITSLOP_ENGINE = chosen;
  await expect(findEngine()).rejects.toThrow("HITSLOP_ENGINE is not executable");
  await chmod(chosen, 0o755);
  expect(await findEngine()).toBe(chosen);
  process.env.HITSLOP_ENGINE = "";
  await expect(findEngine()).rejects.toThrow("HITSLOP_ENGINE is not executable");
});
