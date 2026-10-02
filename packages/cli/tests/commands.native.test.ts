import { test, expect } from "bun:test";
import { mkdtemp, rm, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { HelperProtocol } from "@hitslop/schema/constants";

const served = JSON.stringify(JSON.stringify(HelperProtocol));
const selection = ["--client-protocol", String(HelperProtocol.version)];

async function run(args: string[], env: Record<string, string> = {}) {
  const child = Bun.spawn([process.execPath, "packages/cli/src/cli.ts", ...args], {
    env: { ...process.env, ...env },
    stdout: "pipe",
    stderr: "pipe",
  });
  const [stdout, stderr, code] = await Promise.all([
    new Response(child.stdout).text(),
    new Response(child.stderr).text(),
    child.exited,
  ]);
  return { stdout, stderr, code };
}

test("native forwarding preserves JSON, paths, flags and exit status", async () => {
  if (process.platform !== "darwin") return;
  const root = await mkdtemp(join(tmpdir(), "hsl-command-"));
  try {
    const helper = join(root, "helper");
    await writeFile(
      helper,
      `#!${process.execPath}\nif (process.argv[2] === "--protocol") console.log(${served}); else { console.log(JSON.stringify(process.argv.slice(2))); process.exit(23); }\n`,
      { mode: 0o755 },
    );
    const json = '{ "type": "text.replace", "value": "hello \\"world\\"" }';
    const args = ["apply", "a file.slop", "--op", json];
    const result = await run(args, { HITSLOP_NATIVE_CLI: helper });
    expect(result.code).toBe(23);
    expect(JSON.parse(result.stdout)).toEqual([...selection, ...args]);
    const imported = ["import", "a file.slop", "new data.json", "--path", '["rows"]'];
    const importing = await run(imported, { HITSLOP_NATIVE_CLI: helper });
    expect(importing.code).toBe(23);
    expect(JSON.parse(importing.stdout)).toEqual([...selection, ...imported]);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("create and open forward to a helper that serves this CLI's protocol", async () => {
  if (process.platform !== "darwin") return;
  const root = await mkdtemp(join(tmpdir(), "hsl-create-open-"));
  try {
    const helper = async (name: string, protocol: string) => {
      const path = join(root, `helper-${name}`);
      await writeFile(
        path,
        `#!${process.execPath}\nif (process.argv[2] === "--protocol") console.log(${JSON.stringify(protocol)}); else console.log(JSON.stringify(process.argv.slice(2)));\n`,
        { mode: 0o755 },
      );
      return path;
    };
    const matching = await helper("matching", JSON.parse(served));
    const create = ["create", "--from", "a template.slop", "--output", "my doc.slop"];
    const created = await run(create, { HITSLOP_NATIVE_CLI: matching });
    expect(created.code).toBe(0);
    expect(JSON.parse(created.stdout)).toEqual([...selection, ...create]);
    const opened = await run(["open", "my doc.slop"], { HITSLOP_NATIVE_CLI: matching });
    expect(opened.code).toBe(0);
    expect(JSON.parse(opened.stdout)).toEqual([...selection, "open", "my doc.slop"]);
    const newer = { version: HelperProtocol.version + 1, minimum: HelperProtocol.version + 1 };
    const refused = await run(create, { HITSLOP_NATIVE_CLI: await helper("newer", JSON.stringify(newer)) });
    expect(refused.code).not.toBe(0);
    expect(refused.stdout).toBe("");
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});
