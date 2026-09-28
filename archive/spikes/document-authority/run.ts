import { mkdir, cp, readFile, readdir } from "node:fs/promises";
import { resolve, join } from "node:path";
import { createHash } from "node:crypto";
const here = import.meta.dir, repo = resolve(here,"../.."), previous = resolve(here,"../engine-placement");
const argv = new Set(process.argv.slice(2)), matrix = argv.has("--matrix");
const web = argv.has("--web"), headless = !web || argv.has("--headless");
const destination = resolve(repo,".hitslop/v1-evidence/document-authority",new Date().toISOString().replaceAll(":","-"));
await mkdir(destination,{recursive:true});
const env = {...process.env, PATH:`/opt/homebrew/bin:/usr/bin:/bin:/usr/sbin:/sbin:${process.env.PATH ?? ""}`};
let active: ReturnType<typeof Bun.spawn> | undefined;
process.on("SIGTERM",()=>{active?.kill();process.exit(143);});
async function run(args: string[], timeout = 180_000) {
  const p = Bun.spawn(args,{cwd:repo,env,stdout:"pipe",stderr:"pipe"}); active=p;
  const timer = setTimeout(()=>p.kill(),timeout);
  const [stdout,stderr,exit] = await Promise.all([new Response(p.stdout).text(),new Response(p.stderr).text(),p.exited]);
  clearTimeout(timer); active=undefined;
  if(exit) throw new Error(`${args.join(" ")}\n${stdout}\n${stderr}`);
  return stdout.trim();
}
await run(["swift","build","--package-path",here,"-c","release"]);
const build = await run(["swift","build","--package-path",here,"-c","release","--show-bin-path"]);
await cp(join(build,"document-authority"),join(destination,"harness"));
if(web){
  await run(["swift","build","--package-path",previous,"-c","release"]);
  const oldBuild = await run(["swift","build","--package-path",previous,"-c","release","--show-bin-path"]);
  await cp(join(oldBuild,"engine-placement"),join(destination,"control"));
}
await cp(join(here,"dist"),join(destination,"assets"),{recursive:true});
await cp(join(here,"fixtures.json"),join(destination,"fixtures.json"));
// Ad-hoc hardened-runtime signing of disposable binaries, empty entitlements like the app.
for(const binary of ["harness",...(web?["control"]:[])]) {
  await run(["codesign","--force","--sign","-","--options","runtime",join(destination,binary)]);
  await run(["codesign","--verify","--strict",join(destination,binary)]);
}
const hash = (bytes: Uint8Array) => createHash("sha256").update(bytes).digest("hex");
async function hashes(root: string, prefix=""): Promise<Record<string,string>> {
  const result: Record<string,string> = {};
  for(const item of await readdir(join(root,prefix),{withFileTypes:true})) {
    const path=join(prefix,item.name);
    if(item.isDirectory()) Object.assign(result,await hashes(root,path));
    else result[path]=hash(await readFile(join(root,path)));
  }
  return result;
}
const metadata={
  timestamp:new Date().toISOString(),mode:matrix?"matrix":"smoke",web,headless,
  os:await run(["sw_vers"]),cpu:await run(["sysctl","-n","machdep.cpu.brand_string"]),
  ramBytes:await run(["sysctl","-n","hw.memsize"]),commit:await run(["git","rev-parse","HEAD"]),
  sourceStatus:await run(["git","status","--short"]),
  swift:await run(["swift","--version"]),bun:Bun.version,
  harnessHash:hash(await readFile(join(destination,"harness"))),
  controlHash:web?hash(await readFile(join(destination,"control"))):null,
  assets:await hashes(join(destination,"assets")),
  signing:"ad-hoc hardened runtime, no entitlements; not a Developer ID/notarization test; JIT mode not inferred",
  methods:{
    headless:"Process physical footprint and lifetime peak; no WKWebView allocation. Library loading included in empty process. Full-copy atomic reducer, full validation and UTF-8 capacity check included.",
    web:"Same placement Svelte view and workload, 40 rendered rows. Two rAF render-opportunity proxy; 200ms first-unsaved periodic save parity; acceptance is not durability. Headless durableEdit measures commit completion separately.",
    contention:"Foreground 100-row checkbox arrives ~1ms after a 40k-row 1024-op batch begins; queue/VM scheduling order is not guaranteed. Controlled arrival delay is reported.",
    memory:"Host plus identified WebContent only for UI; GPU/network excluded. Peak is host-only, not sum of unrelated process peaks. Release/GC may retain capacity.",
  }
};
await Bun.write(join(destination,"environment.json"),JSON.stringify(metadata,null,2));
console.log(`Evidence: ${destination}`);
const configs: any[] = [];
const topologies=["shared-vm","separate-vm","shared-context"];
const base={core:join(destination,"assets/core.js"),fixtures:join(destination,"fixtures.json"),dist:join(destination,"assets")};
const trials=matrix?5:1;
configs.push({...base,mode:"fixtures",trial:0});
for(let trial=0;trial<trials;trial++) {
  const rotation=trial%3, order=[...topologies.slice(rotation),...topologies.slice(0,rotation)];
  if(headless) {
    for(const rows of matrix?[100,1000,5000]:[1000]) for(const documents of matrix?[1,5,10,25]:[1,10]) for(const topology of order)
      configs.push({...base,mode:"headless",topology,rows,documents,samples:matrix?30:8,trial,seed:join(destination,`assets/checklist-${rows}.json`)});
    for(const topology of order) {
      configs.push({...base,mode:"headless",topology,rows:40000,documents:1,samples:matrix?30:8,trial,seed:join(destination,"assets/checklist-40000.json")});
      configs.push({...base,mode:"contention",topology,rows:100,documents:2,trial,seed:join(destination,"assets/checklist-100.json"),largeSeed:join(destination,"assets/checklist-40000.json")});
    }
  }
  if(web) for(const rows of matrix?[100,1000,5000]:[1000]) for(const documents of matrix?[1,5,10]:[1]) {
    const candidates=[...topologies.map(t=>`jsc-${t}`),"baseline","native"];
    const offset=(trial+rows+documents)%candidates.length;
    for(const candidate of [...candidates.slice(offset),...candidates.slice(0,offset)])
      configs.push({...base,mode:"web",candidate,topology:candidate.replace("jsc-",""),rows,documents,samples:matrix?20:8,trial,seed:join(destination,`assets/checklist-${rows}.json`)});
  }
}
const index: any[] = [];
for(const [i,config] of configs.entries()) {
  const stem=`${String(i).padStart(4,"0")}-${config.mode}-${config.candidate??config.topology??"fixtures"}-${config.rows??0}-${config.documents??0}-t${config.trial}`;
  const input=join(destination,`${stem}.config.json`), output=join(destination,`${stem}.json`);
  await Bun.write(input,JSON.stringify(config));
  try {
    if(config.mode==="web"&&!config.candidate.startsWith("jsc"))
      await run([join(destination,"control"),base.dist,output,config.candidate,String(config.rows),String(config.documents),"checklist",String(config.samples)]);
    else await run([join(destination,"harness"),input,output]);
    const result=await Bun.file(output).json();
    index.push({file:output,config,passed:true});
    console.log(`${i+1}/${configs.length} ${stem} ${JSON.stringify({memory:result.hostAndContentMiB??result.loaded?.MiB,checkbox:result.results?.checkbox?.accepted?.p95??result.results?.checkbox?.apply?.p95})}`);
  } catch(error) {
    index.push({file:output,config,passed:false,error:String(error)});
    await Bun.write(join(destination,"index.json"),JSON.stringify(index,null,2));
    throw error;
  }
  await Bun.write(join(destination,"index.json"),JSON.stringify(index,null,2));
}
console.log(`Completed ${index.length} cases: ${destination}`);
