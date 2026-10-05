import { test, expect } from "bun:test";
import { Database } from "bun:sqlite";
import { stageProject } from "../src/build";
import { engine, findEngine } from "../src/engine";
import { exec } from "../src/process";
import { mkdtemp, cp, readFile, writeFile, rm } from "node:fs/promises";
import { join, resolve } from "node:path";
import { readdir, mkdir, symlink } from "node:fs/promises";
import { overrideSlop } from "./source-fixture";
import { parseManifest, PackageFormat, RuntimeABI } from "@hitslop/schema";
import { buildTemplate } from "../src/template";

/** A project's stage: what the build compiles and evaluates, before the engine packs it. */
async function stage(source: string, output: string) {
  await stageProject(source, output);
  return output;
}
/** What the engine reads in a built file. */
const inspect = async (file: string) => JSON.parse(await engine(["inspect", file]));
/** What a new document of a built template holds: created and read by the engine. */
async function initialValue(template: string) {
  const document = join(template + ".created", "Document.slop");
  await engine(["create", "--from", template, "--output", document]);
  try {
    const { stdout } = await exec([await findEngine(), "request"], { stdin: JSON.stringify({ method: "get", documentPath: document }) });
    return JSON.parse(stdout).state.state.value;
  } finally {
    await rm(template + ".created", { recursive: true, force: true });
  }
}
/** One value from a built file, read outside the engine. */
function read<T>(file: string, sql: string): T {
  const database = new Database(file, { readonly: true });
  try {
    return database.query(sql).get() as T;
  } finally {
    database.close();
  }
}
// Built apps import nothing from the runtime and reach the host only through ctx.
test("apps contain no runtime code and cannot reach the engine, bridge or remote boot resources", async () => {
  const root = await mkdtemp(join(process.cwd(), ".build-test-"));
  try {
    const source = join(root, "source");
    await cp("examples/slops/quick-checklist", source, { recursive: true });
    const output = await stage(source, join(root, "built"));
    const js = await readFile(join(output, "assets/app.js"), "utf8");
    expect(js).not.toContain("/__shell__/");
    expect(js).not.toContain("loro_wasm_bg");
    expect(await readdir(output)).not.toContain("app.html");
    const app = await readFile(join(source, "App.svelte"), "utf8");
    for (const [code, error] of [
      // Apps never embed a document engine; the host's core owns the document.
      ['import {LoroDoc} from "loro-crdt"; console.log(new LoroDoc());', "cannot import loro-crdt"],
      ['import("/__shell__/index.js").then(console.log);', "cannot import /__shell__/index.js"],
      ["globalThis.webkit.messageHandlers.storage.postMessage({ method: 'ready' });", "host bridge"],
      ['import "./remote.css";', "remote stylesheets, fonts or scripts"],
    ] as const) {
      await writeFile(join(source, "remote.css"), '@font-face { font-family: R; src: url("https://example.com/r.woff2"); }');
      await writeFile(join(source, "App.svelte"), app.replace('<script lang="ts">', `<script lang="ts">\n${code}`));
      await expect(stage(source, join(root, "bad"))).rejects.toThrow(error);
    }
  } finally {
    await rm(root, { recursive: true, force: true });
  }
}, 60000);

// A theme is the colors a person may override; fonts and derived colors belong in CSS.
test("build refuses a theme that is not a palette of hex colors", async () => {
  const root = await mkdtemp(join(process.cwd(), ".build-test-"));
  try {
    for (const [name, theme, error] of [
      ["font", `{ font: '"Avenir Next", sans-serif' }`, "Theme color font must be lowercase"],
      ["opaque", `{ accent: "#aabbccff" }`, "Theme color accent"],
      ["derived", `{ rule: "color-mix(in srgb, var(--slop-ink) 14%, transparent)" }`, "Theme color rule"],
    ] as const) {
      const source = join(root, name);
      await cp("examples/slops/quick-checklist", source, { recursive: true });
      await overrideSlop(source, { theme });
      await expect(stage(source, join(root, name))).rejects.toThrow(error);
    }
  } finally {
    await rm(root, { recursive: true, force: true });
  }
}, 60000);

// slop.ts declares the app's row: the build evaluates it, checks it as the app opens files,
// and never ships it in the app.
test("slop.ts declares the app without shipping in app.js and is checked like a stored app", async () => {
  const root = await mkdtemp(join(process.cwd(), ".build-test-"));
  try {
    const source = join(root, "sentinel");
    await cp("examples/slops/quick-checklist", source, { recursive: true });
    const sentinel = `slop-ts-sentinel-${crypto.randomUUID()}`;
    await overrideSlop(source, { initial: `{ ...slop.initial, title: ${JSON.stringify(sentinel)} }` });
    const output = await stage(source, join(root, "built"));
    const app = JSON.parse(await readFile(join(output, "app.json"), "utf8"));
    expect(app.manifest.slug).toBe("sentinel");
    expect(app.initial.title).toBe(sentinel);
    expect(await readFile(join(output, "assets/app.js"), "utf8")).not.toContain(sentinel);
    for (const [name, fields, statements, error] of [
      ["mismatch", { schema: "defineDocument({ title: s.text() })" }, 'import { defineDocument, s } from "@hitslop/document";', "slop.ts: schema must be schema.ts's default export"],
      ["unknown", { lineage: '"future"' }, "", 'Invalid manifest at /'],
      ["field", { title: '""' }, "", "Invalid manifest at /title"],
      ["no-initial", { initial: "undefined" }, "", "slop.ts: initial is required"],
      ["no-theme", { theme: "undefined" }, "", "slop.ts: theme is required"],
      ["css-import", {}, 'import "./styles.css";', "slop.ts: styles.css cannot be imported here"],
      ["outside-import", {}, 'import "../sentinel/schema";', "slop.ts: ../sentinel/schema.ts is outside the project"],
      ["slug", { slug: '"other-slug"' }, "", "slop.ts: remove slug"],
      ["initial", { initial: "{ ...slop.initial, title: 42 }" }, "", "type_mismatch"],
      ["theme", { theme: '{ accent: "#ABCDEF" }' }, "", "Theme color"],
    ] as const) {
      const project = join(root, name);
      await cp("examples/slops/quick-checklist", project, { recursive: true });
      await overrideSlop(project, fields, statements);
      await expect(stage(project, join(root, name + "-stage"))).rejects.toThrow(error);
    }
    const misnamed = join(root, "Quick Checklist");
    await cp("examples/slops/quick-checklist", misnamed, { recursive: true });
    await expect(stage(misnamed, join(root, "misnamed-stage"))).rejects.toThrow("folder's name is its slug");
  } finally {
    await rm(root, { recursive: true, force: true });
  }
}, 60000);

// slop.ts is build-only: an app that imports it would ship author metadata and initial
// values, and run build-time code in the page.
test("an app that imports slop.ts is refused", async () => {
  const root = await mkdtemp(join(process.cwd(), ".build-test-"));
  try {
    const source = join(root, "imports-slop");
    await cp("examples/slops/quick-checklist", source, { recursive: true });
    const app = await readFile(join(source, "App.svelte"), "utf8");
    await writeFile(
      join(source, "App.svelte"),
      app.replace('<script lang="ts">', '<script lang="ts">\nimport slop from "./slop";\nconsole.log(slop.initial.title);'),
    );
    await expect(stage(source, join(root, "stage"))).rejects.toThrow("slop.ts is build-only");
  } finally {
    await rm(root, { recursive: true, force: true });
  }
}, 60000);

// `slop check` evaluates slop.ts as a build does, so a value only the app's rules refuse
// (here a color the types accept) fails the check.
test("slop check reports slop.ts errors a build would refuse", async () => {
  const root = await mkdtemp(join(process.cwd(), ".build-test-"));
  try {
    const source = join(root, "checked");
    await cp("examples/slops/quick-checklist", source, { recursive: true });
    const check = async () => {
      const child = Bun.spawn([process.execPath, "packages/cli/src/cli.ts", "check", source], { stdout: "ignore", stderr: "pipe" });
      const [stderr, code] = await Promise.all([new Response(child.stderr).text(), child.exited]);
      return { stderr, code };
    };
    expect((await check()).code).toBe(0);
    await overrideSlop(source, { theme: '{ ...slop.theme, accent: "#ABCDEF" }' });
    const failed = await check();
    expect(failed.code).not.toBe(0);
    expect(failed.stderr).toContain("Theme color");
  } finally {
    await rm(root, { recursive: true, force: true });
  }
}, 120000);

test("init creates a buildable source and refuses to overwrite it", async () => {
  const root = await mkdtemp(join(process.cwd(), ".build-test-"));
  const source = join(root, "starter");
  try {
    const run = () =>
      Bun.spawn([process.execPath, "packages/cli/src/cli.ts", "init", source], {
        stdout: "ignore",
        stderr: "ignore",
      }).exited;
    expect(await run()).toBe(0);
    expect(await run()).toBe(1);
    const metadata = JSON.parse(await readFile(join(source, "package.json"), "utf8"));
    expect(metadata.dependencies["@hitslop/document"]).not.toContain("__HITSLOP");
    const built = await inspect(await buildTemplate(source, undefined, join(root, "starter.slop")));
    expect(built.kind).toBe("template");
    expect(built.manifest.slug).toBeTruthy();
    // The file names the levels it needs beside its authored manifest, so an older app
    // refuses it up front.
    expect(built.packageFormat).toBe(PackageFormat);
    expect(built.runtimeABI).toBe(RuntimeABI);
    expect(() => parseManifest(built.manifest)).not.toThrow();
  } finally {
    await rm(root, { recursive: true, force: true });
  }
}, 60000);

test("build paths and fresh source evaluation do not depend on authored stdout", async () => {
  const root = await mkdtemp(join(process.cwd(), ".build-test-"));
  try {
    // The folder's name is the slug; the path above it may hold spaces.
    const source = join(root, "path with spaces", "starter");
    await cp("packages/cli/templates/checklist", source, { recursive: true });
    await overrideSlop(source, {}, 'console.log("authored output is not a path");');
    const defaultOutput = await buildTemplate(source, undefined);
    expect(defaultOutput).toBe(join(source, "dist", "starter.slop"));
    const first = await initialValue(defaultOutput);
    await overrideSlop(source, { initial: JSON.stringify({ ...first, title: "Fresh evaluation" }) }, 'console.log("another log");');
    const explicitOutput = join(root, "output with spaces.slop");
    expect(await buildTemplate(source, undefined, explicitOutput)).toBe(explicitOutput);
    expect((await initialValue(explicitOutput)).title).toBe("Fresh evaluation");
  } finally {
    await rm(root, { recursive: true, force: true });
  }
}, 60000);

test("copied fonts retain their URLs without duplicate bundles", async () => {
  const root = await mkdtemp(join(process.cwd(), ".build-test-"));
  try {
    const source = join(root, "source");
    await cp("examples/slops/quick-checklist", source, { recursive: true });
    await mkdir(join(source, "assets/fonts"), { recursive: true });
    await mkdir(join(source, "dependency"));
    await writeFile(join(source, "assets/fonts/My Font.ttf"), "copied-font");
    await writeFile(join(source, "assets/fonts/OFL.txt"), "font license");
    await writeFile(join(source, "dependency/External.woff2"), "dependency-font");
    await writeFile(
      join(source, "font-test.css"),
      `
      @font-face { font-family: Local; src: url('./assets/fonts/My Font.ttf'); }
      @font-face { font-family: Direct; src: url('/assets/fonts/My%20Font.ttf'); }
      @font-face { font-family: External; src: url('./dependency/External.woff2'); }
    `,
    );
    await writeFile(
      join(source, "App.svelte"),
      `<script>
      import './font-test.css';
      import fontURL from './assets/fonts/My Font.ttf';
      console.log(fontURL);
      </script><p>Fonts</p>`,
    );
    const output = await stage(source, join(root, "fonts"));
    const files = await readdir(join(output, "assets"), { recursive: true });
    expect(files.filter((file) => file.endsWith(".ttf"))).toEqual(["fonts/My Font.ttf"]);
    expect(files.filter((file) => file.endsWith(".woff2"))).toHaveLength(1);
    expect(await readFile(join(output, "assets/fonts/OFL.txt"), "utf8")).toBe("font license");
    expect(await readFile(join(output, "assets/app.css"), "utf8")).toContain(
      "/assets/fonts/My%20Font.ttf",
    );
    expect(await readFile(join(output, "assets/app.js"), "utf8")).toContain(
      "/assets/fonts/My%20Font.ttf",
    );
  } finally {
    await rm(root, { recursive: true, force: true });
  }
}, 60000);

// Remote cache keys exclude checkout paths. Identical sources must therefore emit
// identical portable bytes; the existing build tests use only one source location.
test("Svelte styles compile identically in different checkout locations", async () => {
  const root = await mkdtemp(join(process.cwd(), ".build-test-"));
  try {
    const outputs: string[] = [];
    for (const location of ["first-checkout", "second-checkout"]) {
      const source = join(root, location);
      await cp("packages/cli/templates/checklist", source, { recursive: true });
      await writeFile(
        join(source, "App.svelte"),
        "<p>Portable styles</p><style>p { color: rebeccapurple; }</style>",
      );
      const built = await stage(source, join(root, location + "-stage"));
      outputs.push(await readFile(join(built, "assets/app.js"), "utf8"));
    }
    expect(outputs[0]).toBe(outputs[1]);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
}, 60000);

for (const [name, properties, initial, code] of [
  ["oversized string bound", '{value:s.string({maxLength:5_000_000})}', { value: "" }, "invalid_schema"],
  ["optional handle name", '{value:s.optional(s.object({set:s.string()}))}', {}, "invalid_schema"],
  ["invalid row id", '{rows:s.list(s.object({value:s.string()}))}', { rows: [{ $id: "bad id", value: "" }] }, "invalid_id"],
  ["duplicate row id", '{rows:s.list(s.object({value:s.string()}))}', { rows: [{ $id: "same", value: "a" }, { $id: "same", value: "b" }] }, "duplicate_id"],
] as const) test(`build enforces core validation: ${name}`, async () => {
  const root = await mkdtemp(join(process.cwd(), ".core-validation-test-"));
  try {
    const source = join(root, "source");
    await cp("examples/slops/quick-checklist", source, { recursive: true });
    await writeFile(join(source, "schema.ts"), `import {defineDocument,s} from '@hitslop/document'; export default defineDocument(${properties});`);
    await overrideSlop(source, { initial: JSON.stringify(initial) });
    await expect(stage(source, join(root, "invalid"))).rejects.toThrow(code);
  } finally { await rm(root, { recursive: true, force: true }); }
});

test("only generated Svelte entries build and capture discovery is exact-case", async () => {
  const root = await mkdtemp(join(process.cwd(), ".build-test-"));
  try {
    const source = join(root, "source");
    await cp("packages/cli/templates/checklist", source, { recursive: true });
    await writeFile(join(source, "Export.svelte"), '<script>const = ;</script>');
    await writeFile(join(source, "main.ts"), 'export default {mount(){return {rendered(){},unmount(){}}}};');
    await expect(stage(source, join(root, "custom"))).rejects.toThrow("main.ts is not supported");
    await rm(join(source, "main.ts"));
    await expect(stage(source, join(root, "discovered"))).rejects.toThrow();
    await rm(join(source, "Export.svelte"));
    await writeFile(join(source, "export.svelte"), '<script>const = ;</script>');
    await stage(source, join(root, "lowercase"));
  } finally {
    await rm(root, {recursive: true, force: true});
  }
}, 60000);

/** A minimal RGBA (colour type 6) PNG header: enough for the build's checks. */
function pngHeader(width: number, height: number, colorType = 6) {
  const bytes = Buffer.alloc(33);
  Buffer.from("89504e470d0a1a0a0000000d49484452", "hex").copy(bytes);
  bytes.writeUInt32BE(width, 16);
  bytes.writeUInt32BE(height, 20);
  bytes[24] = 8;
  bytes[25] = colorType;
  return bytes;
}

// Authoring builds on any platform: artwork is supplied or absent, and no helper runs.
test("a portable build copies supplied artwork and needs no helper", async () => {
  const root = await mkdtemp(join(process.cwd(), ".build-test-"));
  const previous = process.env.HITSLOP_NATIVE_CLI;
  // A helper lookup would fail on this path.
  process.env.HITSLOP_NATIVE_CLI = join(root, "missing-helper");
  try {
    const source = join(root, "source");
    await cp("examples/slops/quick-checklist", source, { recursive: true });
    const bare = await buildTemplate(source, undefined, join(root, "bare.slop"));
    expect((await inspect(bare)).artwork).toEqual([]);
    await mkdir(join(source, "artwork"));
    await writeFile(join(source, "artwork/preview.png"), pngHeader(640, 480));
    await writeFile(join(source, "artwork/icon.png"), pngHeader(512, 512));
    const output = await buildTemplate(source, undefined, join(root, "art.slop"));
    expect((await inspect(output)).artwork.map((artwork: { name: string }) => artwork.name)).toEqual(["icon", "preview"]);
    const icon = read<{ png: Uint8Array }>(output, "SELECT png FROM artwork WHERE name = 'icon'").png;
    expect(Buffer.from(icon)).toEqual(pngHeader(512, 512));
  } finally {
    if (previous === undefined) delete process.env.HITSLOP_NATIVE_CLI;
    else process.env.HITSLOP_NATIVE_CLI = previous;
    await rm(root, { recursive: true, force: true });
  }
}, 60000);

// The engine packs by the rules the app opens files with, on any platform.
test("a portable build refuses templates the app would refuse and keeps the previous output", async () => {
  const root = await mkdtemp(join(process.cwd(), ".build-test-"));
  try {
    const fixture = join(root, "fixture");
    await cp("examples/slops/quick-checklist", fixture, { recursive: true });
    const output = await buildTemplate(fixture, undefined, join(root, "out.slop"));
    const before = await readFile(output);
    const cases: [string, (source: string) => Promise<unknown>, string][] = [
      ["artwork", async (source) => {
        await mkdir(join(source, "artwork"));
        await writeFile(join(source, "artwork/preview.png"), "not a png");
      }, "artwork/preview.png must be a valid PNG"],
      ["symlink", async (source) => {
        await mkdir(join(source, "assets"), { recursive: true });
        await symlink("/etc/hosts", join(source, "assets/hosts"));
      }, "without symbolic links"],
      ["entries", async (source) => {
        await mkdir(join(source, "assets/many"), { recursive: true });
        for (let index = 0; index < 260; index++) await writeFile(join(source, "assets/many", `${index}.txt`), "");
      }, "exceeds 256 assets"],
      ["skin", async (source) => {
        // Quick Checklist's window is 480 × 620.
        await overrideSlop(source, { presentation: '{ width: 480, height: 620, skin: "assets/skin.png" }' });
        await mkdir(join(source, "assets"), { recursive: true });
        await writeFile(join(source, "assets/skin.png"), pngHeader(481, 620));
      }, "window skin must match the window's width and height"],
    ];
    for (const [name, damage, error] of cases) {
      const source = join(root, name);
      await cp(fixture, source, { recursive: true });
      await damage(source);
      await expect(buildTemplate(source, undefined, output)).rejects.toThrow(error);
      expect(await readFile(output)).toEqual(before);
    }
  } finally {
    await rm(root, { recursive: true, force: true });
  }
}, 120000);

// The CLI validates and previews only the runtimes it knows, so it refuses a newer SDK
// before building anything.
test("build refuses a project SDK that needs a newer runtime than this CLI", async () => {
  const root = await mkdtemp(join(process.cwd(), ".build-test-"));
  try {
    const source = join(root, "source");
    await cp("examples/slops/quick-checklist", source, { recursive: true });
    const sdk = join(source, "node_modules/@hitslop/document");
    await mkdir(sdk, { recursive: true });
    await writeFile(join(sdk, "package.json"), JSON.stringify({ name: "@hitslop/document", type: "module", exports: { ".": "./index.js", "./abi": "./abi.js" } }));
    await writeFile(join(sdk, "index.js"), `export * from ${JSON.stringify(resolve("packages/document/src/schema.ts"))};`);
    await writeFile(join(sdk, "abi.js"), `export const RuntimeABI = ${RuntimeABI + 1};`);
    const output = join(root, "out.slop");
    await expect(buildTemplate(source, undefined, output)).rejects.toThrow("Update @hitslop/cli");
    expect(await readdir(root)).toEqual(["source"]);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
}, 60000);
