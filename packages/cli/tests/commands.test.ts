import { test, expect } from "bun:test";
import { cp, mkdtemp, rm, symlink, readFile, lstat } from "node:fs/promises";
import { loadProject, normalizeApp } from "../src/build";
import { join, resolve } from "node:path";
import { tmpdir } from "node:os";
import metadata from "../package.json";

async function run(
  args: string[],
  env: Record<string, string> = {},
  cli = "packages/cli/src/cli.ts",
) {
  const child = Bun.spawn([process.execPath, cli, ...args], {
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

/** The manifest a build would store for `project`, from its slop.ts. */
async function manifestOf(project: string) {
  return (await normalizeApp(project, await loadProject(project))).manifest;
}

// A project's folder name is its slug, so init refuses a folder that is not one before
// creating anything.
test("init names a project by its folder and refuses a folder that is not a slug", async () => {
  // Inside the checkout, so the created slop.ts resolves @hitslop/document.
  const root = await mkdtemp(join(process.cwd(), ".build-test-"));
  try {
    for (const name of ["a", "foo--bar", "Foo", "my app", "a".repeat(65)]) {
      const target = join(root, name);
      const result = await run(["init", target, "--yes"]);
      expect(result.code).not.toBe(0);
      expect(result.stderr).toContain("folder's name is its slug");
      expect(await lstat(target).catch(() => undefined)).toBeUndefined();
    }
    const target = join(root, "a".repeat(64));
    expect((await run(["init", target, "--yes"])).code).toBe(0);
    expect((await manifestOf(target)).slug).toBe("a".repeat(64));
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("init flags and defaults produce validated metadata without prompts or partial projects", async () => {
  const root = await mkdtemp(join(process.cwd(), ".build-test-"));
  try {
    const target = join(root, "budget-book");
    const created = await run([
      "init",
      target,
      "--yes",
      "--brief",
      "Track monthly spending.",
      "--title",
      'My "budget"',
      "--category",
      "finance",
      "--category",
      "personal",
      "--author",
      "Jordan",
      "--description",
      "Track spending.",
    ]);
    expect(created.code).toBe(0);
    expect(created.stderr).toBe("");
    expect(await readFile(join(target, "BRIEF.md"), "utf8")).toContain("Track monthly spending.");
    const manifest = await manifestOf(target);
    expect({
      title: manifest.title,
      slug: manifest.slug,
      categories: manifest.categories,
      author: manifest.author,
      description: manifest.description,
    }).toEqual({
      title: 'My "budget"',
      slug: "budget-book",
      categories: ["finance", "personal"],
      author: { name: "Jordan" },
      description: "Track spending.",
    });
    const defaults = join(root, "defaults");
    expect((await run(["init", defaults], { CI: "1" })).code).toBe(0);
    const fallback = await manifestOf(defaults);
    expect([
      fallback.title,
      fallback.author.name,
      fallback.description,
      fallback.categories,
    ]).toEqual(["defaults", "Anonymous", "A hitSlop mini app.", ["productivity"]]);
    const invalid = [
      ["--title", ""],
      ["--title", "x".repeat(81)],
      ["--author", " "],
      ["--description", ""],
      ["--category", "unknown"],
      ["--category", "finance", "--category", "finance"],
      ["--category", "finance", "--category", "personal", "--category", "other"],
    ];
    for (const [index, flags] of invalid.entries()) {
      const destination = join(root, `invalid-${index}`);
      expect((await run(["init", destination, ...flags])).code).not.toBe(0);
      expect(await lstat(destination).catch(() => undefined)).toBeUndefined();
    }
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("help and version do not invoke native or authoring handlers", async () => {
  // A fresh checkout has sources and dependencies, but no generated skill bundle.
  const root = await mkdtemp(join(tmpdir(), "hsl-cli-help-"));
  try {
    await cp("packages/cli/src", join(root, "src"), { recursive: true });
    await cp("packages/cli/package.json", join(root, "package.json"));
    await symlink(resolve("packages/cli/node_modules"), join(root, "node_modules"));
    for (const args of [
      [],
      ["--help"],
      ["--version"],
      ["-v"],
      ["build", "--help"],
      ["apply", "--help"],
      ["get", "--help"],
      ["skills", "--help"],
      ["skill", "-h"],
      ["skills", "install", "--help"],
      ["skills", "repair", "--help"],
      ["skills", "uninstall", "--help"],
      ["init", "--help"],
    ]) {
      const result = await run(
        args,
        { HITSLOP_NATIVE_CLI: "/nonexistent" },
        join(root, "src/cli.ts"),
      );
      expect(result.stderr).toBe("");
      expect(result.code).toBe(0);
      expect(result.stdout).toContain("slop");
      if (args[0] === "--version" || args[0] === "-v")
        expect(result.stdout.trim()).toBe(`slop v${metadata.version}`);
    }
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("invalid input fails before opening documents or building source", async () => {
  for (const args of [
    ["wat"],
    ["get"],
    ["get", "missing.slop", "--wat"],
    ["apply", "missing.slop"],
    ["batch", "missing.slop"],
    ["import", "missing.slop"],
    ["theme", "set", "missing.slop"],
    ["attachments", "export", "missing.slop", "id"],
    ["export", "missing.slop", "--format", "jpeg", "--output", "x"],
    ...["0", "65536", "1.5", "NaN"].map((port) => ["dev", "missing-source", "--port", port]),
  ]) {
    const result = await run(args, { HITSLOP_NATIVE_CLI: "/nonexistent" });
    expect(result.code).not.toBe(0);
    expect(result.stderr).not.toContain("HITSLOP_NATIVE_CLI");
  }
});

test("mistyped commands suggest the intended command", async () => {
  for (const [args, suggestion] of [
    [["gte", "missing.slop"], "get"],
    [["theme", "gett", "missing.slop"], "get"],
    [["skils"], "skills"],
  ] as const) {
    const result = await run([...args], { HITSLOP_NATIVE_CLI: "/nonexistent" });
    expect(result.code).not.toBe(0);
    expect(result.stderr).toContain(`Did you mean "${suggestion}"?`);
  }
});
