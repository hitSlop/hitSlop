import { test, expect } from "bun:test";
import { mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { buildDefinition, resourceKey } from "../../src/cli/definition-build";
import { definitionFixture } from "./definition-fixture";

test("two Vite builds retain real dependencies and remove UI command bodies", async () => {
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
    expect(commands).toContain("BODY_ONLY_CHANGE");
    expect(ui).not.toContain(source);
    expect(commands).not.toContain(source);
    expect(built.dependencies.ui).toContain(join(source,"Room.svelte"));
    expect(built.dependencies.ui).toContain(join(source,"font.woff2"));
    expect(built.dependencies.definition).toContain(join(source,"model.ts"));
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
    const entry=(await readFile(join(source,"slop.ts"),"utf8"))
      .replace("import {rename} from './actions';", "")
      .replace("commands:{rename},", "");
    await writeFile(join(source,"slop.ts"),entry);
    await writeFile(join(source,"Room.svelte"),"<h1>No commands</h1>");
    const {input}=await buildDefinition(source,stage,options);
    expect(input.declaration.commands).toEqual([]);
    expect(input.roles.commands).toBeUndefined();
    expect(input.resources.some(r=>r.key==="commands.js")).toBe(false);
    expect(await Bun.file(join(stage,"resources/commands.js")).exists()).toBe(false);
    await writeFile(join(source,"slop.ts"),"globalThis.clock=Date.now();"+entry);
    await expect(buildDefinition(source,stage,options)).rejects.toThrow("host-provided");
  } finally { await rm(root,{recursive:true,force:true}); }
},60000);

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
    const actions=await readFile(join(source,"actions.ts"),"utf8");
    for (const [prefix,error] of [
      ["import {onMount} from 'svelte'; onMount(()=>{});", "Headless declaration cannot import svelte"],
      ["import app from './slop'; console.log(app);", "Only the generated entry"],
      ["document.body.innerHTML='bad';", "Definition initialization failed"],
      ["globalThis.initializationClock=Date.now();", "host-provided"],
      ["for(;;) {}", "interrupted"],
    ]) {
      await writeFile(join(source,"actions.ts"),prefix+actions);
      await expect(buildDefinition(source,stage,options)).rejects.toThrow(error);
    }
    await writeFile(join(source,"actions.ts"),actions);
    await writeFile(join(source,"actions.ts"),actions.replace("run({tx},args)","async run({tx},args)"));
    await expect(buildDefinition(source,stage,options)).rejects.toThrow("run must be synchronous");
    await writeFile(join(source,"actions.ts"),actions);
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
