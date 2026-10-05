import { test, expect } from "bun:test";
import { Database } from "bun:sqlite";
import { mkdtemp, cp, readFile, writeFile, rm } from "node:fs/promises";
import { join } from "node:path";
import { buildTemplate } from "../src/template";
import { request } from "../src/native";

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
    await cp("packages/cli/templates/checklist", source, { recursive: true });
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
    const output = await buildTemplate(source, [renderer], join(root, "svelte.slop"));
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
    const output = await buildTemplate(source, [renderer], join(root, "probe.slop"));
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
    await buildTemplate("examples/slops/quick-checklist", [renderer], master);
    for (const name of ["preview", "icon"] as const) {
      const png = artwork(master, name);
      expect(png.subarray(0, 8).toString("hex")).toBe("89504e470d0a1a0a");
      if (name === "icon") {
        expect(png.readUInt32BE(16)).toBe(512);
        expect(png.readUInt32BE(20)).toBe(512);
      }
    }
    await buildTemplate("examples/slops/quick-checklist", [renderer], master);
    const before = await readFile(master);
    await expect(
      buildTemplate("examples/slops/quick-checklist", ["/usr/bin/false"], master),
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
    await expect(buildTemplate(badSource, [renderer], master)).rejects.toThrow(
      "Authored icon failed",
    );
    expect(await readFile(master)).toEqual(before);
    // A build never replaces a document.
    const document = join(root, "Document.slop");
    const created = Bun.spawn([renderer, "create", "--from", master, "--output", document], { stdout: "ignore", stderr: "pipe" });
    expect(await created.exited).toBe(0);
    const saved = await readFile(document);
    await expect(buildTemplate("examples/slops/quick-checklist", undefined, document)).rejects.toThrow("Refusing to replace a document");
    expect(await readFile(document)).toEqual(saved);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
}, 120000);
