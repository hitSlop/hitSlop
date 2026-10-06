import { test, expect } from "bun:test";
import { mkdtemp, rm, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { HelperProtocol } from "../../src/schema/constants";

const selection = ["--client-protocol", String(HelperProtocol.version)];

async function run(args: string[], env: Record<string, string> = {}) {
  const child = Bun.spawn([process.execPath, "packages/hitslop/src/cli/cli.ts", ...args], {
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

test("document commands send one request and print its reply", async () => {
  if (process.platform !== "darwin") return;
  const root = await mkdtemp(join(tmpdir(), "hsl-command-"));
  try {
    const helper = join(root, "helper");
    const sent = join(root, "sent.json");
    // Records the request it reads and answers like an owner that accepted a batch.
    await writeFile(
      helper,
      `#!${process.execPath}\n{ await Bun.write(${JSON.stringify(sent)}, JSON.stringify({ args: process.argv.slice(2), body: await new Response(Bun.stdin.stream()).text() })); console.log(JSON.stringify({ ok: true, method: "batch", ids: ["r1"] })); }\n`,
      { mode: 0o755 },
    );
    const op = '{ "type": "set", "path": ["title"], "value": "hello \\"world\\"" }';
    const applied = await run(["apply", "a file.slop", "--op", op], { HITSLOP_NATIVE_CLI: helper, HITSLOP_ENGINE: helper });
    expect(applied.code).toBe(0);
    expect(JSON.parse(applied.stdout)).toEqual({ ids: ["r1"] });
    const { args, body } = JSON.parse(await Bun.file(sent).text());
    expect(args).toEqual([...selection, "request"]);
    // The operation reaches the core as written, inside the batch.
    expect(JSON.parse(body)).toEqual({ method: "batch", documentPath: join(process.cwd(), "a file.slop"), ops: `[${op}]` });
    // `--base` names the version the agent read; its text sets merge from there.
    const based = await run(["batch", "a file.slop", "--ops", `[${op}]`, "--base", "v1"], { HITSLOP_NATIVE_CLI: helper, HITSLOP_ENGINE: helper });
    expect(based.code).toBe(0);
    expect(JSON.parse(JSON.parse(await Bun.file(sent).text()).body).base).toBe("v1");
    const data = join(root, "new data.json");
    await writeFile(data, '{"n": 1.50}');
    const imported = await run(["import", "a file.slop", data, "--path", '["rows"]'], { HITSLOP_NATIVE_CLI: helper, HITSLOP_ENGINE: helper });
    expect(imported.code).toBe(0);
    expect(JSON.parse(JSON.parse(await Bun.file(sent).text()).body).ops).toBe('[{"type":"replace","path":["rows"],"value":{"n": 1.50}}]');
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

// Failure: a refused batch printed only the core's message ("No such row"), so an agent
// could not tell which of its operations to fix. Oracle: stderr names the operation and
// the reason, then whether anything was applied.
test("a refused edit names its operation and says whether it was applied", async () => {
  if (process.platform !== "darwin") return;
  const root = await mkdtemp(join(tmpdir(), "hsl-refused-"));
  try {
    const helper = join(root, "helper");
    await writeFile(
      helper,
      `#!${process.execPath}\nconsole.log(JSON.stringify({ ok: false, error: "No such row", code: "rejected", reason: "path_not_found", opIndex: 2 }));\n`,
      { mode: 0o755 },
    );
    const refused = await run(["batch", "a.slop", "--ops", "[]"], { HITSLOP_NATIVE_CLI: helper, HITSLOP_ENGINE: helper });
    expect(refused.code).not.toBe(0);
    expect(refused.stderr).toContain("Refused ops[2] (path_not_found): No such row\nNot applied.");
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

// Failure: a reply of {ok: true} printed {ids: []} for a batch whose result
// never arrived. Oracle: the exit status and stderr; nothing is printed as the result.
test("a success missing its method's result is an unknown outcome", async () => {
  if (process.platform !== "darwin") return;
  const root = await mkdtemp(join(tmpdir(), "hsl-bare-success-"));
  try {
    const helper = join(root, "helper");
    await writeFile(
      helper,
      `#!${process.execPath}\nconsole.log(JSON.stringify({ ok: true }));\n`,
      { mode: 0o755 },
    );
    for (const args of [["batch", "a.slop", "--ops", "[]"], ["export", "a.slop", "--format", "pdf", "--output", join(root, "a.pdf")]]) {
      const reply = await run(args, { HITSLOP_NATIVE_CLI: helper, HITSLOP_ENGINE: helper });
      expect(reply.code).not.toBe(0);
      expect(reply.stdout).toBe("");
      expect(reply.stderr).toContain("outcome unknown");
    }
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("create and open name this CLI's protocol to the selected engine or native helper", async () => {
  if (process.platform !== "darwin") return;
  const root = await mkdtemp(join(tmpdir(), "hsl-create-open-"));
  try {
    const helper = async (name: string, serves: boolean) => {
      const path = join(root, `helper-${name}`);
      await writeFile(
        path,
        `#!${process.execPath}\nif (!${serves}) { console.error("Unsupported client protocol"); process.exit(2); }\nconsole.log(JSON.stringify(process.argv.slice(2)));\n`,
        { mode: 0o755 },
      );
      return path;
    };
    const matching = await helper("matching", true);
    const create = ["create", "--from", "a template.slop", "--output", "my doc.slop"];
    const created = await run(create, { HITSLOP_NATIVE_CLI: matching, HITSLOP_ENGINE: matching });
    expect(created.code).toBe(0);
    expect(JSON.parse(created.stdout)).toEqual([...selection, ...create]);
    const opened = await run(["open", "my doc.slop"], { HITSLOP_NATIVE_CLI: matching, HITSLOP_ENGINE: matching });
    expect(opened.code).toBe(0);
    expect(JSON.parse(opened.stdout)).toEqual([...selection, "open", "my doc.slop"]);
    const refused = await run(create, { HITSLOP_ENGINE: await helper("newer", false) });
    expect(refused.code).not.toBe(0);
    expect(refused.stdout).toBe("");
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});
