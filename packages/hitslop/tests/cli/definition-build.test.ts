import { createHash } from "node:crypto";
import { extname } from "node:path";
import { test, expect } from "bun:test";
import { mkdir, mkdtemp, readFile, rename, rm, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { buildDefinition } from "../../src/cli/definition-build";
import { definitionFixture } from "./definition-fixture";
const resourceKey = (bytes: Uint8Array, name: string) => `media/${createHash("sha256").update(bytes).digest("hex")}${extname(name)}`;

test("separate Vite graphs retain dependencies without leaking metadata or command bodies", async () => {
  const root = await mkdtemp(join(process.cwd(),".build-test-definition-"));
  try {
    const source=join(root,"source"), stage=join(root,"stage");
    const options = await definitionFixture(source);
    const built = await buildDefinition(source,stage,options);
    const {input} = built;
    expect(input.declaration.metadata.slug).toBe("fixture");
    expect(input.declaration.theme.map(t=>t.token)).toEqual(["z","a"]);
    expect(input.declaration.commands.map(c=>c.name)).toEqual(["rename"]);
    expect(input.declaration.views).toEqual({export:true,icon:false});
    expect(input.roles.style).toBe("ui.css");
    const keys=input.resources.map(r=>r.key);
    for (const name of ["skin.png","font.woff2","lib/paper.svg","export.svg"])
      expect(keys).toContain(resourceKey(await readFile(join(source,name)),name));
    expect(keys).not.toContain(resourceKey(await readFile(join(source,"preview.png")),"preview.png"));
    expect(input.artwork.preview).toBe("artwork/preview.png");
    expect(keys).not.toContain("unused.txt");
    const ui=await readFile(join(stage,"resources/ui.js"),"utf8");
    const commands=await readFile(join(stage,"resources/commands.js"),"utf8");
    expect(ui).not.toContain("BODY_ONLY_CHANGE");
    expect(ui).not.toContain("METADATA_ONLY_SENTINEL");
    expect(input.declaration.initial).toEqual({title:"Original"});
    expect(commands).toContain("BODY_ONLY_CHANGE");
    expect(ui).not.toContain(source);
    expect(commands).not.toContain(source);
    expect(Object.keys(input.declaration.commands[0]!)).toEqual(["name", "description", "args"]);
    expect(built.dependencies.ui).toContain(join(source,"App.svelte"));
    expect(built.dependencies.ui).toContain(join(source,"font.woff2"));
    expect(built.dependencies.definition).toContain(join(source,"schema.ts"));
    expect(built.dependencies.definition).not.toContain(join(source,"export.svg"));
    const again=await buildDefinition(source,join(root,"again"),options);
    expect(again.input).toEqual(input);
    const entry = await readFile(join(source,"slop.ts"),"utf8");
    await writeFile(join(source,"slop.ts"),entry
      .replace("import skin from './skin.png';", "const skin = new URL('./skin.png',import.meta.url).href;")
      .replace("import preview from './preview.png';", "const images = import.meta.glob('./preview.png',{eager:true,query:'?url',import:'default'}); const preview=images['./preview.png'];"));
    const urlBuild = await buildDefinition(source,join(root,"urls"),options);
    expect(urlBuild.input.roles.skin).toBe(input.roles.skin);
    expect(urlBuild.dependencies.definition).toContain(join(source,"skin.png"));
    expect(urlBuild.input.artwork).toEqual(input.artwork);
    const css = await readFile(join(source,"theme.css"),"utf8");
    await writeFile(join(source,"theme.css"),css+'\ndiv{background-image:url("./preview.png")}');
    const reused = await buildDefinition(source,join(root,"reused"),options);
    expect(reused.input.resources.map(r=>r.key)).toContain(resourceKey(await readFile(join(source,"preview.png")),"preview.png"));
    // Artifact sizes are evidence, not a golden assertion on compiler output.
    console.log(`explicit-entry UI: ${built.uiBytes} bytes; ${input.resources.length} resources`);
  } finally { await rm(root,{recursive:true,force:true}); }
},60000);

test("a declaration without commands is evaluated but stores no command program", async () => {
  const root = await mkdtemp(join(process.cwd(),".build-test-definition-"));
  try {
    const source=join(root,"source"),stage=join(root,"stage");
    const options=await definitionFixture(source);
    const entry=await readFile(join(source,"slop.ts"),"utf8");
    await rm(join(source,"commands.ts"));
    await rm(join(source,"Export.svelte"));
    await writeFile(join(source,"App.svelte"),"<h1>No commands</h1>");
    const {input}=await buildDefinition(source,stage,options);
    expect(input.declaration.commands).toEqual([]);
    expect(input.declaration.views).toEqual({export:false,icon:false});
    expect(input.roles.commands).toBeUndefined();
    expect(input.resources.some(r=>r.key==="commands.js")).toBe(false);
    expect(await Bun.file(join(stage,"resources/commands.js")).exists()).toBe(false);
    await writeFile(join(source,"slop.ts"),"globalThis.clock=Date.now();"+entry);
    await expect(buildDefinition(source,stage,options)).rejects.toThrow("host-provided");
  } finally { await rm(root,{recursive:true,force:true}); }
},60000);

test("role names and graph boundaries reject misplaced code", async () => {
  const root = await mkdtemp(join(process.cwd(), ".build-test-definition-"));
  try {
    const source = join(root, "source"), stage = join(root, "stage");
    const options = await definitionFixture(source);
    const app = await readFile(join(source,"App.svelte"),"utf8");
    for (const [module, message] of [["./slop", "slop.ts is metadata"], ["./commands", "commands.ts is agent-only"], ["hitslop/commands", "Only root commands.ts"]]) {
      await writeFile(join(source,"App.svelte"), `<script>import * as forbidden from '${module}'; console.log(forbidden);</script>`);
      await expect(buildDefinition(source, stage, options)).rejects.toThrow(message);
    }
    await writeFile(join(source,"App.svelte"),app);
    for (const name of ["app.svelte", "export.svelte", "Schema.ts", "command.ts"]) {
      // On case-insensitive volumes, do not overwrite the real role.
      const actual = name === "app.svelte" ? "App.svelte" : name === "export.svelte" ? "Export.svelte" : name === "Schema.ts" ? "schema.ts" : "commands.ts";
      await rename(join(source,actual), join(source,"role.tmp"));
      await rename(join(source,"role.tmp"), join(source,name));
      await expect(buildDefinition(source, stage, options)).rejects.toThrow("did you mean");
      await rename(join(source,name), join(source,"role.tmp"));
      await rename(join(source,"role.tmp"), join(source,actual));
    }
    const commands = await readFile(join(source,"commands.ts"),"utf8");
    await writeFile(join(source,"commands.ts"), "export default undefined;");
    await expect(buildDefinition(source,stage,options)).rejects.toThrow("must default-export defineCommands");
    // The page cannot import commands.ts, so its named helpers stay definition-only.
    await writeFile(join(source,"commands.ts"), commands + "export const extra = 7;");
    await buildDefinition(source,stage,options);
    const schema = await readFile(join(source,"schema.ts"),"utf8");
    await writeFile(join(source,"schema.ts"), schema + "\nexport const label = (title: string) => title.trim();\n");
    await buildDefinition(source,stage,options);
    await writeFile(join(source,"schema.ts"), schema);
    await writeFile(join(source,"commands.ts"), commands.replace("import {defineCommands} from 'hitslop/commands'", "import {defineCommands} from './helper'"));
    await writeFile(join(source,"helper.ts"), "export {defineCommands} from 'hitslop/commands';");
    await expect(buildDefinition(source,stage,options)).rejects.toThrow("Only root commands.ts");
  } finally { await rm(root, {recursive:true,force:true}); }
}, 60000);

test("a nonempty public directory is refused instead of silently ignored", async () => {
  const root = await mkdtemp(join(process.cwd(),".build-test-definition-"));
  try {
    const source=join(root,"source"),stage=join(root,"stage");
    const options=await definitionFixture(source);
    await mkdir(join(source,"public"));
    await writeFile(join(source,"public","lost.txt"),"must be imported");
    await expect(buildDefinition(source,stage,options)).rejects.toThrow("Import assets explicitly");
    await rm(join(source,"public","lost.txt"));
    expect((await buildDefinition(source,stage,options)).input.roles.ui).toBe("ui.js");
  } finally { await rm(root,{recursive:true,force:true}); }
},60000);

test("headless declarations fail at the actual import or initialization boundary", async () => {
  const root=await mkdtemp(join(process.cwd(),".build-test-definition-"));
  try {
    const source=join(root,"source"),stage=join(root,"stage");
    const options=await definitionFixture(source);
    const actions=await readFile(join(source,"commands.ts"),"utf8");
    for (const [prefix,error] of [
      ["import {onMount} from 'svelte'; onMount(()=>{});", "Definition modules cannot import svelte"],
      ["import './theme.css';", "App.svelte"],
      ["import App from './App.svelte'; console.log(App);", "App.svelte"],
      ["document.body.innerHTML='bad';", "Definition initialization failed"],
      ["globalThis.initializationClock=Date.now();", "host-provided"],
      ["for(;;) {}", "interrupted"],
    ]) {
      await writeFile(join(source,"commands.ts"),prefix+actions);
      await expect(buildDefinition(source,stage,options)).rejects.toThrow(error);
    }
    await writeFile(join(source,"commands.ts"),actions);
    for (const run of ["async run({tx},args)", "*run({tx},args)", "async *run({tx},args)"]) {
      await writeFile(join(source,"commands.ts"),actions.replace("run({tx},args)", run));
      await expect(buildDefinition(source,stage,options)).rejects.toThrow("must be synchronous");
    }
    await writeFile(join(source,"commands.ts"),actions.replace("defineCommands(doc,", "defineCommands({descriptor:doc.descriptor},"));
    await expect(buildDefinition(source,stage,options)).rejects.toThrow("must pass the schema.ts document");
    await writeFile(join(source,"commands.ts"),actions);
    const entry=await readFile(join(source,"slop.ts"),"utf8");
    await writeFile(join(source,"slop.ts"),entry.replace('image:skin','image:"/assets/missing.png"'));
    await expect(buildDefinition(source,stage,options)).rejects.toThrow("not emitted");
    await writeFile(join(source,"slop.ts"),entry);
    const css = await readFile(join(source,"theme.css"),"utf8");
    await writeFile(join(source,"theme.css"),css+'\nspan{background:url("/assets/not-emitted.svg")}');
    await expect(buildDefinition(source,stage,options)).rejects.toThrow("references an asset no build emitted");
    await writeFile(join(source,"theme.css"),css);
    await rm(join(source,"skin.png"));
    await expect(buildDefinition(source,stage,options)).rejects.toThrow("skin.png");
  } finally { await rm(root,{recursive:true,force:true}); }
},60000);

test("an author folder named src/shell is ordinary app code", async () => {
  const root=await mkdtemp(join(process.cwd(),".build-test-definition-"));
  try {
    const source=join(root,"source"),stage=join(root,"stage");
    const options=await definitionFixture(source);
    await mkdir(join(source,"src","shell"),{recursive:true});
    await writeFile(join(source,"src","shell","Header.svelte"),"<header>Shell of a crab</header>");
    const room=await readFile(join(source,"App.svelte"),"utf8");
    await writeFile(join(source,"App.svelte"),room.replace("<script lang=\"ts\">","<script lang=\"ts\">import Header from './src/shell/Header.svelte';")+"<Header/>");
    const {input}=await buildDefinition(source,stage,options);
    expect(await readFile(join(stage,"resources/ui.js"),"utf8")).toContain("Shell of a crab");
    expect(input.roles.ui).toBe("ui.js");
  } finally { await rm(root,{recursive:true,force:true}); }
},60000);
