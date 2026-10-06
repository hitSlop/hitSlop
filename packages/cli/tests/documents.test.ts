import { test, expect } from "bun:test";
import { existsSync } from "node:fs";
import { mkdtemp, writeFile, readFile, realpath, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { findEngine } from "../src/engine";
import { writeTemplate } from "./template-fixture";

const engine = await findEngine();
/** A small template file in `folder`. */
const template = (folder: string) => writeTemplate(join(folder, "Template.slop"));

const run = async (args: string[], env: Record<string, string> = {}) => {
  const p = Bun.spawn([process.execPath, "packages/cli/src/cli.ts", ...args], {
    stdout: "pipe",
    stderr: "pipe",
    env: { ...process.env, HITSLOP_ENGINE: engine, ...env },
  });
  const [out, error, code] = await Promise.all([
    new Response(p.stdout).text(),
    new Response(p.stderr).text(),
    p.exited,
  ]);
  return { out, error, code };
};
const cli = (command: string, file: string, ...args: string[]) => run([command, file, ...args]);

// A template is never edited: a command refuses it before it can create document state.
test("document CLI refuses a template before mutation", async () => {
  const parent = await mkdtemp(join(tmpdir(), "hsl-master-"));
  try {
    const master = await template(parent);
    const before = await readFile(master);
    const refused = await cli("apply", master, "--op", JSON.stringify({ type: "set", path: ["title"], value: "Changed" }));
    expect(refused.code).not.toBe(0);
    expect(refused.error).toContain("creating a document from it");
    expect(await readFile(master)).toEqual(before);
  } finally {
    await rm(parent, { recursive: true, force: true });
  }
}, 60000);
// An export never replaces a file: not the document it reads, nor another document.
test("document CLI exports never replace a file", async () => {
  const parent = await mkdtemp(join(tmpdir(), "hsl-export-"));
  try {
    const source = await template(parent);
    const [root, other] = [join(parent, "Document.slop"), join(parent, "Other.slop")];
    // A document's first open saves its initial values; after that, reads write nothing.
    for (const output of [root, other]) {
      expect(await Bun.spawn([engine, "create", "--from", source, "--output", output], { stdout: "ignore", stderr: "ignore" }).exited).toBe(0);
      expect((await cli("get", output)).code).toBe(0);
    }
    for (const output of [root, other]) {
      const before = await readFile(output);
      const refused = await cli("theme", "export", root, "--output", output);
      expect(refused.code).not.toBe(0);
      expect(refused.error).toContain("exports never replace a file");
      expect(await readFile(output)).toEqual(before);
      expect((await cli("get", output)).code).toBe(0);
    }
    const theme = join(parent, "theme.json");
    expect((await cli("theme", "export", root, "--output", theme)).code).toBe(0);
    expect((await cli("theme", "export", root, "--output", theme)).code).not.toBe(0);
  } finally {
    await rm(parent, { recursive: true, force: true });
  }
}, 60000);
test("document CLI rejects invalid files and malformed commands before mutation", async () => {
  const parent = await mkdtemp(join(tmpdir(), "hsl-cli-"));
  const root = join(parent, "Document.slop");
  try {
    await writeFile(root, JSON.stringify({ slug: "not-a-slop" }));
    const refused = await cli("get", root);
    expect(refused.code).not.toBe(0);
    expect(refused.error).toContain("not a hitSlop document");
    expect(await readFile(root, "utf8")).toBe(JSON.stringify({ slug: "not-a-slop" }));
    await rm(root);
    const created = Bun.spawn([engine, "create", "--from", await template(parent), "--output", root], { stdout: "ignore", stderr: "pipe" });
    expect(await created.exited).toBe(0);
    expect((await cli("get", root)).code).toBe(0);
    const before = await readFile(root);
    expect((await cli("apply", root, "--op", "null")).code).not.toBe(0);
    expect((await cli("batch", root, "--ops", "null")).code).not.toBe(0);
    expect(await readFile(root)).toEqual(before);
  } finally {
    await rm(parent, { recursive: true, force: true });
  }
}, 60000);


test("create makes parent folders and reports the new document path", async () => {
  const folder = await mkdtemp(join(tmpdir(), "hsl-create-"));
  try {
    const source = await template(folder);
    const output = join(folder, "nested", "New document.slop");
    const created = await cli("create", "--from", source, "--output", output);
    expect(created.code).toBe(0);
    expect(created.out.trim()).toBe(await realpath(output));
    expect((await cli("get", output)).code).toBe(0);
  } finally { await rm(folder, { recursive: true, force: true }); }
}, 60000);

// Failure: the engine created a document named `notes`, which the app refuses to open, and
// documents inside the template catalog. Oracle: the name the app opens, and for the
// catalog the refusal with no file or folder made.
test("create names documents as the app does and refuses the template catalog", async () => {
  const folder = await mkdtemp(join(tmpdir(), "hsl-create-place-"));
  try {
    const source = await template(folder);
    const created = await cli("create", "--from", source, "--output", join(folder, "notes"));
    expect(created.code).toBe(0);
    expect(created.out.trim()).toBe(join(await realpath(folder), "notes.slop"));
    expect(await Bun.file(join(folder, "notes")).exists()).toBe(false);
    const templates = join(folder, "templates");
    const refused = await run(["create", "--from", source, "--output", join(templates, "Copy.slop")], { HITSLOP_TEMPLATES_ROOT: templates });
    expect(refused.code).not.toBe(0);
    expect(refused.error).toContain("among installed templates");
    expect(existsSync(templates)).toBe(false);
  } finally { await rm(folder, { recursive: true, force: true }); }
}, 60000);
