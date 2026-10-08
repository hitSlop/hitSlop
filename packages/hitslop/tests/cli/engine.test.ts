import { afterEach, expect, test } from "bun:test";
import { chmod, mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { findEngine } from "../../src/cli/engine";

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
  const { findEngine } = await import("../../src/cli/engine");
  const root = await mkdtemp(join(tmpdir(), "hitslop-owner-engine-")); roots.push(root);
  const engine = join(root, "slop-engine"); await writeFile(engine, "#!/bin/sh\n", { mode: 0o755 });
  const old = process.env.HITSLOP_NATIVE_CLI;
  try {
    process.env.HITSLOP_NATIVE_CLI = join(root, "missing-renderer");
    process.env.HITSLOP_ENGINE = engine;
    expect(await findEngine()).toBe(engine);
    process.env.HITSLOP_ENGINE = join(root, "missing-engine");
    await expect(findEngine()).rejects.toThrow("HITSLOP_ENGINE is not executable");
  } finally {
    if (old === undefined) delete process.env.HITSLOP_NATIVE_CLI; else process.env.HITSLOP_NATIVE_CLI = old;
  }
});

test("a renderer never selects the document engine", async () => {
  const { findEngine } = await import("../../src/cli/engine");
  const root = await mkdtemp(join(tmpdir(), "hitslop-owner-sibling-")); roots.push(root);
  const helper = join(root, "hitslop-native"); await writeFile(helper, "#!/bin/sh\n", { mode: 0o755 });
  const old = process.env.HITSLOP_NATIVE_CLI;
  try {
    delete process.env.HITSLOP_ENGINE;
    process.env.HITSLOP_NATIVE_CLI = helper;
    const own = await findEngine();
    const engine = join(root, "slop-engine"); await writeFile(engine, "#!/bin/sh\n", { mode: 0o755 });
    expect(await findEngine()).toBe(own);
    // Authoring must keep the CLI's evaluated metadata and its validating core together.
    expect(await findEngine()).not.toBe(engine);
  } finally {
    if (old === undefined) delete process.env.HITSLOP_NATIVE_CLI; else process.env.HITSLOP_NATIVE_CLI = old;
  }
});

test("one JSON transport validates results and never retries ambiguous invocations", async () => {
  const { request } = await import("../../src/cli/engine");
  const root = await mkdtemp(join(tmpdir(), "hitslop-wire-")); roots.push(root);
  const binary = join(root, "engine"), sent = join(root, "sent");
  for (const reply of [
    { ok: true, method: "call" },
    { ok: true, method: "templates", catalog: { folders: [], templates: [], issues: [] } },
    { ok: true, method: "call", result: null, ids: [], unexpected: true },
    { ok: true, method: "call", result: null, ids: [1] },
    { ok: true, method: "call", ids: [] },
    { ok: false, code: "future_outcome", error: "Unclassified" },
  ]) {
    await writeFile(binary, `#!${process.execPath}
const body = await Bun.stdin.json();
await Bun.write(${JSON.stringify(sent)}, JSON.stringify({args:process.argv.slice(2),body}));
console.log(${JSON.stringify(JSON.stringify(reply))});
`, { mode: 0o755 });
    const body = { method: "call" as const, documentPath: "-My doc.slop", command: "addTask", args: {} };
    await expect(request(body, { binary })).rejects.toThrow("Outcome unknown");
    const observed = await Bun.file(sent).json();
    expect(observed.body).toEqual(body);
    expect(observed.args).toEqual(["--client-protocol", String((await import("../../src/schema/constants")).HelperProtocol.version)]);
  }
  await writeFile(binary, `#!${process.execPath}
await Bun.stdin.json(); console.log(JSON.stringify({ok:true,method:"screenshot",output:null}));
`, { mode: 0o755 });
  expect(await request({ method: "screenshot", documentPath: "a.slop", output: "a.png", target: "icon", ifPresent: true }, { binary })).toEqual({ ok: true, method: "screenshot", output: null });
});

test("only protocol refusals suggest updating the CLI", async () => {
  const { execute, ExitStatus, request } = await import("../../src/cli/engine");
  const root = await mkdtemp(join(tmpdir(), "hitslop-refusal-")); roots.push(root);
  const binary = join(root, "engine");
  for (const reason of ["requires_update", "invalid_request"] as const) {
    await writeFile(binary, `#!${process.execPath}
await Bun.stdin.json(); console.log(JSON.stringify({ok:false,code:"rejected",reason:${JSON.stringify(reason)},error:"update the hitSlop CLI"}));
`, { mode: 0o755 });
    try { await execute({ method: "templates" }, { binary }); throw new Error("unexpected success"); }
    catch (error) {
      expect((error as Error).message.includes("bunx")).toBe(reason === "requires_update");
    }
  }
  await writeFile(binary, '#!/bin/sh\necho "update the hitSlop CLI" >&2\nexit 2\n', { mode: 0o755 });
  await expect(request({ method: "templates" }, { binary })).rejects.toBeInstanceOf(ExitStatus);
  await writeFile(binary, '#!/bin/sh\nexit 19\n', { mode: 0o755 });
  await expect(request({ method: "templates" }, { binary })).rejects.toThrow("Outcome unknown");
});
