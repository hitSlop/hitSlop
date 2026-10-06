import { test, expect } from "bun:test";
import { Database } from "bun:sqlite";
import { mkdtemp, cp, readFile, writeFile, rm } from "node:fs/promises";
import { dirname, join } from "node:path";
import { buildTemplate } from "../../src/cli/template";
import { execute, request } from "../../src/cli/engine";

/** The helper `bun run verify native` names, which renders native artwork. */
const renderer = process.env.HITSLOP_NATIVE_CLI!;
import { overrideSlop } from "./source-fixture";

/** A built file's preview or icon, read outside the engine. */
function artwork(file: string, name: "preview" | "icon"): Buffer {
  const database = new Database(file, { readonly: true });
  try {
    const row = database.query("SELECT png FROM artwork WHERE name = ?").get(name) as { png: Uint8Array } | null;
    if (!row) throw new Error(`No ${name} artwork in ${file}`);
    return Buffer.from(row.png);
  } finally {
    database.close();
  }
}

// The shell applies theme defaults before mount, so the first view is never unstyled.
// Existing controller tests do not mount apps.
test("generated Svelte app mounts with theme defaults before native capture", async () => {
  const root = await mkdtemp(join(process.cwd(), ".build-test-"));
  try {
    const source = join(root, "source");
    await cp("packages/hitslop/templates/checklist", source, { recursive: true });
    await overrideSlop(source, { theme: '{ accent: "#123456" }' });
    await writeFile(join(source, "App.svelte"), `
      <script>
        import { onMount } from "svelte";
        import doc from "./schema";
        let root;
        onMount(() => {
          if (getComputedStyle(root).backgroundColor !== "rgb(18, 52, 86)")
            throw new Error("Theme defaults were not applied before mount");
        });
      </script>
      <main bind:this={root} style="background:var(--slop-accent)">{doc.current.title}</main>
    `);
    const output = await buildTemplate(source, { env: { ...process.env, HITSLOP_NATIVE_CLI: renderer } }, join(root, "svelte.slop"));
    const png = artwork(output, "preview");
    expect(png.subarray(0, 8).toString("hex")).toBe("89504e470d0a1a0a");
  } finally {
    await rm(root, { recursive: true, force: true });
  }
}, 90000);

test("discovered capture components share the document and receive preview/export mode", async () => {
  const root = await mkdtemp(join(process.cwd(), ".build-test-"));
  const source = join(root, "source");
  try {
    await cp("examples/slops/quick-checklist", source, { recursive: true });
    await writeFile(
      join(source, "Child.svelte"),
      `
      <script lang="ts">
        import doc from "./schema";
        if (doc.fields !== (globalThis as any).appFields) throw new Error("Child received another document");
      </script>
      <h1>{doc.current.title}</h1>
    `,
    );
    await writeFile(
      join(source, "App.svelte"),
      `
      <script lang="ts">
        import Child from "./Child.svelte";
        import doc from "./schema";
        (globalThis as any).appFields = doc.fields;
      </script>
      <Child />
    `,
    );
    await writeFile(join(source, "Icon.svelte"), '<script>import Child from "./Child.svelte";</script><Child />');
    await writeFile(join(source, "Export.svelte"), `<script>
      import Child from "./Child.svelte";
      let {mode} = $props();
      if (mode !== "preview") throw new Error("Expected preview mode, got " + mode);
    </script><Child />`);
    const output = await buildTemplate(source, { env: { ...process.env, HITSLOP_NATIVE_CLI: renderer } }, join(root, "probe.slop"));
    for (const name of ["preview", "icon"] as const) {
      const png = artwork(output, name);
      expect(png.subarray(0, 8).toString("hex")).toBe("89504e470d0a1a0a");
    }
    await writeFile(join(source, "Export.svelte"), `<script>
      import Child from "./Child.svelte";
      let {mode} = $props();
      if (mode !== "export") throw new Error("Expected export mode, got " + mode);
    </script><Child />`);
    const exported = await buildTemplate(source, undefined, join(root, "export.slop"));
    expect(await request({ method: "export", documentPath: exported, format: "png", output: join(root, "export.png") })).toMatchObject({ ok: true });
  } finally {
    await rm(root, { recursive: true, force: true });
  }
}, 90000);

test("native artwork is complete before a rebuild replaces a registered template", async () => {
  const root = await mkdtemp(join(process.cwd(), ".build-test-"));
  try {
    // Registering builds into the template folder, replacing an earlier build.
    const templates = join(root, "templates");
    const master = join(templates, "quick-checklist.slop");
    await buildTemplate("examples/slops/quick-checklist", { env: { ...process.env, HITSLOP_NATIVE_CLI: renderer } }, master);
    for (const name of ["preview", "icon"] as const) {
      const png = artwork(master, name);
      expect(png.subarray(0, 8).toString("hex")).toBe("89504e470d0a1a0a");
      if (name === "icon") {
        expect(png.readUInt32BE(16)).toBe(512);
        expect(png.readUInt32BE(20)).toBe(512);
      }
    }
    await buildTemplate("examples/slops/quick-checklist", { env: { ...process.env, HITSLOP_NATIVE_CLI: renderer } }, master);
    const before = await readFile(master);
    await expect(
      buildTemplate("examples/slops/quick-checklist", { binary: "/usr/bin/false" }, master),
    ).rejects.toThrow();
    expect(await readFile(master)).toEqual(before);
    const badSource = join(root, "bad-capture-source");
    await cp("examples/slops/quick-checklist", badSource, { recursive: true });
    await writeFile(
      join(badSource, "App.svelte"),
      `
      <script lang="ts">
        import doc from "./schema";
      </script>
      <h1>{doc.current.title}</h1>
    `,
    );
    await writeFile(join(badSource, "Icon.svelte"), '<script>function broken(){throw new Error("Authored icon failed");}</script><span>{broken()}</span>');
    await expect(buildTemplate(badSource, { env: { ...process.env, HITSLOP_NATIVE_CLI: renderer } }, master)).rejects.toThrow(
      "Authored icon failed",
    );
    expect(await readFile(master)).toEqual(before);
    // A build never replaces a document.
    const document = join(root, "Document.slop");
    await execute({ method: "create", from: master, output: document });
    const saved = await readFile(document);
    await expect(buildTemplate("examples/slops/quick-checklist", undefined, document)).rejects.toThrow("Refusing to replace a document");
    expect(await readFile(document)).toEqual(saved);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
}, 120000);

// Failure: register built into $HOME/.hitslop/templates whatever folder the app lists, so
// with HITSLOP_TEMPLATES_ROOT set the template never reached the catalog. Oracle: the file
// in the listed folder, and `templates` listing it as installed.
test("register builds into the installed folder the catalog lists", async () => {
  const root = await mkdtemp(join(process.cwd(), ".build-test-"));
  try {
    const templates = join(root, "templates");
    // A stand-in home, so a register that ignores the listed folder never writes the real one.
    const env = { ...process.env, HITSLOP_TEMPLATES_ROOT: templates, HOME: join(root, "home") };
    const slop = async (...args: string[]) => {
      const child = Bun.spawn([process.execPath, "packages/hitslop/src/cli/cli.ts", ...args], { env, stdout: "pipe", stderr: "pipe" });
      const [out, error, code] = await Promise.all([new Response(child.stdout).text(), new Response(child.stderr).text(), child.exited]);
      expect(code, error).toBe(0);
      return out;
    };
    await slop("register", "examples/slops/quick-checklist");
    expect(await Bun.file(join(templates, "quick-checklist.slop")).exists()).toBe(true);
    const listed = JSON.parse(await slop("templates")).templates.filter((t: { source: string }) => t.source === "installed");
    expect(listed.map((t: { slug: string }) => t.slug)).toEqual(["quick-checklist"]);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
}, 120000);
