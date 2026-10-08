import { LoroDoc } from "loro-crdt";
import { mkdir } from "node:fs/promises";
import { resolve } from "node:path";
// Independent numeric oracle: (1e16 - 1e16) + 1 is 1. Same operation history
// must not project differently merely because exports regroup its deltas.
const a = new LoroDoc(),
  counter = a.getCounter("counter");
counter.increment(1e16);
a.commit();
const seed = a.export({ mode: "snapshot" }),
  from = a.oplogVersion();
counter.increment(-1e16);
a.commit();
counter.increment(1);
a.commit();
const b = new LoroDoc();
b.import(seed);
b.import(a.export({ mode: "update", from }));
const report = {
  loro: "1.16.2",
  expected: 1,
  accepted: counter.value,
  replayed: b.getCounter("counter").value,
  sameVersion:
    JSON.stringify(a.oplogVersion().toJSON()) === JSON.stringify(b.oplogVersion().toJSON()),
  pass: counter.value === 1 && b.getCounter("counter").value === 1,
};
const out = resolve(import.meta.dir, "../../.hitslop/v1-evidence/engine-placement");
await mkdir(out, { recursive: true });
await Bun.write(out + "/counter-probe.json", JSON.stringify(report, null, 2));
console.log(report);
a.free();
b.free();
from.free();
if (!report.pass) process.exitCode = 2;
