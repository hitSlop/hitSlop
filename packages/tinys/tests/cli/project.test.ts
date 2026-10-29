import { expect, test } from "bun:test";
import { mkdtemp, mkdir, writeFile, rm } from "node:fs/promises";
import { join, resolve } from "node:path";
import { tmpdir } from "node:os";
import { exec } from "../../../../scripts/lib/test-process";
import metadata from "../../package.json";
import starter from "../../templates/checklist/package.json";
import { checker } from "../../src/cli/check";

test("cwd cannot redirect document commands; only an explicit project prefix delegates", async () => {
  const root = await mkdtemp(join(tmpdir(), "tinys-project-"));
  try {
    const installed = join(root, "node_modules/@hitslop/tinys");
    await mkdir(join(installed, "src/cli"), { recursive: true });
    await writeFile(join(installed, "package.json"), JSON.stringify({ name: "@hitslop/tinys", version: "2.0.0", exports: { "./package.json": "./package.json" } }));
    await writeFile(join(installed, "src/cli/cli.ts"), 'console.log("project CLI: " + process.argv.slice(2).join(" "));');
    const cli = resolve("packages/tinys/src/cli/cli.ts");
    const ordinary = await exec([process.execPath, cli, "--version"], { cwd: root });
    expect(ordinary.stdout.trim()).toBe(`tiny v${metadata.version}`);
    const explicit = await exec([process.execPath, cli, `--project=${root}`, "get", "Working.tiny"], { cwd: root });
    expect(explicit.stdout.trim()).toBe("project CLI: get Working.tiny");
    const document = await exec([process.execPath, cli, "get", "Missing.tiny"], { cwd: root });
    expect(document.stdout).not.toContain("project CLI");
    expect(document.code).not.toBe(0);
  } finally { await rm(root, { recursive: true, force: true }); }
});

// The CLI carries no checker: `tiny check` runs the project's own, or says what to install.
test("tiny check runs the project's checker and names what to install without one", async () => {
  const root = await mkdtemp(join(tmpdir(), "tinys-check-"));
  try {
    const cli = resolve("packages/tinys/src/cli/cli.ts");
    const project = join(root, "project");
    await mkdir(project);
    await writeFile(join(project, "package.json"), JSON.stringify({ private: true }));
    const tools = Object.entries(starter.devDependencies).filter(([name]) => name !== "@hitslop/tinys");
    const missing = await exec([process.execPath, cli, "check", project]);
    expect(missing.code).not.toBe(0);
    expect(missing.stderr).toContain(`bun add -d ${tools.map(([name, version]) => `${name}@${version}`).join(" ")}`);
    await writeFile(join(project, "package.json"), JSON.stringify({ private: true, devDependencies: { [checker]: "*" } }));
    expect((await exec([process.execPath, cli, "check", project])).stderr).toContain(`Run bun install in ${project}`);
    // Found as `bun run` finds a binary, here hoisted one directory up as in a workspace.
    const bin = join(root, "node_modules/.bin");
    await mkdir(bin, { recursive: true });
    await writeFile(join(bin, checker), 'console.log("project checker: " + process.argv.slice(2).join(" "));', { mode: 0o755 });
    expect((await exec([process.execPath, cli, "check", project])).stdout).toContain(`project checker: --workspace ${project}`);
  } finally { await rm(root, { recursive: true, force: true }); }
});
