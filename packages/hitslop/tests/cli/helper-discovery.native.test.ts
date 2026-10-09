import { test, expect } from "bun:test";
import { mkdtemp, writeFile, rm } from "node:fs/promises";
import { join, resolve } from "node:path";
import { tmpdir } from "node:os";
import { HelperProtocol } from "../../src/schema/constants";
import { findEngine } from "../../src/cli/engine";

test("the helper reports its protocol and refuses unknown or unnamed protocols before document access", async () => {
  const helper = process.env.HITSLOP_NATIVE_CLI!;
  const run = async (args: string[]) => {
    const child = Bun.spawn([helper, ...args], { stdout: "pipe", stderr: "pipe" });
    const [stdout, stderr, code] = await Promise.all([new Response(child.stdout).text(), new Response(child.stderr).text(), child.exited]);
    return { stdout, stderr, code };
  };
  const direct = await run(["--protocol"]);
  expect(direct.code).toBe(0);
  expect(JSON.parse(direct.stdout)).toEqual({ version: HelperProtocol.version });
  expect(await run(["--client-protocol", String(HelperProtocol.version), "--protocol"])).toEqual(direct);
  // The permanent refusal path: status 2 and one line naming the side to update.
  const newer = await run(["--client-protocol", String(HelperProtocol.version + 1), "export"]);
  expect(newer.code).toBe(2);
  expect(newer.stderr).toBe("This command needs a newer hitSlop app; update hitSlop\n");
  const rejected = await run(["--client-protocol", "0", "export"]);
  expect(rejected.code).toBe(2);
  expect(rejected.stderr).toBe("This hitSlop app needs a newer command line; update the hitSlop CLI\n");
  expect(rejected.stderr).not.toContain("Missing");
  // A document command that names no protocol is a usage error, refused before its request is read.
  const unnamed = await run(["export"]);
  expect(unnamed.code).not.toBe(0);
  expect(unnamed.stderr).toContain("Use --client-protocol N");
});

// The CLI runs the document engine, which passes an export to the selected helper.
const engine = await findEngine();

// The CLI names its protocol; a tool that serves it runs the command, and any other
// refuses before the document command runs, saying which side must update. The CLI passes
// that refusal on, naming its own update command when it is the older side.
const tools: [string, string | undefined, string | undefined][] = [
  ["serves", undefined, undefined],
  ["refuses as newer", "This command needs a newer hitSlop app; update hitSlop", "update hitSlop"],
  ["refuses as older", "This hitSlop app needs a newer command line; update the hitSlop CLI", "update the hitSlop CLI ("],
];
test.each(tools)("a native tool that %s this CLI's protocol", async (behavior, refusal, shown) => {
  if (process.platform !== "darwin") return;
  const root = await mkdtemp(join(tmpdir(), "hsl-protocol-"));
  try {
    const helper = join(root, "helper");
    const touched = join(root, "command-ran");
    await writeFile(helper, `#!${process.execPath}
if (${JSON.stringify(behavior)}.startsWith("refuses")) { console.error(${JSON.stringify(refusal)}); process.exit(2); }
await Bun.write(${JSON.stringify(touched)}, JSON.stringify({ args: process.argv.slice(2), request: await Bun.stdin.json() }));
console.log(JSON.stringify({ ok: true, method: "export", output: "capture.pdf" }));
`, { mode: 0o755 });
    const child = Bun.spawn([process.execPath, "packages/hitslop/src/cli/cli.ts", "export", "example.slop", "--format", "pdf", "--output", "capture.pdf"], {
      env: { ...process.env, HITSLOP_ENGINE: engine, HITSLOP_NATIVE_CLI: helper }, stdout: "pipe", stderr: "pipe",
    });
    const code = await child.exited;
    if (refusal) {
      expect(code).toBe(1);
      expect(await new Response(child.stderr).text()).toContain(shown!);
    } else expect(code).toBe(0);
    expect(await Bun.file(touched).exists()).toBe(!refusal);
    if (!refusal)
      expect(await Bun.file(touched).json()).toEqual({
        args: ["--client-protocol", String(HelperProtocol.version)],
        request: { method: "export", documentPath: resolve("example.slop"), format: "pdf", output: resolve("capture.pdf") },
      });
  } finally { await rm(root, { recursive: true, force: true }); }
});

test("explicit native helper overrides fail without falling back or retrying", async () => {
  const root = await mkdtemp(join(tmpdir(), "hsl-native-discovery-"));
  const run = async (helper: string) => {
    const child = Bun.spawn([process.execPath, "packages/hitslop/src/cli/cli.ts", "export", "example.slop", "--format", "pdf", "--output", "capture.pdf"], {
      env: { ...process.env, HITSLOP_ENGINE: engine, HITSLOP_NATIVE_CLI: helper },
      stdout: "pipe",
      stderr: "pipe",
    });
    return { code: await child.exited, error: await new Response(child.stderr).text() };
  };
  try {
    if (process.platform !== "darwin") return;
    const missing = await run(join(root, "missing"));
    expect(missing.code).not.toBe(0);
    expect(missing.error).toContain("HITSLOP_NATIVE_CLI is not executable");
    const helper = join(root, "helper");
    await writeFile(helper, '#!/bin/sh\necho "selected helper failed" >&2\nexit 23\n', {
      mode: 0o755,
    });
    const selected = await run(helper);
    expect(selected.code).toBe(1);
    expect(selected.error).toContain("Native helper stopped without a valid reply");
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});
