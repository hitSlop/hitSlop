import { mkdir } from "node:fs/promises";
import { join } from "node:path";
const here = import.meta.dir;
const roots = process.argv.slice(2).filter(x=>!x.startsWith("--"));
if(!roots.length) throw new Error("Usage: bun summarize.ts EVIDENCE_DIR [EVIDENCE_DIR ...]");
const records: any[] = [], environments: any[] = [];
for(const root of roots) {
  environments.push(await Bun.file(join(root,"environment.json")).json());
  const index = await Bun.file(join(root,"index.json")).json();
  for(const entry of index) {
    if(!entry.passed) throw new Error(`Failed case: ${entry.file}`);
    const result = await Bun.file(entry.file).json();
    records.push({...result,config:entry.config,file:entry.file});
  }
}
const median = (v:number[]) => [...v].sort((a,b)=>a-b)[Math.floor(v.length/2)];
const range = (v:number[]) => ({median:median(v),min:Math.min(...v),max:Math.max(...v)});
const format = (n:number|undefined) => n===undefined?"—":n.toFixed(1);
function groups(mode:string) {
  const result = new Map<string,any[]>();
  for(const r of records.filter(r=>r.config.mode===mode)) {
    const c=r.config,key=JSON.stringify([c.candidate??c.topology,c.rows,c.documents]);
    result.set(key,[...(result.get(key)??[]),r]);
  }
  return [...result.values()];
}
const headless=groups("headless").map(rs=>({
  topology:rs[0].config.topology,rows:rs[0].config.rows,documents:rs[0].config.documents,trials:rs.length,
  loaded:range(rs.map(r=>r.loaded.MiB)),baseline:range(rs.map(r=>r.before.MiB)),
  delta:range(rs.map(r=>r.loaded.MiB-r.before.MiB)),peak:range(rs.map(r=>r.finalMemory.peakMiB)),
  checkbox:range(rs.map(r=>r.results.checkbox.apply.p95)),move:range(rs.map(r=>r.results.move.apply.p95)),
  serialize:range(rs.map(r=>r.results.checkbox.serialize.p95)),durable:range(rs.map(r=>r.durableEdit.p95)),
  open:range(rs.map(r=>r.openMS)),
  release:range(rs.map(r=>r.released.MiB)),cycles:rs.map(r=>r.afterCycles.map((x:any)=>x.MiB)),
}));
const web=groups("web").map(rs=>({
  candidate:rs[0].config.candidate,rows:rs[0].config.rows,windows:rs[0].config.documents,trials:rs.length,
  memory:range(rs.map(r=>r.hostAndContentMiB)),open:range(rs.map(r=>r.openMS)),
  checkbox:range(rs.map(r=>r.results.checkbox.accepted.p95)),move:range(rs.map(r=>r.results.move.accepted.p95)),
  render:range(rs.map(r=>r.results.checkbox.renderOpportunity.p95)),
  drain:range(rs.map(r=>r.sustainedTyping.drainMS)),saves:range(rs.map(r=>r.sustainedTyping.savesDuringTyping)),
}));
const contention=groups("contention").map(rs=>({
  topology:rs[0].config.topology,trials:rs.length,
  idle:range(rs.map(r=>r.idle.p95)),busy:range(rs.map(r=>r.contended.p95)),background:range(rs.map(r=>r.background.p95)),
}));
const summary={environments,headless,web,contention,records};
await mkdir(join(here,"evidence"),{recursive:true});
await Bun.write(join(here,"evidence/results.json"),JSON.stringify(summary,null,2));
let md="# JavaScriptCore authority measurements\n\nEach latency is the median of fresh-process p95s, not a pooled percentile. Memory is median MiB. Full ranges, raw samples and environment hashes are in results.json.\n\n";
md+="## Headless\n\nThe pre-JSC baseline already includes source and seed buffers.\n\n| Topology | Rows | Documents | Trials | Loaded MiB | Above pre-JSC MiB | Lifetime peak MiB | Checkbox ms | Move ms | Serialize ms | Durable edit ms |\n|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|\n";
for(const r of headless) md+=`| ${r.topology} | ${r.rows} | ${r.documents} | ${r.trials} | ${format(r.loaded.median)} | ${format(r.delta.median)} | ${format(r.peak.median)} | ${format(r.checkbox.median)} | ${format(r.move.median)} | ${format(r.serialize.median)} | ${format(r.durable.median)} |\n`;
md+="\n## Cross-document contention\n\nForeground 100-row edit during a 40,000-row 1,024-op batch. The ~1ms arrival offset is reported in raw evidence.\n\n| Topology | Trials | Idle p95 ms | Contended p95 ms | Background p95 ms |\n|---|---:|---:|---:|---:|\n";
for(const r of contention) md+=`| ${r.topology} | ${r.trials} | ${format(r.idle.median)} | ${format(r.busy.median)} | ${format(r.background.median)} |\n`;
md+="\n## Same Svelte workload\n\nFull snapshots for JSC, existing incremental publication for controls. All use first-unsaved 200ms periodic saving. Acceptance does not mean persistence. Render is a two-frame opportunity proxy, not paint.\n\n| Candidate | Rows | Windows | Trials | Host + content MiB | Open ms | Checkbox ms | Move ms | Render ms | Drain ms |\n|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|\n";
for(const r of web) md+=`| ${r.candidate} | ${r.rows} | ${r.windows} | ${r.trials} | ${format(r.memory.median)} | ${format(r.open.median)} | ${format(r.checkbox.median)} | ${format(r.move.median)} | ${format(r.render.median)} | ${format(r.drain.median)} |\n`;
await Bun.write(join(here,"evidence/table.md"),md);
console.log(`Summarized ${records.length} records from ${roots.length} runs.`);
