// Building a project: what a built app may contain and how the build's output depends on
// its source. slop.ts is covered in slop-ts.test.ts, portable builds in portable.test.ts.
import { test, expect } from "bun:test";
import { findEngine } from "../../src/cli/engine";
import { negotiate } from "../../src/cli/native";
import { exec, run } from "../../src/cli/process";
import { mkdtemp, cp, readFile, writeFile, rm, readdir, mkdir } from "node:fs/promises";
import { join } from "node:path";
import { overrideSlop, stage } from "./source-fixture";
import { buildTemplate } from "../../src/cli/template";

/** What a new document of a built template holds: created and read by the engine. */
async function initialValue(template: string) {
  const document = join(template + ".created", "Document.slop");
  const named = negotiate(await findEngine());
  await run([...named, "create", "--from", template, "--output", document]);
  try {
    const { stdout } = await exec([...named, "request"], { stdin: JSON.stringify({ method: "get", documentPath: document }) });
    return JSON.parse(stdout).state.value;
  } finally {
    await rm(template + ".created", { recursive: true, force: true });
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

test("build paths and fresh source evaluation do not depend on authored stdout", async () => {
  const root = await mkdtemp(join(process.cwd(), ".build-test-"));
  try {
    // The folder's name is the slug; the path above it may hold spaces.
    const source = join(root, "path with spaces", "starter");
    await cp("packages/hitslop/templates/checklist", source, { recursive: true });
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
      await cp("packages/hitslop/templates/checklist", source, { recursive: true });
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
    await writeFile(join(source, "schema.ts"), `import {defineDocument,s} from 'hitslop'; export default defineDocument(${properties});`);
    await overrideSlop(source, { initial: JSON.stringify(initial) });
    await expect(stage(source, join(root, "invalid"))).rejects.toThrow(code);
  } finally { await rm(root, { recursive: true, force: true }); }
});

test("only generated Svelte entries build and capture discovery is exact-case", async () => {
  const root = await mkdtemp(join(process.cwd(), ".build-test-"));
  try {
    const source = join(root, "source");
    await cp("packages/hitslop/templates/checklist", source, { recursive: true });
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
