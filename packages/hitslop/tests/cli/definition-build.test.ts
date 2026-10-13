import { createHash } from "node:crypto";
import { extname } from "node:path";
import { test, expect } from "bun:test";
import { mkdir, mkdtemp, readFile, rename, rm, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { buildDefinition } from "../../src/cli/definition-build";
import { definitionFixture } from "./definition-fixture";
import { runInNewContext } from "node:vm";
import { buildGraphs } from "../../src/cli/definition-config";

test("capture roles require icon and preview while export is independent and optional", async () => {
  const root = await mkdtemp(join(process.cwd(), ".build-test-capture-roles-"));
  try {
    const options = await definitionFixture(root);
    await writeFile(join(root, "Icon.svelte"), "<p>Icon only</p>");
    await writeFile(join(root, "Preview.svelte"), "<p>Preview only</p>");
    await rm(join(root, "Preview.svelte"));
    await expect(buildGraphs(root, options)).rejects.toThrow("Missing required Preview.svelte");
    await writeFile(join(root, "Preview.svelte"), "<p>Preview only</p>");
    await rm(join(root, "Icon.svelte"));
    await expect(buildGraphs(root, options)).rejects.toThrow("Missing required Icon.svelte");
    await writeFile(join(root, "Icon.svelte"), "<p>Icon only</p>");
    await rm(join(root, "Export.svelte"));
    const { outputs } = await buildGraphs(root, options);
    expect(outputs.definition.has("export.js")).toBe(false);
    for (const kind of ["icon", "preview"] as const) {
      const context: Record<string, any> = {};
      runInNewContext(new TextDecoder().decode(outputs.definition.get(`${kind}.js`)!.bytes), context);
      const result = JSON.parse(context.__slopCapture(JSON.stringify({kind, width:512, height:512, snapshot:{}, theme:{}})));
      expect(result.frames[0].html).toContain(kind === "icon" ? "Icon only" : "Preview only");
      expect(result.frames[0].html).toContain(`data-slop-capture-target="${kind}"`);
    }
  } finally { await rm(root, {recursive:true, force:true}); }
}, 60000);

test("private widget SSR reads saved state, packages font aliases, and refuses writes and async views", async () => {
  const root = await mkdtemp(join(process.cwd(), ".build-test-widget-"));
  try {
    const source = join(root, "source");
    const options = await definitionFixture(source);
    const view = '<script>import doc from "./schema"; let {family}=$props();</script><h1>{doc.current.title} {family}</h1><svg width="16" height="16"><rect width="16" height="16" fill="var(--slop-accent)"/><text>{doc.current.title}</text></svg>';
    await writeFile(join(source, "Widget.svelte"), view);
    const bundle = async () => new TextDecoder().decode((await buildGraphs(source, options)).outputs.definition.get("widget.js")!.bytes);
    const script = await bundle();
    const render = (script: string, title: string) => {
      const context: Record<string, any> = {};
      runInNewContext(script, context);
      return JSON.parse(context.__slopWidget(JSON.stringify({snapshot: {title}, theme: {accent: "#123456"}, viewports: [{family: "small", width: 164, height: 164}]})));
    };
    const result = render(script, "Saved 😀");
    expect(result.frames[0].html).toContain("Saved 😀 small");
    expect(result.frames[0].html).toContain('fill="#123456"');
    expect(render(script, "var(--slop-accent)").frames[0].html).toContain("var(--slop-accent)</text>");
    expect(result.frames[0].css).toContain("--slop-accent:#123456");
    expect(result.fonts[0].family).toBe("Fixture");
    expect(Buffer.from(result.fonts[0].bytes, "base64")).toEqual(await readFile(join(source, "font.woff2")));
    expect(render(script, "Another document").frames[0].html).not.toContain("Saved");
    await writeFile(join(source, "Icon.svelte"), '<script>import doc from "./schema";</script><svg width="512" height="512"><rect width="512" height="512" fill="var(--slop-accent)"/><text>{doc.current.title}</text></svg>');
    const iconScript = new TextDecoder().decode((await buildGraphs(source, options)).outputs.definition.get("icon.js")!.bytes);
    const iconContext: Record<string, any> = {};
    runInNewContext(iconScript, iconContext);
    const icon = JSON.parse(iconContext.__slopCapture(JSON.stringify({kind:"icon", width:512, height:512, snapshot: {title:"Saved icon"}, theme:{accent:"#123456"}})));
    expect(icon.frames).toHaveLength(1);
    expect(icon.frames[0].html).toContain("Saved icon");
    expect(icon.frames[0].html).toContain('<html data-slop-renderer="true" data-slop-capture="icon">');
    expect(icon.frames[0].html).toContain('fill="#123456"');
    const literal = JSON.parse(iconContext.__slopCapture(JSON.stringify({kind:"icon",width:512,height:512,snapshot:{title:"var(--slop-accent)"},theme:{accent:"#123456"}})));
    expect(literal.frames[0].html).toContain("var(--slop-accent)</text>");
    await writeFile(join(source, "Export.svelte"), '<script>import doc from "./schema";</script><p>document {doc.current.title}</p>');
    expect((await buildGraphs(source, options)).outputs.definition.has("export.js")).toBe(true);
    await writeFile(join(source, "Export.svelte"), '<script>const title = document.title;</script><p>{title}</p>');
    await expect(buildGraphs(source, options)).rejects.toThrow("uses document");
    await writeFile(join(source, "BrowserPart.svelte"), '<script>import {onMount} from "svelte"; onMount(()=>{});</script><p>Browser scene</p>');
    await writeFile(join(source, "Export.svelte"), '<script>import BrowserPart from "./BrowserPart.svelte";</script><BrowserPart/>');
    await expect(buildGraphs(source, options)).rejects.toThrow("BrowserPart.svelte: onMount");
    const localeDependency = join(source, "node_modules", "capture-locale");
    await mkdir(localeDependency, {recursive:true});
    await writeFile(join(localeDependency, "package.json"), JSON.stringify({name:"capture-locale",type:"module",exports:"./index.js"}));
    await writeFile(join(localeDependency, "index.js"), 'export const format = value => new Intl.NumberFormat("en").format(value);');
    await writeFile(join(source, "Export.svelte"), '<script>import {format} from "capture-locale";</script><p>{format(1234)}</p>');
    await expect(buildGraphs(source, options)).rejects.toThrow("Intl");
    await writeFile(join(source, "helper.ts"), 'export const format = value => new Intl.NumberFormat("en").format(value); export const trim = value => value.trim();');
    await writeFile(join(source, "Export.svelte"), '<script>import {trim} from "./helper"; const document = " Saved ";</script><p>{trim(document)}</p>');
    expect((await buildGraphs(source, options)).outputs.definition.has("export.js")).toBe(true);
    await writeFile(join(source, "Export.svelte"), '<script>import {attachments} from "hitslop/svelte";</script><img src={attachments.url("photo")} alt="Saved photo"/><img src={attachments.url("photo")} alt="Same photo"/>');
    const attached = new TextDecoder().decode((await buildGraphs(source, options)).outputs.definition.get("export.js")!.bytes);
    const attachedContext: Record<string, any> = {};
    runInNewContext(attached, attachedContext);
    const request = {kind:"export",width:320,height:240,snapshot:{},theme:{},resources:{photo:"data:image/png;base64," + "A".repeat(1024 * 1024)}};
    const image = JSON.parse(attachedContext.__slopCapture(JSON.stringify(request)));
    expect(image.frames[0].html.match(/src="slop-attachment:photo"/g)).toHaveLength(2);
    expect(image.frames[0].html).not.toContain("base64");
    expect(image.frames[0].html.length).toBeLessThan(1024);
    expect(image.frames[0].html).toContain('data-slop-capture="static"');
    for (const resources of [{}, {photo:null}, {photo:""}, null])
      expect(() => attachedContext.__slopCapture(JSON.stringify({...request,resources}))).toThrow("missing or is not a supported image");
    for (const method of ['read("photo")', 'import(null,()=>{})']) {
      await writeFile(join(source, "Export.svelte"), `<script>import {attachments} from "hitslop/svelte"; attachments.${method};</script><p>Read only</p>`);
      const forbidden = new TextDecoder().decode((await buildGraphs(source, options)).outputs.definition.get("export.js")!.bytes);
      runInNewContext(forbidden, attachedContext);
      expect(() => attachedContext.__slopCapture(JSON.stringify(request))).toThrow("read-only");
    }
    await writeFile(join(source, "Export.svelte"), "<p>Capture</p>");
    for (const write of ['doc.current.title="bad"', 'doc.change(()=>{})', 'doc.fields.title.set("bad")']) {
      await writeFile(join(source, "Widget.svelte"), view.replace('let {family}', `${write}; let {family}`));
      const mutation = await bundle();
      expect(() => render(mutation, "Original")).toThrow();
    }
    await writeFile(join(source, "Widget.svelte"), '{#await Promise.resolve(1) then value}<p>{value}</p>{/await}');
    await expect(bundle()).rejects.toThrow("synchronous");
  } finally { await rm(root, {recursive: true, force: true}); }
}, 60000);
const resourceKey = (bytes: Uint8Array, name: string) => `media/${createHash("sha256").update(bytes).digest("hex")}${extname(name)}`;

test("conditional dependency exports preserve require and import semantics", async () => {
  const root = await mkdtemp(join(process.cwd(), ".build-test-definition-"));
  try {
    const source = join(root, "source"), stage = join(root, "stage");
    const options = await definitionFixture(source);
    const dependency = join(source, "node_modules", "conditional-helper");
    await mkdir(dependency, { recursive: true });
    await writeFile(join(dependency, "package.json"), JSON.stringify({
      name: "conditional-helper", exports: { import: "./import.mjs", require: "./require.cjs" },
    }));
    await writeFile(join(dependency, "import.mjs"), "export default () => 'import';");
    await writeFile(join(dependency, "require.cjs"), "module.exports = () => 'require';");
    await writeFile(join(source, "legacy.cjs"), "module.exports = require('conditional-helper')();");
    const entry = await readFile(join(source, "slop.ts"), "utf8");
    await writeFile(join(source, "slop.ts"),
      "import legacy from './legacy.cjs'; import modern from 'conditional-helper';\n" +
      entry.replace("title:'Fixture'", "title:legacy + '/' + modern()"));
    const { input } = await buildDefinition(source, stage, options);
    expect(input.declaration.metadata.title).toBe("require/import");
  } finally { await rm(root, { recursive: true, force: true }); }
}, 60000);

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
    expect(input.declaration.views).toEqual({export:true,icon:true,preview:true});
    expect(input.declaration.capabilities).toEqual([]);
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
    expect(input.declaration.views).toEqual({export:false,icon:true,preview:true});
    expect(input.roles.export).toBeUndefined();
    expect(input.resources.some(r=>r.key==="export.js")).toBe(false);
    expect(input.roles.commands).toBeUndefined();
    expect(input.resources.some(r=>r.key==="commands.js")).toBe(false);
    expect(await Bun.file(join(stage,"resources/commands.js")).exists()).toBe(false);
    await writeFile(join(source,"slop.ts"),"globalThis.clock=Date.now();"+entry);
    await expect(buildDefinition(source,stage,options)).rejects.toThrow("host-provided");
  } finally { await rm(root,{recursive:true,force:true}); }
},60000);

test("Widget.svelte is a dedicated UI role and scheduling requires that role", async () => {
  const root = await mkdtemp(join(process.cwd(), ".build-test-definition-"));
  try {
    const source = join(root, "source"), stage = join(root, "stage");
    const options = await definitionFixture(source);
    const entry = await readFile(join(source, "slop.ts"), "utf8");
    await writeFile(join(source, "slop.ts"), entry.replace("title:'Fixture'", "title:'Fixture',widget:{refreshMinutes:30}"));
    await expect(buildDefinition(source, stage, options)).rejects.toThrow("requires a root Widget.svelte");
    await writeFile(join(source, "Widget.svelte"), '<script lang="ts">import art from "./export.svg"; let {family,width,height} = $props();</script><img src={art} alt=""/><p>{family} {width}×{height}</p>');
    const built = await buildDefinition(source, stage, options);
    expect(built.input.declaration.views.widget).toEqual({refreshMinutes:30});
    expect(built.dependencies.ui).toContain(join(source, "Widget.svelte"));
    expect(built.dependencies.definition).not.toContain(join(source, "Widget.svelte"));
    await writeFile(join(source, "slop.ts"), entry);
    expect((await buildDefinition(source, stage, options)).input.declaration.views.widget).toEqual({});
  } finally { await rm(root, {recursive:true, force:true}); }
}, 60000);

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
    for (const name of ["app.svelte", "export.svelte", "preview.svelte", "icon.svelte", "Schema.ts", "command.ts"]) {
      // On case-insensitive volumes, do not overwrite the real role.
      const actual = name === "app.svelte" ? "App.svelte" : name === "export.svelte" ? "Export.svelte" : name === "preview.svelte" ? "Preview.svelte" : name === "icon.svelte" ? "Icon.svelte" : name === "Schema.ts" ? "schema.ts" : "commands.ts";
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
