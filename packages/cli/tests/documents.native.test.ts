import { test, expect } from "bun:test";
import { mkdtemp, writeFile, readFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { writeTemplate } from "./template-fixture";

const helper = process.env.HITSLOP_NATIVE_CLI!;
/** A small template file in `folder`. */
const template = (folder: string) => writeTemplate(join(folder, "Template.slop"));

const cli = async (command: string, file: string, ...args: string[]) => {
  const p = Bun.spawn([process.execPath, "packages/cli/src/cli.ts", command, file, ...args], {
    stdout: "pipe",
    stderr: "pipe",
  });
  const [out, error, code] = await Promise.all([
    new Response(p.stdout).text(),
    new Response(p.stderr).text(),
    p.exited,
  ]);
  return { out, error, code };
};

// A template is never edited: a command refuses it before it can create document state.
test("native CLI refuses a template before mutation", async () => {
  if (process.platform !== "darwin") return;
  const parent = await mkdtemp(join(tmpdir(), "hsl-master-"));
  try {
    const master = await template(parent);
    const before = await readFile(master);
    const refused = await cli("apply", master, "--op", JSON.stringify({ type: "set", path: ["title"], value: "Changed" }));
    expect(refused.code).not.toBe(0);
    expect(refused.error).toContain("create a document from it");
    expect(await readFile(master)).toEqual(before);
  } finally {
    await rm(parent, { recursive: true, force: true });
  }
}, 60000);
// An export never replaces a file: not the document it reads, nor another document.
test("native CLI exports never replace a file", async () => {
  if (process.platform !== "darwin") return;
  const parent = await mkdtemp(join(tmpdir(), "hsl-export-"));
  try {
    const source = await template(parent);
    const [root, other] = [join(parent, "Document.slop"), join(parent, "Other.slop")];
    // A document's first open saves its initial values; after that, reads write nothing.
    for (const output of [root, other]) {
      expect(await Bun.spawn([helper, "create", "--from", source, "--output", output], { stdout: "ignore", stderr: "ignore" }).exited).toBe(0);
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
test("native CLI rejects invalid files and malformed commands before mutation", async () => {
  if (process.platform !== "darwin") return;
  const parent = await mkdtemp(join(tmpdir(), "hsl-cli-"));
  const root = join(parent, "Document.slop");
  try {
    await writeFile(root, JSON.stringify({ slug: "not-a-slop" }));
    const refused = await cli("get", root);
    expect(refused.code).not.toBe(0);
    expect(refused.error).toContain("Invalid hitSlop file");
    expect(await readFile(root, "utf8")).toBe(JSON.stringify({ slug: "not-a-slop" }));
    await rm(root);
    const created = Bun.spawn([helper, "create", "--from", await template(parent), "--output", root], { stdout: "ignore", stderr: "pipe" });
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
