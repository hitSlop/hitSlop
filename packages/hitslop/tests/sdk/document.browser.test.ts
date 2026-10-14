// Gap: getDocument through a real client mount. Each svelteApp mount gives its components
// one reactive document through Svelte context, independent of other mounts; a remount gets
// a fresh one. Oracle: WebKit's DOM over a minimal ctx whose document publishes snapshots and
// reports handle reads as the shell's does (abi.ts SlopDocument).
import { afterAll, beforeAll, expect, test } from "bun:test";
import { webkit, type Browser, type Page } from "playwright";
import { build } from "vite";
import { mkdtemp, rm, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { sveltePlugin } from "../../src/cli/app-vite";

const files = {
  "schema.ts": `import { defineDocument, s } from "hitslop";
export default defineDocument({ label: s.string() }, { initial: { label: "" } });
export const other = defineDocument({ label: s.string() }, { initial: { label: "" } });`,
  "Label.svelte": `<script lang="ts">
  import { getDocument } from "hitslop/svelte";
  import schema from "./schema";
  const doc = getDocument(schema);
  (window as any).acquired.push(doc);
</script>
<h1>{doc.current.label}</h1>`,
  "App.svelte": `<script lang="ts">
  import { getDocument } from "hitslop/svelte";
  import schema from "./schema";
  import Label from "./Label.svelte";
  const doc = getDocument(schema);
  (window as any).acquired.push(doc);
  let late = $state("");
  function acquireLate() {
    try { getDocument(schema); } catch (error) { late = (error as Error).message; }
  }
</script>
<Label />
<input aria-label="Label" bind:value={doc.fields.label.value} />
<button onclick={acquireLate}>Late</button>
<output>{late}</output>`,
  "Other.svelte": `<script lang="ts">
  import { getDocument } from "hitslop/svelte";
  import { other } from "./schema";
  getDocument(other);
</script>`,
  // A ctx whose document publishes like the shell's: an accepted write replaces the frozen
  // snapshot and notifies subscribers; reading a handle's value reports to the observer.
  "entry.ts": `import { svelteApp } from "hitslop/svelte";
import schema from "./schema";
import App from "./App.svelte";
import Other from "./Other.svelte";
const win = window as any;
win.acquired = [];
win.apps = { App: svelteApp(App, { schema }), Other: svelteApp(Other, { schema }) };
win.context = (label: string) => {
  let current = Object.freeze({ label });
  let observer = () => {};
  const listeners = new Set<() => void>();
  const errors: string[] = [];
  const publish = (next: string) => {
    current = Object.freeze({ label: next });
    for (const listener of [...listeners]) listener();
  };
  const document = {
    get current() { return current; },
    fields: { label: {
      get value() { observer(); return current.label; },
      set value(next: string) { queueMicrotask(() => publish(next)); },
    } },
    subscribe(listener: () => void) { listeners.add(listener); return () => listeners.delete(listener); },
    observe(read: () => void) { observer = read; return () => { if (observer === read) observer = () => {}; }; },
  };
  return { ctx: { document, reportError: (error: Error) => errors.push(error.message) }, publish, listeners, errors };
};`,
};

let folder: string;
let browser: Browser;
let page: Page;
beforeAll(async () => {
  folder = await mkdtemp(join(import.meta.dir, ".build-test-mount-"));
  for (const [name, source] of Object.entries(files)) await writeFile(join(folder, name), source);
  const built = await build({
    root: folder,
    configFile: false,
    envFile: false,
    publicDir: false,
    logLevel: "error",
    plugins: [sveltePlugin()],
    resolve: { dedupe: ["svelte"] },
    build: {
      write: false,
      target: "safari17",
      rolldownOptions: { input: join(folder, "entry.ts"), output: { format: "es", codeSplitting: false } },
    },
  });
  const output = (Array.isArray(built) ? built : [built]).flatMap((result) => ("output" in result ? result.output : []));
  const script = output.find((item) => item.type === "chunk")!.code;
  browser = await webkit.launch();
  page = await browser.newPage();
  await page.setContent(`<!doctype html><main id="a"></main><main id="b"></main><main id="c"></main>`);
  await page.addScriptTag({ type: "module", content: script });
  await page.waitForFunction(() => !!(window as any).apps);
}, 60000);
afterAll(async () => {
  await browser?.close();
  await rm(folder, { recursive: true, force: true });
});

test("each mount's components share one reactive document, independent of other mounts", async () => {
  const result = await page.evaluate(async () => {
    const win = window as any;
    const tick = () => new Promise((resolve) => setTimeout(resolve, 0));
    const text = (id: string) => document.querySelector(`#${id} h1`)!.textContent;
    const input = (id: string) => document.querySelector<HTMLInputElement>(`#${id} input`)!;
    const first = win.context("First"), second = win.context("Second");
    const a = await win.apps.App.mount(first.ctx, document.getElementById("a"));
    const b = await win.apps.App.mount(second.ctx, document.getElementById("b"));
    await a.rendered(); await b.rendered();
    const [appA, labelA, appB, labelB] = win.acquired;
    const mounted = [text("a"), text("b"), input("a").value];
    first.publish("Published");
    await tick();
    const published = [text("a"), input("a").value, text("b")];
    input("b").value = "Typed";
    input("b").dispatchEvent(new Event("input", { bubbles: true }));
    await tick(); await tick();
    const typed = [text("b"), text("a")];
    return {
      mounted, published, typed,
      shared: appA === labelA && appB === labelB, independent: appA !== appB,
    };
  });
  expect(result).toEqual({
    mounted: ["First", "Second", "First"],
    published: ["Published", "Published", "Second"],
    typed: ["Typed", "Published"],
    shared: true,
    independent: true,
  });
});

test("a remount acquires a fresh document and the previous one stops listening", async () => {
  const result = await page.evaluate(async () => {
    const win = window as any;
    const target = document.getElementById("c")!;
    const third = win.context("Third");
    const before = win.acquired.length;
    const view = await win.apps.App.mount(third.ctx, target);
    await view.rendered();
    const listening = third.listeners.size;
    await view.unmount();
    const afterUnmount = third.listeners.size;
    third.publish("Reloaded");
    const again = await win.apps.App.mount(third.ctx, target);
    await again.rendered();
    const [first, , second] = win.acquired.slice(before);
    const shown = target.querySelector("h1")!.textContent;
    await again.unmount();
    return { listening, afterUnmount, fresh: first !== second, shown };
  });
  expect(result).toEqual({ listening: 1, afterUnmount: 0, fresh: true, shown: "Reloaded" });
});

test("getDocument refuses calls after initialization and a definition the app did not mount", async () => {
  const result = await page.evaluate(async () => {
    const win = window as any;
    const target = document.getElementById("c")!;
    const view = await win.apps.App.mount(win.context("Late").ctx, target);
    await view.rendered();
    target.querySelector("button")!.click();
    await view.rendered();
    const late = target.querySelector("output")!.textContent;
    await view.unmount();
    const other = win.context("Other");
    const refused = await win.apps.Other.mount(other.ctx, target);
    await refused.rendered();
    const shown = target.querySelector("[role=alert]")?.textContent;
    await refused.unmount();
    return { late, errors: other.errors, shown };
  });
  expect(result.late).toContain("getDocument(schema) must run while a component initializes");
  expect(result.errors).toEqual([expect.stringContaining("takes the schema.ts definition the app mounted with")]);
  expect(result.shown).toContain("Could not render this document");
});
