import { test, expect } from "bun:test";
import { mkdtemp, rm, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { coreBuildId } from "../src/core";

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
      `#!${process.execPath}\nif (process.argv[2] === "--core-build") console.log(${JSON.stringify(await coreBuildId())}); else { console.log(JSON.stringify(process.argv.slice(2))); process.exit(23); }\n`,
      { mode: 0o755 },
    );
    const json = '{ "type": "text.replace", "value": "hello \\"world\\"" }';
    const args = ["apply", "a file.slop", "--op", json];
    const result = await run(args, { HITSLOP_NATIVE_CLI: helper });
    expect(result.code).toBe(23);
    expect(JSON.parse(result.stdout)).toEqual(args);
    const imported = ["import", "a file.slop", "new data.json", "--path", '["rows"]'];
    const importing = await run(imported, { HITSLOP_NATIVE_CLI: helper });
    expect(importing.code).toBe(23);
    expect(JSON.parse(importing.stdout)).toEqual(imported);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("create and open forward to the identity-checked helper", async () => {
  if (process.platform !== "darwin") return;
  const root = await mkdtemp(join(tmpdir(), "hsl-create-open-"));
  try {
    const helper = async (id: string) => {
      const path = join(root, `helper-${id === "another-core" ? "other" : "matching"}`);
      await writeFile(
        path,
        `#!${process.execPath}\nif (process.argv[2] === "--core-build") console.log(${JSON.stringify(id)}); else console.log(JSON.stringify(process.argv.slice(2)));\n`,
        { mode: 0o755 },
      );
      return path;
    };
    const matching = await helper(await coreBuildId());
    const create = ["create", "--from", "a template.slop", "--output", "my doc.slop"];
    const created = await run(create, { HITSLOP_NATIVE_CLI: matching });
    expect(created.code).toBe(0);
    expect(JSON.parse(created.stdout)).toEqual(create);
    const opened = await run(["open", "my doc.slop"], { HITSLOP_NATIVE_CLI: matching });
    expect(opened.code).toBe(0);
    expect(JSON.parse(opened.stdout)).toEqual(["open", "my doc.slop"]);
    const refused = await run(create, { HITSLOP_NATIVE_CLI: await helper("another-core") });
    expect(refused.code).not.toBe(0);
    expect(refused.stdout).toBe("");
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});
