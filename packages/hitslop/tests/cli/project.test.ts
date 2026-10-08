import { expect, test } from "bun:test";
import { mkdtemp, mkdir, writeFile, rm } from "node:fs/promises";
import { join, resolve } from "node:path";
import { tmpdir } from "node:os";
import { exec } from "../../../../scripts/lib/test-process";
import metadata from "../../package.json";

test("cwd cannot redirect document commands; only an explicit project prefix delegates", async () => {
  const root = await mkdtemp(join(tmpdir(), "hitslop-project-"));
  try {
    const installed = join(root, "node_modules/hitslop");
    await mkdir(join(installed, "src/cli"), { recursive: true });
    await writeFile(join(installed, "package.json"), JSON.stringify({ name: "hitslop", version: "2.0.0", exports: { "./package.json": "./package.json" } }));
    await writeFile(join(installed, "src/cli/cli.ts"), 'console.log("project CLI: " + process.argv.slice(2).join(" "));');
    const cli = resolve("packages/hitslop/src/cli/cli.ts");
    const ordinary = await exec([process.execPath, cli, "--version"], { cwd: root });
    expect(ordinary.stdout.trim()).toBe(`slop v${metadata.version}`);
    const explicit = await exec([process.execPath, cli, `--project=${root}`, "get", "Working.slop"], { cwd: root });
    expect(explicit.stdout.trim()).toBe("project CLI: get Working.slop");
    const document = await exec([process.execPath, cli, "get", "Missing.slop"], { cwd: root });
    expect(document.stdout).not.toContain("project CLI");
    expect(document.code).not.toBe(0);
  } finally { await rm(root, { recursive: true, force: true }); }
});
