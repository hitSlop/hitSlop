import { join, resolve } from "node:path";
import { mkdir } from "node:fs/promises";
import { isDeepStrictEqual } from "node:util";
import { Document } from "../../packages/document/src/document";
import { fromDescriptor, schemaKey } from "../../packages/document/src/schema";
import { MemoryStore } from "../../packages/document/src/memory";
import { LoroDoc } from "../../node_modules/loro-crdt";
const here = import.meta.dir,
  out = resolve(here, "../../.hitslop/v1-evidence/engine-placement/audit");
await mkdir(out, { recursive: true });
const cases = [
  { name: "checklist_projection", fixture: "checklist", ops: [] },
  { name: "rich_text_projection", fixture: "richtext", ops: [] },
  { name: "mixed_projection", fixture: "mixed", ops: [] },
  {
    name: "atomic_rejection",
    fixture: "checklist",
    ops: [
      { type: "splice", path: ["title"], index: 0, deleteCount: 0, text: "bad" },
      { type: "set", path: ["title"], value: true },
    ],
    rejected: true,
  },
  { name: "merged_plain_text_scalar", fixture: "checklist", ops: [], anomaly: true },
];
const results = [];
for (const c of cases) {
  const schemaFile = join(here, `dist/${c.fixture}.schema.json`),
    schema = await Bun.file(schemaFile).json();
  let seed = join(here, `dist/${c.fixture}-1.snapshot`);
  if (c.anomaly) {
    const engine = new LoroDoc();
    engine.import(new Uint8Array(await Bun.file(seed).arrayBuffer()));
    engine.getMap("data").set("title", "plain JSON text");
    engine.commit();
    seed = join(out, "anomaly.snapshot");
    await Bun.write(seed, engine.export({ mode: "snapshot" }));
    engine.free();
  }
  const store = new MemoryStore();
  await store.checkpoint(
    "0",
    new Uint8Array(await Bun.file(seed).arrayBuffer()),
    schemaKey(schema),
  );
  const baseline = await Document.open(fromDescriptor(schema), store, {});
  const request = join(out, c.name + ".request.json"),
    response = join(out, c.name + ".response.json");
  await Bun.write(request, JSON.stringify({ schema: schemaFile, seed, operations: c.ops }));
  const process = Bun.spawn(
    [join(here, ".build/out/Products/Release/engine-placement"), "probe", request, response],
    { stdout: "pipe", stderr: "pipe" },
  );
  const stderr = await new Response(process.stderr).text();
  if (await process.exited) throw new Error(stderr);
  const native = await Bun.file(response).json();
  results.push({
    name: c.name,
    pass:
      isDeepStrictEqual(native.data, baseline.current) &&
      (!c.rejected || (native.rejected !== null && native.versionUnchanged)),
    expected: baseline.current,
    actual: native.data,
    expectedIssues: baseline.issues,
    rejected: native.rejected,
    versionUnchanged: native.versionUnchanged,
  });
  await baseline.close();
}
const report = {
  method:
    "Current SDK projection is the oracle; native Loro 1.16.2, baseline 1.16.1. These are semantic conformance probes, not speed measurements.",
  results,
  missingCapabilities: [
    "Full preserve-and-flag issue stream and derived identity ownership",
    "Tree editing",
    "Remote-aware text draft ancestry, IME and selection",
    "Identity-addressed structural operations across stale UI revisions",
    "Complete author SDK surface and subscriptions",
    "Native creation from initial.json and mergeable composite initialization",
    "Request idempotency / unknown-acceptance recovery",
    "Durable missing-dependency buffering and discard rules",
    "Integrated undo and real WebContent-crash recovery",
    "Durable relay protocol and remote-visible-frame measurement",
  ],
};
await Bun.write(join(out, "report.json"), JSON.stringify(report, null, 2));
console.log(results.map((r) => `${r.pass ? "PASS" : "GAP "} ${r.name}`).join("\n"));
if (results.some((r) => !r.pass) || report.missingCapabilities.length) process.exitCode = 2;
