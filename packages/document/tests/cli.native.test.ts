import { test, expect } from "bun:test";
import { mkdtemp, mkdir, writeFile, readFile, rm, readdir, symlink } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { defineDocument, s } from "../src/schema";
import identity from "../src/runtime-identity.json";
const schema = defineDocument({ title: s.text() });

test("JSON CLI creates atomically, replaces across closed sessions and rejects stale versions", async () => {
  const parent = await mkdtemp(join(tmpdir(), "hsl-import-"));
  const template = join(parent, "Template.slop"),
    destination = join(parent, "New.slop"),
    file = join(parent, "mapped.json");
  const cli = async (...args: string[]) => {
    const child = Bun.spawn([process.execPath, "packages/cli/src/cli.ts", ...args], {
      env: {
        ...process.env,
        HITSLOP_NATIVE_CLI: new URL(
          "../../../apps/apple/Packages/HitSlopApple/.build/debug/hitslop-native",
          import.meta.url,
        ).pathname,
      },
      stdout: "pipe",
      stderr: "pipe",
    });
    const [out, error, code] = await Promise.all([
      new Response(child.stdout).text(),
      new Response(child.stderr).text(),
      child.exited,
    ]);
    return { out, error, code };
  };
  try {
    await mkdir(join(template, "assets"), { recursive: true });
    const manifest = await readFile("examples/slops/quick-checklist/manifest.json", "utf8");
    const { runtimeRevision, ...provenance } = identity;
    const files = {
      "manifest.json": manifest,
      "assets/app.js": "throw new Error('Authored code must not run');",
      "initial.json": JSON.stringify({ title: "Template" }),
      "state.schema.json": JSON.stringify(schema.descriptor),
      "assets/theme.json": "{}",
      "assets/runtime.json": JSON.stringify({ ...provenance, minRuntimeRevision: runtimeRevision }),
    };
    for (const [path, bytes] of Object.entries(files)) await writeFile(join(template, path), bytes);
    await writeFile(file, JSON.stringify({ title: 42 }));
    expect((await cli("import", destination, "--from", template, "--file", file)).code).not.toBe(0);
    expect(await readdir(parent)).not.toContain("New.slop");
    expect((await readdir(parent)).some((name) => name.startsWith(".hitslop-import-"))).toBe(false);
    await writeFile(file, JSON.stringify({ title: "Imported" }));
    const created = await cli("import", destination, "--from", template, "--file", file);
    expect(created.error).toBe("");
    expect(created.code).toBe(0);
    const snapshot = JSON.parse(created.out);
    expect(snapshot.data).toEqual({ title: "Imported" });
    expect(JSON.parse((await cli("get", destination, "--snapshot")).out).version).toBe(
      snapshot.version,
    );
    expect((await cli("import", destination, "--from", template, "--file", file)).code).not.toBe(0);
    // Larger than the ordinary 1 MiB socket request budget; the same import bound applies closed/live.
    const text = "bulk ".repeat(230_000);
    await writeFile(file, JSON.stringify({ title: text }));
    const replaced = await cli(
      "import",
      destination,
      "--replace",
      "--if-version",
      snapshot.version,
      "--file",
      file,
    );
    expect(replaced.error).toBe("");
    expect(replaced.code).toBe(0);
    expect(JSON.parse(replaced.out).data.title === text).toBe(true);
    const stale = await cli(
      "import",
      destination,
      "--replace",
      "--if-version",
      snapshot.version,
      "--file",
      file,
    );
    expect(stale.code).not.toBe(0);
    expect(stale.error).toContain("Destination changed");
    const link = join(parent, "link.json");
    await symlink(file, link);
    expect(
      (
        await cli(
          "import",
          destination,
          "--replace",
          "--if-version",
          JSON.parse(replaced.out).version,
          "--file",
          link,
        )
      ).code,
    ).not.toBe(0);
    expect(await readdir(template)).not.toContain("state");
    for (const [path, bytes] of Object.entries(files))
      expect(await readFile(join(template, path), "utf8")).toBe(bytes);
  } finally {
    await rm(parent, { recursive: true, force: true });
  }
}, 120000);
// A configured catalog root must be refused before a command can create mutable state.
test("native CLI refuses an environment-configured template master before mutation", async () => {
  if (process.platform !== "darwin") return;
  const parent = await mkdtemp(join(tmpdir(), "hsl-master-v1-"));
  const root = join(parent, "Master.slop");
  try {
    await mkdir(join(root, "assets"), { recursive: true });
    const manifest = await readFile("examples/slops/quick-checklist/manifest.json", "utf8");
    const { runtimeRevision, ...provenance } = identity;
    const files: Record<string, string> = {
      "manifest.json": manifest,
      "assets/app.js": "export default { mount() { return {}; } };",
      "state.schema.json": JSON.stringify(schema.descriptor),
      "initial.json": JSON.stringify({ title: "Master" }),
      "assets/theme.json": "{}",
      "assets/runtime.json": JSON.stringify({ ...provenance, minRuntimeRevision: runtimeRevision }),
    };
    for (const [path, bytes] of Object.entries(files)) await writeFile(join(root, path), bytes);
    const child = Bun.spawn(
      [
        process.execPath,
        "packages/cli/src/cli.ts",
        "apply",
        root,
        "--op",
        JSON.stringify({ type: "text.replace", path: ["title"], value: "Changed" }),
      ],
      {
        env: {
          ...process.env,
          HITSLOP_TEMPLATES_ROOT: parent,
          HITSLOP_NATIVE_CLI: new URL(
            "../../../apps/apple/Packages/HitSlopApple/.build/debug/hitslop-native",
            import.meta.url,
          ).pathname,
        },
        stdout: "pipe",
        stderr: "pipe",
      },
    );
    const [, stderr, code] = await Promise.all([
      new Response(child.stdout).text(),
      new Response(child.stderr).text(),
      child.exited,
    ]);
    expect(code).not.toBe(0);
    expect(stderr).toContain("writable copy");
    expect(await readdir(root)).not.toContain("state");
    for (const [path, bytes] of Object.entries(files))
      expect(await readFile(join(root, path), "utf8")).toBe(bytes);
  } finally {
    await rm(parent, { recursive: true, force: true });
  }
}, 60000);
test("native CLI rejects old runtimes and malformed commands before mutation", async () => {
  if (process.platform !== "darwin") return;
  const parent = await mkdtemp(join(tmpdir(), "hsl-cli-v1-"));
  const root = join(parent, "Document.slop");
  await (await import("node:fs/promises")).mkdir(root);
  const cli = async (command: string, ...args: string[]) => {
    const p = Bun.spawn([process.execPath, "packages/cli/src/cli.ts", command, root, ...args], {
      env: {
        ...process.env,
        HITSLOP_NATIVE_CLI: new URL(
          "../../../apps/apple/Packages/HitSlopApple/.build/debug/hitslop-native",
          import.meta.url,
        ).pathname,
      },
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
  try {
    await writeFile(join(root, "manifest.json"), JSON.stringify({ runtime: "old-experiment" }));
    expect((await cli("get")).error).toContain("invalid v1 manifest");
    expect(await readdir(root)).toEqual(["manifest.json"]);
    const manifest = JSON.parse(
      await readFile("examples/slops/quick-checklist/manifest.json", "utf8"),
    );
    await writeFile(join(root, "manifest.json"), JSON.stringify(manifest));
    await mkdir(join(root, "assets"));
    await writeFile(join(root, "assets/theme.json"), "{}");
    await writeFile(join(root, "assets/app.js"), "export default { mount() { return {}; } };");
    await writeFile(join(root, "state.schema.json"), JSON.stringify(schema.descriptor));
    await writeFile(join(root, "initial.json"), JSON.stringify({ title: "Initial" }));
    expect(
      (
        await cli(
          "apply",
          "--op",
          '{"type":"text.replace","path":["title"],"value":"bad"}',
          "--id",
          "alone",
        )
      ).error,
    ).toContain("Unknown");
    expect((await cli("get", "--request-id", "old-name")).error).toContain("Unknown");
    expect(await readdir(root)).not.toContain("state");
    expect((await cli("get")).error).toContain("Missing runtime requirements");
    expect(await readdir(root)).not.toContain("state");
    const { runtimeRevision, ...provenance } = identity;
    await writeFile(
      join(root, "assets/runtime.json"),
      JSON.stringify({ ...provenance, minRuntimeRevision: runtimeRevision }),
    );
    expect((await cli("get")).code).toBe(0);
    const before = await readFile(join(root, "state/document.sqlite"));
    const requirements = await readFile(join(root, "assets/runtime.json"));
    await writeFile(
      join(root, "assets/runtime.json"),
      JSON.stringify({ ...provenance, runtimeContract: 99, minRuntimeRevision: 1 }),
    );
    expect((await cli("get")).error).toContain("requires runtime contract 99");
    expect(await readFile(join(root, "state/document.sqlite"))).toEqual(before);
    await writeFile(join(root, "assets/runtime.json"), requirements);
    expect((await cli("apply", "--op", "null")).code).not.toBe(0);
    expect((await cli("batch", "--ops", "null")).code).not.toBe(0);
    const changed = defineDocument({ title: s.text(), extra: s.string() });
    await writeFile(join(root, "state.schema.json"), JSON.stringify(changed.descriptor));
    expect((await cli("get")).error).toContain("Incompatible document schema");
    expect(await readFile(join(root, "state/document.sqlite"))).toEqual(before);
  } finally {
    await rm(parent, { recursive: true, force: true });
  }
}, 60000);
