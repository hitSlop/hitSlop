import { test, expect } from "bun:test";
import { mkdtemp, writeFile, rm } from "node:fs/promises";
import { join, resolve } from "node:path";
import { tmpdir } from "node:os";
import { HelperProtocol } from "@hitslop/schema/constants";

test("native protocol selection defaults to 1 and refuses unknown versions before document access", async () => {
  const helper = process.env.HITSLOP_NATIVE_CLI!;
  const run = async (args: string[]) => {
    const child = Bun.spawn([helper, ...args], { stdout: "pipe", stderr: "pipe" });
    const [stdout, stderr, code] = await Promise.all([new Response(child.stdout).text(), new Response(child.stderr).text(), child.exited]);
    return { stdout, stderr, code };
  };
  const direct = await run(["--protocol"]);
  expect(direct.code).toBe(0);
  expect(await run(["--client-protocol", "1", "--protocol"])).toEqual(direct);
  const rejected = await run(["--client-protocol", "2", "request"]);
  expect(rejected.code).not.toBe(0);
  expect(rejected.stderr).toContain("Unsupported command protocol 2");
  expect(rejected.stderr).not.toContain("Missing");
});

// The CLI and the app update separately. Any helper whose protocol range includes this
// CLI's version runs the command, whatever core it embeds; any other is refused before
// the document command runs, with the side that must update.
const protocols: [string, string | undefined][] = [
  [JSON.stringify({ version: HelperProtocol.version, minimum: HelperProtocol.version }), undefined],
  [JSON.stringify({ version: HelperProtocol.version + 2, minimum: HelperProtocol.version }), undefined],
  [JSON.stringify({ version: HelperProtocol.version + 2, minimum: HelperProtocol.version + 1 }), "update @hitslop/cli"],
  [JSON.stringify({ version: 0, minimum: 0 }), "update hitSlop"],
  ["another-core", "did not report its command protocol"],
  ["", "did not report its command protocol"],
];
test.each(protocols)("helper protocol %s", async (reported, refusal) => {
  if (process.platform !== "darwin") return;
  const root = await mkdtemp(join(tmpdir(), "hsl-protocol-"));
  try {
    const helper = join(root, "helper");
    const touched = join(root, "command-ran");
    await writeFile(helper, `#!${process.execPath}
if (process.argv[2] === "--protocol") console.log(${JSON.stringify(reported)});
else {
  await Bun.write(${JSON.stringify(touched)}, JSON.stringify({ args: process.argv.slice(2), request: await Bun.stdin.json() }));
  console.log(JSON.stringify({ ok: true }));
}
`, { mode: 0o755 });
    const child = Bun.spawn([process.execPath, "packages/cli/src/cli.ts", "compact", "example.slop"], {
      env: { ...process.env, HITSLOP_NATIVE_CLI: helper }, stdout: "pipe", stderr: "pipe",
    });
    const code = await child.exited;
    if (refusal) {
      expect(code).not.toBe(0);
      expect(await new Response(child.stderr).text()).toContain(refusal);
    } else expect(code).toBe(0);
    expect(await Bun.file(touched).exists()).toBe(!refusal);
    if (!refusal)
      expect(await Bun.file(touched).json()).toEqual({
        args: ["--client-protocol", String(HelperProtocol.version), "request"],
        request: { method: "compact", documentPath: resolve("example.slop") },
      });
  } finally { await rm(root, { recursive: true, force: true }); }
});

test("explicit native helper overrides fail without falling back or retrying", async () => {
  const root = await mkdtemp(join(tmpdir(), "hsl-native-discovery-"));
  const run = async (helper: string) => {
    const child = Bun.spawn([process.execPath, "packages/cli/src/cli.ts", "get", "example.slop"], {
      env: { ...process.env, HITSLOP_NATIVE_CLI: helper },
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
    expect(selected.code).toBe(23);
    expect(selected.error).toContain("selected helper failed");
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});
