import {mkdir} from "node:fs/promises";
import {join,resolve} from 'node:path';
const directory=resolve(process.argv[2] ?? (await Bun.file(new URL('../../.hitslop/v1-evidence/engine-placement/latest.txt',import.meta.url)).text()).trim());
const index=await Bun.file(join(directory,'index.json')).json();
const records=await Promise.all(index.map(async(x:any)=>({...await Bun.file(join(directory,x.file)).json(),trial:x.trial})));
const median=(xs:number[])=>xs.toSorted((a,b)=>a-b)[Math.floor(xs.length/2)]!;
const scenarios=['checkbox','typing','move','paste'];
const cells=[];
for(const rows of [1000,5000,40000])for(const windows of [1,5,10]){
 const get=(c:string)=>records.filter((r:any)=>r.candidate===c&&r.rows===rows&&r.windows===windows);
 const allRust=get('rust-core'),allSwift=get('native'),allWasm=get('matched');
 if(!allRust.length||!allSwift.length||!allWasm.length)continue;
 const rust=allRust.filter((r:any)=>!r.failed),swift=allSwift.filter((r:any)=>!r.failed),wasm=allWasm.filter((r:any)=>!r.failed);
 const complete=rust.length===5&&swift.length===5&&wasm.length===5;
 const gates=[];
 for(const name of scenarios){
  const acceptance=median(rust.map((r:any)=>r.results[name].accepted.p95));
  const control=median(swift.map((r:any)=>r.results[name].accepted.p95));
  const render=median(rust.map((r:any)=>r.results[name].renderOpportunity.p95));
  const webRender=median(wasm.map((r:any)=>r.results[name].renderOpportunity.p95));
  gates.push({scenario:name,acceptance,swift:control,wasm:median(wasm.map((r:any)=>r.results[name].accepted.p95)),
   worstP95:Math.max(...rust.map((r:any)=>r.results[name].accepted.p95)),render,wasmRender:webRender,
   relativePass:acceptance<=control+Math.max(2,control*.2),renderPass:render<=webRender+16.7,
   patchUpperBoundWorstP95:Math.max(...rust.map((r:any)=>r.results[name].patchUpperBound?.p95??Infinity))});
 }
 const memory=median(rust.map((r:any)=>r.hostAndContentMiB)),swiftMemory=median(swift.map((r:any)=>r.hostAndContentMiB));
 cells.push({rows,windows,stress:rows===40000,runs:allRust.length,successfulRuns:rust.length,complete,failures:[...allRust,...allSwift,...allWasm].filter((r:any)=>r.failed).map((r:any)=>({candidate:r.candidate,trial:r.trial,error:r.error})),memory,swiftMemory,wasmMemory:median(wasm.map((r:any)=>r.hostAndContentMiB)),memoryPass:memory<=swiftMemory*1.1,
  openMS:median(rust.map((r:any)=>r.openMS)),worstDrainMS:Math.max(...rust.map((r:any)=>r.sustainedTyping.drainMS)),
  accepted50Pass:complete&&rust.every((r:any)=>r.results.checkbox.accepted.p95<=50&&r.results.move.accepted.p95<=50),
  drainPass:complete&&rust.every((r:any)=>r.sustainedTyping.drainMS<50),scenarios:gates});
}
const probePath = new URL('../../.hitslop/v1-evidence/hitslop-core/publication-cost.json', import.meta.url);
const publicationProbe = await Bun.file(probePath).exists() ? await Bun.file(probePath).json() : undefined;
const publicationAloneFails = publicationProbe?.results.some((r:any)=>r.rows===5000 && r.p95MS>2) ?? false;
const report={directory,publicationProbe,ordinaryPass:cells.filter(c=>!c.stress).length===6&&cells.filter(c=>!c.stress).every(c=>c.complete&&c.memoryPass&&c.accepted50Pass&&c.drainPass&&c.scenarios.every(s=>s.relativePass&&s.renderPass)),
 patch2MSPass:publicationAloneFails ? false : null,
 patchDiagnosticWithin2MS:cells.filter(c=>c.rows===5000).length===3&&cells.filter(c=>c.rows===5000).every(c=>c.complete&&c.scenarios.every(s=>s.patchUpperBoundWorstP95<=2)),
 method:'patchUpperBound is a diagnostic estimate: includes request transport/queueing, excludes native response encoding within apply time. It is not an isolated transfer measurement or proof of the 2 ms gate.',cells};
await Bun.write(join(directory,'native-gates.json'),JSON.stringify(report,null,2)+'\n');
const lines=['| Rows | Windows | Checkbox p95 Rust / Swift / WASM | Move p95 Rust / Swift / WASM | Memory MiB Rust / Swift / WASM | Worst drain ms | Rust successful runs |', '|---:|---:|---|---|---|---:|---:|'];
for(const c of cells){const check=c.scenarios[0]!,move=c.scenarios[2]!;const f=(n:number)=>Number.isFinite(n)?n.toFixed(1):"—";lines.push(`| ${c.rows} | ${c.windows} | ${[check.acceptance,check.swift,check.wasm].map(f).join(' / ')} | ${[move.acceptance,move.swift,move.wasm].map(f).join(' / ')} | ${[c.memory,c.swiftMemory,c.wasmMemory].map(f).join(' / ')} | ${f(c.worstDrainMS)} | ${c.successfulRuns}/${c.runs} |`);}
await Bun.write(join(directory,'native-table.md'),lines.join('\n')+'\n');
console.log(JSON.stringify({directory,ordinaryPass:report.ordinaryPass,patch2MSPass:report.patch2MSPass,cells:cells.length}));console.log(lines.join('\n'));

if (process.argv.includes("--record")) {
  if (records.length !== 99 || cells.filter(c=>!c.stress).some(c=>c.runs!==5)) throw Error("Cannot record an incomplete matrix");
  const output = new URL('./evidence/', import.meta.url); await mkdir(output,{recursive:true});
  const environment = await Bun.file(join(directory,'environment.json')).json();
  const compact = records.map((r:any)=>r.failed ? {
    candidate:r.candidate,rows:r.rows,windows:r.windows,trial:r.trial,failed:true,
    error:r.error.slice(r.error.indexOf('SPIKE FAILED:')),
  } : {
    candidate:r.candidate,rows:r.rows,windows:r.windows,trial:r.trial,openMS:r.openMS,
    hostAndContentMiB:r.hostAndContentMiB,drainMS:r.sustainedTyping.drainMS,savesDuringTyping:r.sustainedTyping.savesDuringTyping,
    scenarios:Object.fromEntries(scenarios.map(name=>[name,{accepted:r.results[name].accepted,renderOpportunity:r.results[name].renderOpportunity,engine:r.results[name].engine,patchEstimate:r.results[name].patchUpperBound}])),
  });
  const gateCopy=JSON.parse(JSON.stringify(report));
  gateCopy.directory=directory.split('/').at(-1);
  for(const cell of gateCopy.cells)for(const failure of cell.failures)failure.error=failure.error.slice(failure.error.indexOf('SPIKE FAILED:'));
  await Bun.write(new URL('native-results.json',output),JSON.stringify({
    environment,matrixPolicy:await Bun.file(join(directory,'matrix-policy.json')).json(),
    integration:await Bun.file(join(directory,'integration.json')).json(),
    gates:gateCopy,runs:compact,
    reruns:['0-40000-1-rust-core.json','1-5000-1-matched.json'],
    note:'Interrupted attempt databases and results were archived; these two cells were repeated from fresh SQLite state using identical frozen executables/assets. Raw evidence remains under .hitslop/v1-evidence/engine-placement/'+directory.split('/').at(-1),
  },null,2)+'\n');
  await Bun.write(new URL('native-table.md',output),lines.join('\n')+'\n');
}
