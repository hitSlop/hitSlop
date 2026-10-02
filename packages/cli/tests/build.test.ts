import { test, expect } from "bun:test";
import { buildProject } from "../src/build";
import { mkdtemp, cp, readFile, writeFile, rm } from "node:fs/promises";
import { join, resolve } from "node:path";
import { readdir, mkdir, symlink } from "node:fs/promises";
import { copySourceFixture } from "./source-fixture";
import { parseManifest, parsePackageManifest, PackageFormat, RuntimeABI } from "@hitslop/schema";
import { buildTemplate } from "../src/template";
// Built apps import nothing from the runtime and reach the host only through ctx.
test("apps contain no runtime code and cannot reach the engine, bridge or remote boot resources", async () => {
  const root = await mkdtemp(join(process.cwd(), ".build-test-"));
  try {
    const source = join(root, "source");
    await copySourceFixture("examples/slops/quick-checklist", source);
    const output = await buildProject(source, join(root, "built.slop"));
    const js = await readFile(join(output, "assets/app.js"), "utf8");
    expect(js).not.toContain("/__shell__/");
    expect(js).not.toContain("loro_wasm_bg");
    expect(await readdir(output)).not.toContain("app.html");
    const entry = 'import App from "./App.svelte"; import schema from "./schema.ts"; import { defineSlop } from "@hitslop/document/svelte"; export default defineSlop(App, { schema });\n';
    for (const [code, error] of [
      // Apps never embed a document engine; the host's core owns the document.
      ['import {LoroDoc} from "loro-crdt"; console.log(new LoroDoc());', "cannot import loro-crdt"],
      ['const runtime = await import("/__shell__/index.js"); console.log(runtime);', "cannot import /__shell__/index.js"],
      ["globalThis.webkit.messageHandlers.storage.postMessage({ method: 'ready' });", "host bridge"],
      ['import "./remote.css";', "remote stylesheets, fonts or scripts"],
    ] as const) {
      await writeFile(join(source, "remote.css"), '@font-face { font-family: R; src: url("https://example.com/r.woff2"); }');
      await writeFile(join(source, "main.ts"), entry + code);
      await expect(buildProject(source, join(root, "bad.slop"))).rejects.toThrow(error);
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
      ["font", `{ font: '"Avenir Next", sans-serif' }`, "theme.ts: out_of_range: Theme color font must be lowercase"],
      ["opaque", `{ accent: "#aabbccff" }`, "theme.ts: out_of_range: Theme color accent"],
      ["derived", `{ rule: "color-mix(in srgb, var(--slop-ink) 14%, transparent)" }`, "Theme color rule"],
    ] as const) {
      const source = join(root, name);
      await copySourceFixture("examples/slops/quick-checklist", source);
      await writeFile(join(source, "theme.ts"), `export default { defaults: ${theme} };`);
      await expect(buildProject(source, join(root, name + ".slop"))).rejects.toThrow(error);
    }
  } finally {
    await rm(root, { recursive: true, force: true });
  }
}, 60000);

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
    const built = await buildProject(source, join(root, "starter.slop"));
    const manifest = JSON.parse(await readFile(join(built, "manifest.json"), "utf8"));
    expect(manifest.slug).toBeTruthy();
    // The built package names the level it needs, so an older app refuses it up front.
    expect(manifest.packageFormat).toBe(PackageFormat);
    expect(() => parseManifest(manifest)).toThrow();
    expect(parsePackageManifest(manifest).packageFormat).toBe(PackageFormat);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
}, 60000);

test("build paths and fresh source evaluation do not depend on authored stdout", async () => {
  const root = await mkdtemp(join(process.cwd(), ".build-test-"));
  try {
    const source = join(root, "source with spaces");
    await cp("packages/cli/templates/checklist", source, { recursive: true });
    const initialPath = join(source, "initial.ts");
    const original = await readFile(initialPath, "utf8");
    await writeFile(initialPath, original + '\nconsole.log("authored output is not a path");\n');
    const defaultOutput = await buildProject(source);
    expect(defaultOutput).toBe(join(source, "dist", "quick-checklist.slop"));
    const first = JSON.parse(await readFile(join(defaultOutput, "initial.json"), "utf8"));
    await writeFile(
      initialPath,
      `console.log("another log"); export default ${JSON.stringify({ ...first, title: "Fresh evaluation" })};`,
    );
    const explicitOutput = join(root, "output with spaces.slop");
    expect(await buildProject(source, explicitOutput)).toBe(explicitOutput);
    expect(JSON.parse(await readFile(join(explicitOutput, "initial.json"), "utf8")).title).toBe(
      "Fresh evaluation",
    );
  } finally {
    await rm(root, { recursive: true, force: true });
  }
}, 60000);

test("copied fonts retain their URLs without duplicate bundles", async () => {
  const root = await mkdtemp(join(process.cwd(), ".build-test-"));
  try {
    const source = join(root, "source");
    await copySourceFixture("examples/slops/quick-checklist", source);
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
      join(source, "main.ts"),
      `import "./styles.css"; import App from "./App.svelte"; import schema from "./schema.ts"; import { defineSlop } from "@hitslop/document/svelte"; export default defineSlop(App, { schema });` +
        `
      import './font-test.css';
      import fontURL from './assets/fonts/My Font.ttf';
      console.log(fontURL);
    `,
    );
    const output = await buildProject(source, join(root, "fonts.slop"));
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
        join(source, "Styled.svelte"),
        "<p>Portable styles</p><style>p { color: rebeccapurple; }</style>",
      );
      await writeFile(
        join(source, "main.ts"),
        'import { mount } from "svelte"; import Styled from "./Styled.svelte"; export default { mount: (ctx, target) => (mount(Styled, { target }), {}) };',
      );
      const built = await buildProject(source, join(root, `${location}.slop`));
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
    await copySourceFixture("examples/slops/quick-checklist", source);
    await writeFile(join(source, "schema.ts"), `import {defineDocument,s} from '@hitslop/document'; export default defineDocument(${properties});`);
    await writeFile(join(source, "initial.ts"), `export default ${JSON.stringify(initial)};`);
    await expect(buildProject(source, join(root, "invalid.slop"))).rejects.toThrow(code);
  } finally { await rm(root, { recursive: true, force: true }); }
});

test("custom main owns registration and conventional capture discovery is exact-case", async () => {
  const root = await mkdtemp(join(process.cwd(), ".build-test-"));
  try {
    const source = join(root, "source");
    await cp("packages/cli/templates/checklist", source, { recursive: true });
    await writeFile(join(source, "Export.svelte"), '<script>const = ;</script>');
    await writeFile(join(source, "main.ts"), 'export default {mount(){return {rendered(){},unmount(){}}}};');
    await buildProject(source, join(root, "custom.slop"));
    await rm(join(source, "main.ts"));
    await expect(buildProject(source, join(root, "discovered.slop"))).rejects.toThrow();
    await rm(join(source, "Export.svelte"));
    await writeFile(join(source, "export.svelte"), '<script>const = ;</script>');
    await buildProject(source, join(root, "lowercase.slop"));
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
    await copySourceFixture("examples/slops/quick-checklist", source);
    const bare = await buildTemplate(source, undefined, join(root, "bare.slop"));
    expect(await readdir(bare)).not.toContain("QuickLook");
    await mkdir(join(source, "artwork"));
    await writeFile(join(source, "artwork/preview.png"), pngHeader(640, 480));
    await writeFile(join(source, "artwork/icon.png"), pngHeader(512, 512));
    const output = await buildTemplate(source, undefined, join(root, "art.slop"));
    expect((await readdir(join(output, "QuickLook"))).sort()).toEqual(["Icon.png", "Preview.png"]);
    expect(await readFile(join(output, "QuickLook/Icon.png"))).toEqual(pngHeader(512, 512));
  } finally {
    if (previous === undefined) delete process.env.HITSLOP_NATIVE_CLI;
    else process.env.HITSLOP_NATIVE_CLI = previous;
    await rm(root, { recursive: true, force: true });
  }
}, 60000);

// Without the native open that rendering used to run, the build applies its package rules.
test("a portable build refuses packages the app would refuse and keeps the previous output", async () => {
  const root = await mkdtemp(join(process.cwd(), ".build-test-"));
  try {
    const fixture = join(root, "fixture");
    await copySourceFixture("examples/slops/quick-checklist", fixture);
    const output = await buildTemplate(fixture, undefined, join(root, "out.slop"));
    const before = await readFile(join(output, "assets/app.js"));
    const cases: [string, (source: string) => Promise<unknown>, string][] = [
      ["artwork", async (source) => {
        await mkdir(join(source, "artwork"));
        await writeFile(join(source, "artwork/preview.png"), "not a png");
      }, "QuickLook/Preview.png must be a valid PNG"],
      ["symlink", async (source) => {
        await mkdir(join(source, "assets"), { recursive: true });
        await symlink("/etc/hosts", join(source, "assets/hosts"));
      }, "without symlinks"],
      ["forbidden", (source) => mkdir(join(source, "assets/node_modules"), { recursive: true }), "cannot contain node_modules"],
      ["entries", async (source) => {
        await mkdir(join(source, "assets/many"), { recursive: true });
        for (let index = 0; index < 260; index++) await writeFile(join(source, "assets/many", `${index}.txt`), "");
      }, "exceeds 256 entries"],
      ["skin", async (source) => {
        const manifest = JSON.parse(await readFile(join(source, "manifest.json"), "utf8"));
        const { width, height } = manifest.presentation;
        await writeFile(join(source, "manifest.json"), JSON.stringify({ ...manifest, presentation: { width, height, skin: "assets/skin.png" } }));
        await mkdir(join(source, "assets"), { recursive: true });
        await writeFile(join(source, "assets/skin.png"), pngHeader(width + 1, height));
      }, "window skin must be exactly"],
    ];
    for (const [name, damage, error] of cases) {
      const source = join(root, name);
      await copySourceFixture(fixture, source);
      await damage(source);
      await expect(buildTemplate(source, undefined, output)).rejects.toThrow(error);
      expect(await readFile(join(output, "assets/app.js"))).toEqual(before);
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
    await copySourceFixture("examples/slops/quick-checklist", source);
    const sdk = join(source, "node_modules/@hitslop/document");
    await mkdir(sdk, { recursive: true });
    await writeFile(join(sdk, "package.json"), JSON.stringify({ name: "@hitslop/document", type: "module", exports: { ".": "./index.js", "./abi": "./abi.js" } }));
    await writeFile(join(sdk, "index.js"), `export * from ${JSON.stringify(resolve("packages/document/src/schema.ts"))};`);
    await writeFile(join(sdk, "abi.js"), `export const RuntimeABI = ${RuntimeABI + 1};`);
    const output = join(root, "out.slop");
    await expect(buildProject(source, output)).rejects.toThrow("Update @hitslop/cli");
    expect(await readdir(root)).toEqual(["source"]);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
}, 60000);
