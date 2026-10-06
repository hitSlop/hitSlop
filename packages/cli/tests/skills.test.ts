import { test, expect } from "bun:test";
import {
  mkdtemp,
  rm,
  readFile,
  mkdir,
  symlink,
  readlink,
  writeFile,
  access,
  realpath,
  cp,
  lstat,
  rename,
} from "node:fs/promises";
import { join, resolve } from "node:path";
import { tmpdir } from "node:os";
import { buildSkills } from "../src/skills-build";

test("skills are deterministic, self-contained and describe the active commands", async () => {
  const root = await mkdtemp(join(tmpdir(), "hsl-skills-"));
  try {
    const first = join(root, "first/skills"),
      second = join(root, "second/skills");
    const files = await buildSkills(first);
    expect(await buildSkills(second)).toEqual(files);
    for (const file of files) {
      const content = await readFile(join(first, file), "utf8");
      expect(content).toBe(await readFile(join(second, file), "utf8"));
      expect(content).not.toContain(process.cwd());
      for (const match of content.matchAll(/\]\((references\/[^)#]+)(?:#[^)]*)?\)/g))
        await access(join(first, file, "..", match[1]!));
    }
    for (const command of [
      "init",
      "dev",
      "build",
      "register",
      "schema",
      "get",
      "apply",
      "batch",
      "compact",
      "export",
      "skills",
    ])
      expect(files).toContain(`hitslop-cli/commands/${command}.md`);
    for (const name of [
      "hitslop",
      "hitslop-authoring",
      "hitslop-design",
      "hitslop-document",
      "hitslop-cli",
    ])
      expect(await readFile(join(first, name, "SKILL.md"), "utf8")).toContain(`name: ${name}`);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

const skillNames = ["hitslop", "hitslop-authoring", "hitslop-design", "hitslop-document", "hitslop-cli"];

/** A temporary HOME, Bun home and project, and a runner for any CLI copy inside them. */
async function sandbox(root: string) {
  const home = join(root, "home"),
    project = join(root, "project"),
    bun = join(root, "bun");
  await mkdir(home);
  await mkdir(project);
  const run = async (cli: string, ...args: string[]) => {
    const child = Bun.spawn([process.execPath, join(cli, "src/cli.ts"), ...args], {
      cwd: project,
      env: {
        ...process.env,
        HOME: home,
        BUN_INSTALL: bun,
        BUN_INSTALL_GLOBAL_DIR: "",
        HITSLOP_NATIVE_CLI: "/nonexistent",
        PATH: "/usr/bin:/bin",
      },
      stdout: "pipe",
      stderr: "pipe",
    });
    const [stdout, stderr, code] = await Promise.all([
      new Response(child.stdout).text(),
      new Response(child.stderr).text(),
      child.exited,
    ]);
    return { stdout, stderr, code };
  };
  return { home, project, bun, run };
}

/** The CLI laid out as `bun install -g` leaves it: a plain package directory. */
async function globalCopy(bun: string) {
  const cli = join(bun, "install/global/node_modules/@hitslop/cli");
  await mkdir(cli, { recursive: true });
  await cp("packages/cli/src", join(cli, "src"), { recursive: true });
  await cp("packages/cli/package.json", join(cli, "package.json"));
  await cp("packages/cli/skills", join(cli, "skills"), { recursive: true });
  await symlink(resolve("packages/cli/node_modules"), join(cli, "node_modules"));
  await buildSkills(join(cli, ".crust/root/skills"));
  return cli;
}

const repositoryCli = resolve("packages/cli");

test("agent skills link to the global CLI and follow its upgrades", async () => {
  const root = await mkdtemp(join(tmpdir(), "hsl-skill-global-"));
  try {
    const { home, project, bun, run } = await sandbox(root);
    const cli = await globalCopy(bun);
    const packaged = join(await realpath(cli), ".crust/root/skills");
    const link = join(home, ".agents/skills/hitslop-cli");
    expect((await run(cli, "skills", "--all", "--scope", "global")).code).toBe(0);
    expect(await readlink(link)).toBe(join(packaged, "hitslop-cli"));
    expect((await run(cli, "skill", "install", "--all", "--scope", "project")).code).toBe(0);
    for (const name of skillNames)
      expect(await readlink(join(project, ".agents/skills", name))).not.toStartWith("/");

    // `bun install -g` swaps a new package directory in at the same path.
    const upgraded = join(root, "upgraded");
    await cp(cli, upgraded, { recursive: true, verbatimSymlinks: true });
    const skill = join(upgraded, ".crust/root/skills/hitslop-cli/SKILL.md");
    await writeFile(skill, (await readFile(skill, "utf8")).replace(/version: ".*"/, 'version: "9.9.9"'));
    await rm(cli, { recursive: true });
    await rename(upgraded, cli);
    expect(await readFile(join(link, "SKILL.md"), "utf8")).toContain('version: "9.9.9"');

    // Any copy may remove links; shared content is untouched.
    expect((await run(repositoryCli, "skills", "uninstall", "--all", "--scope", "global")).code).toBe(0);
    expect(await lstat(link).catch(() => undefined)).toBeUndefined();
    expect(await Bun.file(join(packaged, "hitslop-cli/SKILL.md")).exists()).toBe(true);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("copies outside the global install never install or repair agent skill links", async () => {
  const root = await mkdtemp(join(tmpdir(), "hsl-skill-local-"));
  try {
    const { home, project, run } = await sandbox(root);
    for (const args of [
      ["skills"],
      ["skills", "--all", "--scope", "global"],
      ["skill", "install", "--all", "--scope", "project"],
      ["skills", "repair", "--scope", "global"],
    ]) {
      const result = await run(repositoryCli, ...args);
      expect(result.code).not.toBe(0);
      expect(result.stderr).toContain("bun install -g @hitslop/cli");
    }
    expect(await lstat(join(home, ".agents")).catch(() => undefined)).toBeUndefined();
    expect(await lstat(join(project, ".agents")).catch(() => undefined)).toBeUndefined();
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});
