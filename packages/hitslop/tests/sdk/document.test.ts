// Gap: a definition is data; an editor component acquires its own mount's document through
// Svelte context; commands keep their supplied context. Oracle: probe components rendered
// on the server (Svelte context behaves alike there; client mounting is in
// document.browser.test.ts), and the command runner's intents.
import { afterAll, expect, test } from "bun:test";
import { compile } from "svelte/compiler";
import { render } from "svelte/server";
import { mkdtemp, rm, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { defineDocument, s, type LiveDocument, type ObjectNode } from "../../src/sdk/schema";
import { defineCommands } from "../../src/sdk/commands";
import { commandSet } from "../../src/sdk/command-set";
import { documentContext, getDocument } from "../../src/sdk/app/document";
import { evaluate } from "../../src/shell/commands";

const folder = await mkdtemp(join(import.meta.dir, ".build-test-document-"));
afterAll(() => rm(folder, { recursive: true, force: true }));
/** A server-compiled component module. */
async function component(name: string, source: string) {
  const path = join(folder, `${name}.js`);
  await writeFile(path, compile(source, { filename: `${name}.svelte`, generate: "server" }).js.code);
  return await import(path);
}
const sdk = JSON.stringify(new URL("../../src/sdk/app/document.ts", import.meta.url).pathname);
// Records what it acquired; a nested component shows the title.
const Title = await component("Title", `<script module>export const seen = [];</script>
<script>import { getDocument } from ${sdk}; let { schema } = $props(); const doc = getDocument(schema); seen.push(doc);</script>
<h1>{doc.current.title}</h1>`);
const Page = await component("Page", `<script>import Title from "./Title.js"; let { schema } = $props();</script>
<main><Title {schema} /></main>`);

const schema = defineDocument(
  { title: s.string(), rows: s.list(s.object({ text: s.text() })) },
  { initial: { title: "Initial" } },
);
const live = (title: string) =>
  ({ current: { title, rows: [] } }) as unknown as LiveDocument<ObjectNode>;

test("a definition is its frozen descriptor, with the starting value hidden from the page", () => {
  expect(Object.keys(schema)).toEqual(["descriptor"]);
  expect(JSON.parse(JSON.stringify(schema))).toEqual({ descriptor: schema.descriptor });
  expect(Object.isFrozen(schema) && Object.isFrozen(schema.descriptor.properties.rows.item)).toBe(true);
  expect((schema as unknown as { initial: unknown }).initial).toEqual({ title: "Initial" });
  for (const member of ["current", "fields", "at", "change", "flush", "undo", "redo"])
    expect(member in schema).toBe(false);
});

test("components acquire the document of the mount they render in", () => {
  const first = live("First"), second = live("Second");
  const page = (doc: LiveDocument<ObjectNode>) =>
    render(Page.default, { props: { schema }, context: documentContext(schema, doc) }).body;
  expect(page(first)).toContain("<h1>First</h1>");
  expect(page(second)).toContain("<h1>Second</h1>");
  // The adapter itself, not a copy, and each mount its own.
  expect(Title.seen.slice(-2)).toEqual([first, second]);
  expect(Title.seen.at(-2)).toBe(first);
  expect(Title.seen.at(-1)).toBe(second);
});

test("getDocument refuses another definition, a component outside an app and calls outside initialization", () => {
  const other = defineDocument({ title: s.string() }, { initial: { title: "" } });
  const mounted = documentContext(schema, live("Mounted"));
  expect(() => render(Title.default, { props: { schema: other }, context: mounted }).body).toThrow(
    "takes the schema.ts definition the app mounted with",
  );
  expect(() => render(Title.default, { props: { schema } }).body).toThrow(
    "needs a component of a mounted hitSlop app",
  );
  expect(() => getDocument(schema)).toThrow("must run while a component initializes");
});

test("commands over a pure definition collect edits through their own ctx", () => {
  const set = commandSet(
    defineCommands(schema, {
      rename: {
        description: "Rename",
        args: { title: s.string() },
        run: ({ tx, current }, { title }) => {
          tx.fields.title.set(`${current.title} ${title}`);
          tx.fields.rows.insert({ text: title });
          return current.rows.length;
        },
      },
    }),
  )!;
  expect(set.document).toBe(schema);
  const { intents, result } = evaluate(
    { value: { title: "Old", rows: [] }, descriptor: schema.descriptor, now: 0, seed: [1, 2, 3, 4], args: { title: "new" } },
    set.specs.rename!.run,
  );
  expect(intents).toEqual([
    { type: "set", path: ["title"], value: "Old new" },
    { type: "insert", path: ["rows"], id: expect.any(String), value: { text: "new" } },
  ]);
  expect(result).toBe(0);
});
