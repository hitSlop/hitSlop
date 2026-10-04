import { test, expect } from "bun:test";
import { Database } from "bun:sqlite";
import { mkdtemp, mkdir, writeFile, readFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { PackageFormat, RuntimeABI } from "@hitslop/schema/constants";
import { defineDocument, s } from "@hitslop/document";
import { pack } from "../../cli/src/engine";
const schema = defineDocument({ title: s.text() });

const helper = new URL("../../../apps/apple/Packages/HitSlopApple/.build/debug/hitslop-native", import.meta.url).pathname;

/** A small template file, packed by the file engine from a stage. */
async function template(folder: string, title: string) {
  const stage = join(folder, "stage");
  await mkdir(join(stage, "assets"), { recursive: true });
  // A build's stage: the app row `slop build` writes.
  const manifest = {
    author: { name: "hitSlop" },
    slug: "quick-checklist",
    title: "Quick Checklist",
    description: "A pocket checklist.",
    categories: ["productivity"],
    presentation: { width: 480, height: 620 },
  };
  const app = { packageFormat: PackageFormat, runtimeABI: RuntimeABI, manifest, descriptor: schema.descriptor, initial: { title }, theme: { accent: "#335577" } };
  await writeFile(join(stage, "app.json"), JSON.stringify(app));
  await writeFile(join(stage, "assets/app.js"), "export default { mount() { return {}; } };");
  const output = join(folder, "Template.slop");
  await pack(stage, output);
  await rm(stage, { recursive: true });
  return output;
}

const cli = async (command: string, file: string, ...args: string[]) => {
  const p = Bun.spawn([process.execPath, "packages/cli/src/cli.ts", command, file, ...args], {
    env: { ...process.env, HITSLOP_NATIVE_CLI: helper },
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
    const master = await template(parent, "Master");
    const before = await readFile(master);
    const refused = await cli("apply", master, "--op", JSON.stringify({ type: "text.replace", path: ["title"], value: "Changed" }));
    expect(refused.code).not.toBe(0);
    expect(refused.error).toContain("create a document from it");
    expect(await readFile(master)).toEqual(before);
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
    const created = Bun.spawn([helper, "create", "--from", await template(parent, "Initial"), "--output", root], { stdout: "ignore", stderr: "pipe" });
    expect(await created.exited).toBe(0);
    expect((await cli("apply", root, "--op", '{"type":"text.replace","path":["title"],"value":"bad"}', "--id", "alone")).error).toContain("Unknown");
    expect((await cli("get", root, "--request-id", "old-name")).error).toContain("Unknown");
    expect((await cli("get", root)).code).toBe(0);
    const before = await readFile(root);
    expect((await cli("apply", root, "--op", "null")).code).not.toBe(0);
    expect((await cli("batch", root, "--ops", "null")).code).not.toBe(0);
    expect(await readFile(root)).toEqual(before);
    // Saved state is bound to the schema it was saved under; another schema is refused.
    const changed = defineDocument({ title: s.text(), extra: s.boolean() });
    const database = new Database(root);
    database.run("UPDATE app SET descriptor = ?, initial = ?", [JSON.stringify(changed.descriptor), JSON.stringify({ title: "Initial", extra: false })]);
    database.close();
    const rebound = await readFile(root);
    expect((await cli("get", root)).error).toContain("schema differs from saved state");
    expect(await readFile(root)).toEqual(rebound);
  } finally {
    await rm(parent, { recursive: true, force: true });
  }
}, 60000);
