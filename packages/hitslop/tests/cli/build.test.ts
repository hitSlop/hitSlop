// Building a project: what a built app may contain and how the build's output depends on
// its source. slop.ts is covered in slop-ts.test.ts, portable builds in portable.test.ts.
import { test, expect } from "bun:test";
import { execute } from "../../src/cli/engine";
import { negotiate } from "../../src/cli/engine";
import { exec, run } from "../../../../scripts/lib/test-process";
import { mkdtemp, cp, readFile, writeFile, rm, readdir, mkdir, rename } from "node:fs/promises";
import { join } from "node:path";
import { overrideSlop, stage } from "./source-fixture";
import { buildTemplate } from "../../src/cli/template";

/** What a new document of a built template holds: created and read by the engine. */
async function initialValue(template: string) {
  const document = join(template + ".created", "Document.slop");
  await execute({ method: "create", from: template, output: document });
  try {
    const value = (await execute({ method: "get", documentPath: document })).state.value;
    if (!value || typeof value !== "object" || Array.isArray(value)) throw new Error("Expected an object snapshot");
    return value as Record<string, unknown>;
  } finally {
    await rm(template + ".created", { recursive: true, force: true });
  }
}
// Built apps import nothing from the runtime and reach the host only through ctx.
test("apps contain no runtime code and cannot reach the engine, bridge or remote boot resources", async () => {
  const root = await mkdtemp(join(process.cwd(), ".build-test-"));
  try {
    const source = join(root, "source");
    await cp("tests/apps/document", source, { recursive: true });
    const output = await stage(source, join(root, "built"));
    const js = await readFile(join(output, "resources/ui.js"), "utf8");
    expect(js).not.toContain("/__shell__/");
    expect(await readdir(output)).not.toContain("app.html");
    const app = await readFile(join(source, "App.svelte"), "utf8");
    for (const [code, error] of [
      // Apps never embed a document engine; the host's core owns the document.
      ['import {LoroDoc} from "loro-crdt"; console.log(new LoroDoc());', "cannot import loro-crdt"],
      ['import("/__shell__/index.js").then(console.log);', "cannot import /__shell__/index.js"],
      ["globalThis.webkit.messageHandlers.storage.postMessage({ method: 'ready' });", "host bridge"],
      ['import "./remote.css";', "remote resources"],
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
      ["font", `{ font: '"Avenir Next", sans-serif' }`, "Theme color font"],
      ["opaque", `{ accent: "#aabbccff" }`, "Theme color accent"],
      ["derived", `{ rule: "color-mix(in srgb, var(--slop-ink) 14%, transparent)" }`, "Theme color rule"],
    ] as const) {
      const source = join(root, name);
      await cp("tests/apps/document", source, { recursive: true });
      await overrideSlop(source, { theme });
      await expect(stage(source, join(root, name))).rejects.toThrow(error);
    }
  } finally {
    await rm(root, { recursive: true, force: true });
  }
}, 60000);

test("build paths and fresh source evaluation follow the declaration", async () => {
  const root = await mkdtemp(join(process.cwd(), ".build-test-"));
  try {
    // The explicit slug is independent of paths, which may hold spaces.
    const source = join(root, "path with spaces", "starter");
    await cp("tests/apps/document", source, { recursive: true });
    await overrideSlop(source, { slug: '"starter"' });
    const defaultOutput = await buildTemplate(source, undefined);
    expect(defaultOutput).toBe(join(source, "dist", "starter.slop"));
    const first = await initialValue(defaultOutput);
    await overrideSlop(source, { initial: JSON.stringify({ ...first, title: "Fresh evaluation" }) });
    const explicitOutput = join(root, "output with spaces.slop");
    expect(await buildTemplate(source, undefined, explicitOutput)).toBe(explicitOutput);
    expect((await initialValue(explicitOutput)).title).toBe("Fresh evaluation");
  } finally {
    await rm(root, { recursive: true, force: true });
  }
}, 60000);

test("imported fonts are content addressed, deduplicated, and keep their CSS URLs", async () => {
  const root = await mkdtemp(join(process.cwd(), ".build-test-"));
  try {
    const source = join(root, "source");
    await cp("tests/apps/document", source, { recursive: true });
    await mkdir(join(source, "fonts"));
    await writeFile(join(source, "fonts/My Font.woff2"), new Uint8Array([119,79,70,50,1,2,3,4]));
    await writeFile(join(source, "fonts/unused.txt"), "not imported");
    await writeFile(join(source, "App.svelte"), `<script>import font from './fonts/My Font.woff2'; console.log(font);</script><p>Fonts</p><style>@font-face {font-family:Local;src:url('./fonts/My Font.woff2')}</style>`);
    const output = await stage(source, join(root, "fonts"));
    const files = await readdir(join(output, "resources"), { recursive: true });
    const fonts = files.filter(file => file.endsWith(".woff2"));
    expect(fonts).toHaveLength(1);
    expect(fonts[0]).toMatch(/^media\/[a-f0-9]{64}\.woff2$/);
    expect(files.some(file => file.endsWith("unused.txt"))).toBe(false);
    for (const name of ["ui.js", "ui.css"]) expect(await readFile(join(output,"resources",name),"utf8")).toContain(`/assets/${fonts[0]}`);
  } finally { await rm(root, { recursive: true, force: true }); }
},60000);

// Remote cache keys exclude checkout paths. Identical sources must therefore emit
// identical portable bytes; the existing build tests use only one source location.
test("Svelte styles compile identically in different checkout locations", async () => {
  const root = await mkdtemp(join(process.cwd(), ".build-test-"));
  try {
    const outputs: string[] = [];
    for (const location of ["first-checkout", "second-checkout"]) {
      const source = join(root, location);
      await cp("tests/apps/document", source, { recursive: true });
      await writeFile(
        join(source, "App.svelte"),
        "<p>Portable styles</p><style>p { color: rebeccapurple; }</style>",
      );
      const built = await stage(source, join(root, location + "-stage"));
      outputs.push(await readFile(join(built, "resources/ui.js"), "utf8"));
    }
    expect(outputs[0]).toBe(outputs[1]);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
}, 60000);

for (const [name, properties, initial, code] of [
  ["unsafe string bound", '{value:s.string({maxLength:9_007_199_254_740_992})}', { value: "" }, "invalid_schema"],
  ["optional handle name", '{value:s.optional(s.object({set:s.string()}))}', {}, "invalid_schema"],
  ["invalid row id", '{rows:s.list(s.object({value:s.string()}))}', { rows: [{ $id: "bad id", value: "" }] }, "invalid_id"],
  ["duplicate row id", '{rows:s.list(s.object({value:s.string()}))}', { rows: [{ $id: "same", value: "a" }, { $id: "same", value: "b" }] }, "duplicate_id"],
] as const) test(`build enforces core validation: ${name}`, async () => {
  const root = await mkdtemp(join(process.cwd(), ".core-validation-test-"));
  try {
    const source = join(root, "source");
    await cp("tests/apps/document", source, { recursive: true });
    await writeFile(join(source, "schema.ts"), `import {defineDocument,s} from 'hitslop'; export default defineDocument(${properties});`);
    await writeFile(join(source, "commands.ts"), "export {};\n");
    await overrideSlop(source, { initial: JSON.stringify(initial), commands: "{}" });
    await expect(stage(source, join(root, "invalid"))).rejects.toThrow(code);
  } finally { await rm(root, { recursive: true, force: true }); }
});

test("only explicit roles are built; unrelated filenames are ignored", async () => {
  const root = await mkdtemp(join(process.cwd(), ".build-test-"));
  try {
    const source = join(root,"source");
    await cp("tests/apps/document",source,{recursive:true});
    // Keep the declared view distinct on case-insensitive filesystems too.
    await rename(join(source, "Export.svelte"), join(source, "Capture.svelte"));
    const declaration = join(source, "slop.ts");
    await writeFile(declaration, (await readFile(declaration, "utf8")).replace("./Export.svelte", "./Capture.svelte"));
    await writeFile(join(source,"main.ts"), "this is not valid typescript");
    await writeFile(join(source,"export.svelte"), "<script>const = ;</script>");
    await stage(source,join(root,"built"));
    // The imported role, however it is named, is compiled.
    await writeFile(join(source,"Icon.svelte"), "<script>const = ;</script>");
    await expect(stage(source,join(root,"invalid"))).rejects.toThrow();
  } finally { await rm(root,{recursive:true,force:true}); }
},60000);
test("a capture view cannot read state the editor sets", async () => {
  const root = await mkdtemp(join(process.cwd(), ".build-test-"));
  try {
    const source = join(root,"source");
    await cp("tests/apps/document",source,{recursive:true});
    await writeFile(join(source,"ui.svelte.ts"), `export const ui = $state({ tab: "tasks" });\n`);
    await writeFile(join(source,"Tab.svelte"), `<script lang="ts">import { ui } from "./ui.svelte";</script><b>{ui.tab}</b>`);
    await writeFile(join(source,"Export.svelte"), `<script lang="ts">import Tab from "./Tab.svelte";</script><Tab />`);
    // Reusing the editor does not make its transient state available to captures.
    await overrideSlop(source, { view: "Tab", export: "Tab" }, `import Tab from "./Tab.svelte";`);
    await expect(stage(source,join(root,"editor"))).rejects.toThrow("fresh page");
    // A capture view sees only the initial value, even through a shared component.
    await overrideSlop(source, { export: "Export" }, `import Export from "./Export.svelte";`);
    await expect(stage(source,join(root,"export"))).rejects.toThrow("fresh page");
  } finally { await rm(root,{recursive:true,force:true}); }
},60000);
