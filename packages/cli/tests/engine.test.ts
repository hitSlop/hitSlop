import { afterEach, expect, test } from "bun:test";
import { chmod, mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { findEngine } from "../src/engine";

const roots: string[] = [];
const saved = process.env.HITSLOP_ENGINE;
afterEach(async () => {
  if (saved === undefined) delete process.env.HITSLOP_ENGINE;
  else process.env.HITSLOP_ENGINE = saved;
  for (const root of roots.splice(0)) await rm(root, { recursive: true, force: true });
});

/** A checkout-shaped folder: the CLI at `packages/cli`, with the engines named present. */
async function checkout(engines: { packaged?: boolean; built?: boolean }) {
  const root = await mkdtemp(join(tmpdir(), "hitslop-engine-"));
  roots.push(root);
  const cli = join(root, "packages/cli");
  const paths = {
    packaged: join(cli, "engine", `${process.platform}-${process.arch}`, "slop-engine"),
    built: join(root, "target/release/slop-engine"),
  };
  await mkdir(cli, { recursive: true });
  for (const kind of ["packaged", "built"] as const) {
    if (!engines[kind]) continue;
    await mkdir(dirname(paths[kind]), { recursive: true });
    await writeFile(paths[kind], "#!/bin/sh\n");
    await chmod(paths[kind], 0o755);
  }
  return { cli, ...paths };
}

test("the engine installed with the CLI comes before a checkout's build", async () => {
  delete process.env.HITSLOP_ENGINE;
  const both = await checkout({ packaged: true, built: true });
  expect(await findEngine(both.cli)).toBe(both.packaged);
  const built = await checkout({ built: true });
  expect(await findEngine(built.cli)).toBe(built.built);
  const none = await checkout({});
  await expect(findEngine(none.cli)).rejects.toThrow("has no file engine");
});

test("HITSLOP_ENGINE names the engine, and must be executable", async () => {
  const both = await checkout({ packaged: true, built: true });
  const chosen = join(dirname(both.cli), "chosen-engine");
  await writeFile(chosen, "#!/bin/sh\n");
  process.env.HITSLOP_ENGINE = chosen;
  await expect(findEngine(both.cli)).rejects.toThrow("HITSLOP_ENGINE is not executable");
  await chmod(chosen, 0o755);
  expect(await findEngine(both.cli)).toBe(chosen);
  process.env.HITSLOP_ENGINE = "";
  await expect(findEngine(both.cli)).rejects.toThrow("HITSLOP_ENGINE is not executable");
});
