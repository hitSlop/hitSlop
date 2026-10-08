import { build } from "esbuild";
import { compile } from "svelte/compiler";
import { mkdir, readFile, cp } from "node:fs/promises";
import { dirname, join, resolve } from "node:path";
import { Document } from "../../packages/document/src/document";
import { MemoryStore } from "../../packages/document/src/memory";
import { checklist, initial } from "../engine-placement/fixtures";
const here = import.meta.dir, prior = resolve(here, "../engine-placement"), dist = join(here, "dist");
await mkdir(dist, { recursive: true });
await build({entryPoints:[join(here,"runtime.ts")], outfile:join(dist,"core.js"), bundle:true, format:"iife", globalName:"AuthorityCore", platform:"neutral", target:"es2022"});
for (const rows of [100, 1000, 5000, 40000]) {
  const doc = await Document.open(checklist, new MemoryStore(), initial(rows));
  // All candidates read values with identical stable IDs, not independently randomized seeds.
  await Bun.write(join(dist, `checklist-${rows}.json`), JSON.stringify(doc.current));
  await Bun.write(join(dist, `checklist-${rows}.snapshot`), doc.exportSnapshot());
  await doc.close();
}
await Bun.write(join(dist,"checklist.schema.json"), JSON.stringify(checklist.descriptor));
for (const variant of ["baseline","matched"]) {
  const loro = dirname(Bun.resolveSync("loro-crdt/package.json", variant === "baseline" ? resolve(here,"../..") : prior));
  await cp(join(loro,"web"),join(dist,`loro-${variant}`),{recursive:true});
  await build({entryPoints:[join(prior,"web/wasm.ts")],outfile:join(dist,`wasm-${variant}.js`),bundle:true,format:"esm",platform:"browser",target:"safari18",plugins:[{
    name:"loro",setup(b){
      b.onResolve({filter:/^loro-crdt\/web\/loro_wasm(?:\.js)?$/},()=>({path:`./loro-${variant}/loro_wasm.js`,external:true}));
      b.onResolve({filter:/^loro-crdt(?:\/web)?$/},()=>({path:`./loro-${variant}/index.js`,external:true}));
    }
  }]});
}
const original = await readFile(join(prior,"web/app.ts"),"utf8");
const needle = 'if (config.candidate.startsWith("native")) {';
if (original.split(needle).length !== 2) throw new Error("Placement harness changed; review integration");
// Reuse exact Svelte component, scenarios, autosave checks and rendering proxy.
let entry = `import { openJSC } from ${JSON.stringify(join(here,"web/adapter.ts"))};\n` + original.replace(needle,
  `if (config.candidate.startsWith("jsc")) { adapter = await openJSC(); current = adapter.current(); } else ${needle}`);
entry = entry.replace("engineReadyMS,\n        readyMS,", "sharingMS: adapter.sharingMS ?? [],\n        engineReadyMS,\n        readyMS,");
await build({stdin:{contents:entry,loader:"ts",resolveDir:join(prior,"web"),sourcefile:"authority-app.ts"},outfile:join(dist,"app.js"),bundle:true,format:"esm",platform:"browser",target:"safari18",conditions:["browser"],plugins:[{
  name:"svelte",setup(b){ b.onLoad({filter:/\.svelte$/},async a=>({contents:compile(await readFile(a.path,"utf8"),{filename:a.path,generate:"client",css:"injected"}).js.code,loader:"js",resolveDir:dirname(a.path)})); }
}]});
await Bun.write(join(dist,"index.html"),'<!doctype html><meta charset="utf-8"><title>Document authority spike</title><script type="module" src="app.js"></script>');
console.log("Prepared shared core, identical fixtures and reused placement view/workload.");
