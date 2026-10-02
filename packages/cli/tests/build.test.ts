import { test, expect } from "bun:test";
import { buildProject } from "../src/build";
import { mkdtemp, cp, readFile, writeFile, rm } from "node:fs/promises";
import { join } from "node:path";
import { readdir, mkdir } from "node:fs/promises";
import { copySourceFixture } from "./source-fixture";
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
    expect(JSON.parse(await readFile(join(built, "manifest.json"), "utf8")).slug).toBeTruthy();
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
