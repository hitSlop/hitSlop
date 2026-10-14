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

test("one JSON transport validates results and never retries ambiguous invocations", async () => {
  const { request } = await import("../../src/cli/engine");
  const root = await mkdtemp(join(tmpdir(), "hitslop-wire-")); roots.push(root);
  const binary = join(root, "engine"), sent = join(root, "sent");
  for (const reply of [
    { ok: true, method: "call" },
    { ok: true, method: "templates", catalog: { folders: [], templates: [], issues: [] } },
    { ok: true, method: "call", result: null, ids: [1] },
    { ok: true, method: "call", ids: [] },
    { ok: false, code: "future_outcome", error: "Unclassified" },
    { ok: false, code: "rejected" },
    { ok: false, error: "No code" },
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
    expect(observed.args).toEqual(["--client-protocol", String((await import("../../src/schema/constants")).EngineProtocol.version)]);
  }
  await writeFile(binary, `#!${process.execPath}
await Bun.stdin.json(); console.log(JSON.stringify({ok:true,method:"artwork.export",output:null}));
`, { mode: 0o755 });
  expect(await request({ method: "artwork.export", file: "a.slop", target: "icon", output: "a.png" }, { binary })).toEqual({ ok: true, method: "artwork.export", output: null });
});

// A newer engine may add members to its replies; an older CLI reads the ones it knows and
// ignores the rest, so a mutation that applied is never reported as an unknown outcome.
test("replies may carry members this CLI does not know", async () => {
  const { execute, request } = await import("../../src/cli/engine");
  const root = await mkdtemp(join(tmpdir(), "hitslop-open-reply-")); roots.push(root);
  const binary = join(root, "engine");
  const answer = (reply: unknown) =>
    writeFile(binary, `#!${process.execPath}
await Bun.stdin.json(); console.log(${JSON.stringify(JSON.stringify(reply))});
`, { mode: 0o755 });
  const body = { method: "call" as const, documentPath: "a.slop", command: "addTask", args: {} };
  await answer({ ok: true, method: "call", result: { n: 1 }, ids: ["r1"], later: { added: [true] } });
  const applied = await execute(body, { binary });
  expect(applied.ids).toEqual(["r1"]);
  expect(applied.result).toEqual({ n: 1 });
  await answer({ ok: true, method: "artwork.export", output: null, later: 1 });
  expect((await request({ method: "artwork.export", file: "a.slop", target: "icon", output: "a.png" }, { binary })).ok).toBe(true);
  // A refusal with extra members is still the refusal, with its outcome.
  await answer({ ok: false, code: "rejected", error: "No such row", reason: "path_not_found", opIndex: 2, later: [1] });
  await expect(execute(body, { binary })).rejects.toThrow("Refused ops[2] (path_not_found): No such row\nNot applied.");
  await answer({ ok: false, code: "save_failed", error: "Disk full", later: {} });
  await expect(execute(body, { binary })).rejects.toThrow("Disk full\nApplied, but not yet saved.");
  // Extra members never stand in for the ones a reply needs.
  for (const reply of [
    { ok: true, method: "call", result: null, later: 1 },
    { ok: true, method: "call", ids: [], later: 1 },
    { ok: true, later: 1 },
    { ok: false, code: "rejected", later: 1 },
    { ok: false, error: "No code", later: 1 },
  ]) {
    await answer(reply);
    await expect(request(body, { binary })).rejects.toThrow("Outcome unknown");
  }
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
