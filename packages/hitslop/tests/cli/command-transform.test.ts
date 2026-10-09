import { test, expect } from "bun:test";
import { CommandBindings } from "../../src/cli/command-bindings";
import { commandSites, stripCommandBodies } from "../../src/cli/command-transform";

async function analyze(source: string, file = "actions.ts") {
  const id = `/project/${file}`;
  const bindings = new CommandBindings(async () => `import {defineDocument} from 'hitslop'; export default defineDocument({});`);
  const members = await bindings.members(source, id, "/project", async spec => spec.startsWith(".") ? "/project/model.ts" : undefined);
  return { source, id, members };
}
async function strip(source: string, file = "actions.ts") {
  const a = await analyze(source, file);
  return stripCommandBodies(a.source, a.id, false, undefined, a.members)?.code;
}
async function sites(source: string) {
  const a = await analyze(source);
  return commandSites(a.source, a.id, a.id, a.members);
}

test("an unrelated package command keeps its implementation even with the same keys", async () => {
  const source = `import {runner} from 'other-library'; runner.command({description:'job',args:{},run(){return 'KEEP_BODY';}});`;
  expect((await strip(source)) ?? source).toContain("KEEP_BODY");
});

test("document command declarations lose only their body", async () => {
  const code = (await strip(`import doc from './model';
export const rename = doc.command({ description: "Rename", args: {}, run({ tx }) {
  // run({ tx }) { } in a comment, "run" in a string, and nested { braces }
  const sentinel = "BODY_ONLY"; return { sentinel };
} });`))!;
  expect(code).not.toContain("BODY_ONLY");
  expect(code).toContain('doc.command({ description: "Rename", args: {} })');
});

test("other .command calls are left alone", async () => {
  for (const source of [
    `program.command("serve").action(() => run());`,
    `cli.command({ name: "x", run() { return "KEEP"; } });`,
    `cli.command({ description: "x", run() { return "KEEP"; } });`,
  ]) expect(await strip(source)).toBeUndefined();
});

test("document aliases work, extracted methods fail, and local shadowing is respected", async () => {
  const prefix = `import doc from './model'; `;
  expect(await strip(prefix + `const alias=doc; alias.command({description:'x',args:{},run(){return 'BODY';}});`)).not.toContain("BODY");
  for (const declaration of ["const declare=doc.command;", "const {command: declare}=doc;"])
    await expect(strip(prefix + declaration)).rejects.toThrow("direct document.command");
  const shadow = prefix + `function use(doc: any){return doc.command({description:'x',args:{},run(){return 'KEEP';}});}`;
  expect((await strip(shadow)) ?? shadow).toContain("KEEP");
});

test("re-exports, local libraries and source changes resolve by binding rather than spelling", async () => {
  const files: Record<string, string> = {
    "/project/model.ts": `import {defineDocument} from 'hitslop'; export const doc=defineDocument({});`,
    "/project/barrel.ts": `export {doc as renamed} from './model';`,
    "/project/library.ts": `export const runner={command:(spec:unknown)=>spec};`,
  };
  const bindings = new CommandBindings(async file => files[file]!);
  const resolve = async (specifier: string) => `/project/${specifier.slice(2)}.ts`;
  const source = `import * as schema from './barrel'; import {runner} from './library';
    schema.renamed.command({description:'x',args:{},run(){return 'REMOVE';}});
    runner.command({description:'x',args:{},run(){return 'KEEP';}});`;
  const transform = async () => stripCommandBodies(source, "/project/actions.ts", false, undefined,
    await bindings.members(source, "/project/actions.ts", "/project", resolve))?.code ?? source;
  const first = await transform();
  expect(first).not.toContain("REMOVE");
  expect(first).toContain("KEEP");
  files["/project/model.ts"] = `export const doc={command:(spec:unknown)=>spec};`;
  bindings.clear();
  expect(await transform()).toContain("REMOVE");
});

test("JSX modules are transformed like TypeScript", async () => {
  const code = (await strip(`import doc from "./model"; export const c = doc.command({ description: "x", args: {}, run: () => "BODY" }); export const v = <b/>;`, "view.tsx"))!;
  expect(code).not.toContain("BODY");
});

test("a command declaration in another shape is an authoring error, not a shipped body", async () => {
  for (const source of [
    `doc.command({ ...shared, args: {}, run() { return "BODY"; } });`,
    `doc.command({ description: "x", args: {}, examples: [], run() { return "BODY"; } });`,
    `doc.command({ args: {}, run() { return "BODY"; } });`,
    `doc.command({ ...spec });`,
    `doc.command({ description: "x", args: {}, ...implementation });`,
    `doc["command"]({ description: "x", args: {}, ["run"]() { return "BODY"; } });`,
  ]) await expect(strip(`import doc from "./schema";\n${source}`)).rejects.toThrow("exactly those fields");
});

test("a spec on one of this project's documents must be an object literal", async () => {
  for (const source of [
    `import doc from "./schema"; const spec = { description: "x", args: {}, run() { return "BODY"; } }; doc.command(spec);`,
    `import { defineDocument } from "hitslop"; const doc = defineDocument({}); doc.command(makeSpec());`,
  ]) await expect(strip(source)).rejects.toThrow("as an object literal");
  // A package's `.command` is not ours, whatever it is given.
  expect(await strip(`import { program } from "commander"; program.command(spec);`)).toBeUndefined();
});

test("an asynchronous body is an authoring error", async () => {
  await expect(strip(`import doc from "./model"; doc.command({ description: "x", args: {}, async run() {} });`)).rejects.toThrow("synchronous");
  await expect(strip(`import doc from "./model"; doc.command({ description: "x", args: {}, run: async () => 1 });`)).rejects.toThrow("synchronous");
});

test("command discovery uses the same syntax as body stripping", async () => {
  const source = `import doc from "./model"; export const rename = doc.command \n ({description: description(), args: {}, run() { return "BODY"; }});`;
  expect((await sites(source)).map(site => site.name)).toEqual(["rename"]);
  expect(await strip(source)).not.toContain("BODY");
});

test("a command site must create one value at module scope", async () => {
  const declaration = `doc.command({description:'Once',args:{},run(){}})`;
  for (const source of [
    `function make(){return ${declaration};}`,
    `for(let i=0;i<2;i++){ const command=${declaration}; }`,
    `class Commands { command=${declaration}; }`,
  ]) await expect(sites(`import doc from "./model"; ${source}`)).rejects.toThrow("module scope");
});
