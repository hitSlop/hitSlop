import { test, expect, spyOn } from "bun:test";
import { Database } from "bun:sqlite";
import { mkdtemp, mkdir, cp, readFile, writeFile, rm } from "node:fs/promises";
import { join } from "node:path";
import { PNG } from "pngjs";
import { buildTemplate } from "../../src/cli/template";
import { execute, request } from "../../src/cli/engine";
import { png as solidPng } from "./png-fixture";
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

// Inspect actual captured pixels: App onMount hooks never run in native capture.
test("generated preview renders theme defaults through the native capture engine", async () => {
  const root = await mkdtemp(join(process.cwd(), ".build-test-"));
  try {
    const source = join(root, "source");
    await cp("tests/apps/document", source, { recursive: true });
    await overrideSlop(source, { theme: '{ accent: "#123456" }' });
    await writeFile(join(source, "Preview.svelte"), '<div style="width:100%;height:240px;background:var(--slop-accent)"></div>');
    const output = await buildTemplate(source, join(root, "svelte.slop"));
    const png = artwork(output, "preview");
    const pixel = [...PNG.sync.read(png).data.subarray(0, 4)];
    expect(pixel).toEqual([18, 52, 86, 255]);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
}, 90000);

test("dedicated capture views receive the snapshot and mode as props without mounting the editor", async () => {
  const root = await mkdtemp(join(process.cwd(), ".build-test-"));
  const source = join(root, "source");
  try {
    await cp("tests/apps/document", source, { recursive: true });
    await writeFile(
      join(source, "Child.svelte"),
      `
      <script lang="ts">
        let { snapshot }: { snapshot: { title: string } } = $props();
        const previous = (globalThis as any).captureSnapshot;
        if (previous && snapshot !== previous) throw new Error("Child received another document");
        (globalThis as any).captureSnapshot = snapshot;
      </script>
      <h1>{snapshot.title}</h1>
    `,
    );
    await writeFile(
      join(source, "App.svelte"),
      `
      <script module lang="ts">
        throw new Error("The editor module evaluated during a native capture");
      </script>
    `,
    );
    await writeFile(join(source, "Icon.svelte"), '<script>import Child from "./Child.svelte"; let {snapshot} = $props();</script><Child {snapshot} /><Child {snapshot} />');
    await writeFile(join(source, "Preview.svelte"), `<script>
      import Child from "./Child.svelte";
      let {snapshot, mode} = $props();
      if (mode !== "preview") throw new Error("Expected preview mode, got " + mode);
    </script><Child {snapshot} />`);
    await rm(join(source, "Export.svelte"));
    const output = await buildTemplate(source, join(root, "probe.slop"));
    for (const name of ["preview", "icon"] as const) {
      const png = artwork(output, name);
      expect(png.subarray(0, 8).toString("hex")).toBe("89504e470d0a1a0a");
    }
    await writeFile(join(source, "Export.svelte"), `<script>
      import Child from "./Child.svelte";
      let {snapshot, mode} = $props();
      if (mode !== "export") throw new Error("Expected export mode, got " + mode);
    </script><Child {snapshot} />`);
    const template = await buildTemplate(source, join(root, "export.slop"));
    // Closed export renders a document's saved state; a template is refused.
    const document = join(root, "Working.slop");
    await execute({ method: "create", from: template, output: document });
    expect(await request({ method: "export", documentPath: document, format: "png", output: join(root, "export.png") })).toMatchObject({ ok: true });
  } finally {
    await rm(root, { recursive: true, force: true });
  }
}, 90000);

// Supplied artwork wins per role; the engine renders only the role the project lacks, in the
// same pack, and a build that renders reports nothing missing.
test("a build keeps the artwork a project supplies and renders the role it lacks", async () => {
  const root = await mkdtemp(join(process.cwd(), ".build-test-"));
  const warn = spyOn(console, "warn").mockImplementation(() => {});
  try {
    const source = join(root, "source");
    await cp("tests/apps/document", source, { recursive: true });
    await mkdir(join(source, "artwork"));
    await writeFile(join(source, "artwork/preview.png"), solidPng(640, 480));
    await overrideSlop(source, { artwork: "{ preview }" }, 'import preview from "./artwork/preview.png";');
    const output = await buildTemplate(source, join(root, "mixed.slop"));
    const dimensions = (image: Buffer) => [image.readUInt32BE(16), image.readUInt32BE(20)];
    // The supplied preview is stored as given; the icon is the app's own 512px canvas.
    expect(dimensions(artwork(output, "preview"))).toEqual([640, 480]);
    expect(dimensions(artwork(output, "icon"))).toEqual([512, 512]);
    expect(warn).not.toHaveBeenCalled();
  } finally {
    warn.mockRestore();
    await rm(root, { recursive: true, force: true });
  }
}, 90000);

test("native artwork is complete before a rebuild replaces a registered template", async () => {
  const root = await mkdtemp(join(process.cwd(), ".build-test-"));
  try {
    // Registering builds into the template folder, replacing an earlier build.
    const templates = join(root, "templates");
    const master = join(templates, "document-fixture.slop");
    await buildTemplate("tests/apps/document", master);
    for (const name of ["preview", "icon"] as const) {
      const png = artwork(master, name);
      expect(png.subarray(0, 8).toString("hex")).toBe("89504e470d0a1a0a");
      if (name === "icon") {
        expect(png.readUInt32BE(16)).toBe(512);
        expect(png.readUInt32BE(20)).toBe(512);
      }
    }
    await buildTemplate("tests/apps/document", master);
    const before = await readFile(master);
    const badSource = join(root, "bad-capture-source");
    await cp("tests/apps/document", badSource, { recursive: true });
    await writeFile(
      join(badSource, "App.svelte"),
      `
      <script lang="ts">
        import { getDocument } from "hitslop/svelte";
        import schema from "./schema";
        const doc = getDocument(schema);
      </script>
      <h1>{doc.current.title}</h1>
    `,
    );
    await writeFile(join(badSource, "Icon.svelte"), '<script>function broken(){throw new Error("Authored icon failed");}</script><span>{broken()}</span>');
    await expect(buildTemplate(badSource, master)).rejects.toThrow("Authored icon failed");
    expect(await readFile(master)).toEqual(before);
    // A build never replaces a document.
    const document = join(root, "Document.slop");
    await execute({ method: "create", from: master, output: document });
    const saved = await readFile(document);
    await expect(buildTemplate("tests/apps/document", document)).rejects.toThrow("Refusing to replace a document");
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
    await slop("register", "tests/apps/document");
    expect(await Bun.file(join(templates, "document-fixture.slop")).exists()).toBe(true);
    const listed = JSON.parse(await slop("templates")).templates.filter((t: { source: string }) => t.source === "installed");
    expect(listed.map((t: { slug: string }) => t.slug)).toEqual(["document-fixture"]);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
}, 120000);
