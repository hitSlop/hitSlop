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

test("document engine override is independent of the native renderer", async () => {
  const { findDocumentEngine } = await import("../src/engine");
  const root = await mkdtemp(join(tmpdir(), "hitslop-owner-engine-")); roots.push(root);
  const engine = join(root, "slop-engine"); await writeFile(engine, "#!/bin/sh\n", { mode: 0o755 });
  const old = process.env.HITSLOP_NATIVE_CLI;
  try {
    process.env.HITSLOP_NATIVE_CLI = join(root, "missing-renderer");
    process.env.HITSLOP_ENGINE = engine;
    expect(await findDocumentEngine()).toBe(engine);
    process.env.HITSLOP_ENGINE = join(root, "missing-engine");
    await expect(findDocumentEngine()).rejects.toThrow("HITSLOP_ENGINE is not executable");
  } finally {
    if (old === undefined) delete process.env.HITSLOP_NATIVE_CLI; else process.env.HITSLOP_NATIVE_CLI = old;
  }
});

test("an explicit macOS helper requires its own sibling engine without fallback", async () => {
  if (process.platform !== "darwin") return;
  const { findDocumentEngine } = await import("../src/engine");
  const root = await mkdtemp(join(tmpdir(), "hitslop-owner-sibling-")); roots.push(root);
  const helper = join(root, "hitslop-native"); await writeFile(helper, "#!/bin/sh\n", { mode: 0o755 });
  const old = process.env.HITSLOP_NATIVE_CLI;
  try {
    delete process.env.HITSLOP_ENGINE;
    process.env.HITSLOP_NATIVE_CLI = helper;
    await expect(findDocumentEngine()).rejects.toThrow("Missing slop-engine alongside");
    const engine = join(root, "slop-engine"); await writeFile(engine, "#!/bin/sh\n", { mode: 0o755 });
    expect(await findDocumentEngine()).toBe(engine);
    // Authoring must keep the CLI's evaluated metadata and its validating core together.
    expect(await findEngine()).not.toBe(engine);
  } finally {
    if (old === undefined) delete process.env.HITSLOP_NATIVE_CLI; else process.env.HITSLOP_NATIVE_CLI = old;
  }
});
