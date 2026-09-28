import { LoroDoc, LoroText } from "loro-crdt";
import { mkdir } from "node:fs/promises";
import { join, resolve } from "node:path";
import { RelayLog } from "./relay";
// Component integration: native SQLite owners + opaque durable server log + Loro
// WASM. Transport is tested separately; this is not remote-visible-frame timing.
const here = import.meta.dir,
  out = resolve(
    here,
    "../../.hitslop/v1-evidence/engine-placement/replay",
    new Date().toISOString().replaceAll(":", "-"),
  );
await mkdir(out, { recursive: true });
const schema = join(here, "dist/checklist.schema.json"),
  seed = join(here, "dist/checklist-1.snapshot");
const base = new LoroDoc();
base.import(new Uint8Array(await Bun.file(seed).arrayBuffer()));
const version = Buffer.from(base.oplogVersion().encode()).toString("base64");
let sequence = 0;
async function native(name: string, operations: unknown[] = [], imports: string[] = []) {
  const id = ++sequence,
    input = join(out, `${id}.request.json`),
    output = join(out, `${id}.response.json`);
  await Bun.write(
    input,
    JSON.stringify({
      schema,
      seed,
      operations,
      imports,
      storage: join(out, name),
      fromVersion: version,
    }),
  );
  const p = Bun.spawn(
    [join(here, ".build/out/Products/Release/engine-placement"), "probe", input, output],
    { stdout: "pipe", stderr: "pipe" },
  );
  const err = await new Response(p.stderr).text();
  if (await p.exited) throw new Error(err);
  const response = await Bun.file(output).json();
  if (response.rejected) throw new Error(response.rejected);
  return response;
}
const check = (ok: boolean, detail: string) => {
  if (!ok) throw new Error(detail);
};
const a = await native("a", [
  { type: "splice", path: ["title"], index: 0, deleteCount: 0, text: "A " },
]);
const b = await native("b", [
  { type: "splice", path: ["title"], index: 16, deleteCount: 0, text: " B" },
]);
const file = join(out, "relay.sqlite");
let relay = new RelayLog(file);
const first = relay.append("a-1", Buffer.from(a.delta, "base64"));
// Drop the first ACK, retry immutable bytes, then restart the server.
check(
  relay.append("a-1", Buffer.from(a.delta, "base64")) === first,
  "Lost ACK created duplicate entry",
);
relay.append("b-1", Buffer.from(b.delta, "base64"));
let refused = false;
try {
  relay.append("a-1", new Uint8Array([1, 2, 3]));
} catch {
  refused = true;
}
check(refused, "Retry ID accepted different bytes");
relay.close();
relay = new RelayLog(file);
const entries = relay.after(0);
check(entries.length === 2, "Server restart lost committed entries");
const imports = entries.map((e) => Buffer.from(e.bytes).toString("base64"));
const a2 = await native("a", [], imports),
  b2 = await native("b", [], imports.toReversed());
check(
  a2.data.title === "A Engine placement B" && a2.data.title === b2.data.title,
  "Native peers did not converge",
);
const wasm = new LoroDoc();
wasm.import(new Uint8Array(await Bun.file(seed).arrayBuffer()));
wasm.importBatch(entries.map((e) => e.bytes));
check(
  wasm.getMap("data").get("title")!.toString() === a2.data.title,
  "WASM/native projection differs",
);
// Synthetic offline backlog, persisted in the relay before replay begins.
// This does not exercise a client-side durable outbox.
const source = new LoroDoc();
source.import(new Uint8Array(await Bun.file(seed).arrayBuffer()));
const title = source.getMap("data").get("title");
if (!(title instanceof LoroText)) throw new Error("Expected text container");
let from = source.oplogVersion();
for (let i = 0; i < 1000; i++) {
  title.insert(16 + i, "x");
  source.commit();
  const delta = source.export({ mode: "update", from });
  from.free();
  from = source.oplogVersion();
  relay.append(`offline-${i}`, delta);
}
let cursor = 2,
  total = 0;
const started = performance.now();
while (true) {
  const page = relay.after(cursor);
  if (!page.length) break;
  await native(
    "backlog",
    [],
    page.map((e) => Buffer.from(e.bytes).toString("base64")),
  );
  // Advance the receive cursor only after the native process durably completes.
  cursor = page.at(-1)!.seq;
  total += page.length;
}
const reopened = await native("backlog");
check(
  total === 1000 && reopened.data.title === "Engine placement" + "x".repeat(1000),
  "Backlog lost edits on restart",
);
const report = {
  pass: true,
  convergedTitle: a2.data.title,
  lostAckDeduplicated: true,
  reusedIDRefused: true,
  serverRestart: true,
  nativeReopen: true,
  wasmNativeConvergence: true,
  backlogEntries: total,
  backlogDurableReplayMS: performance.now() - started,
  method:
    "Native headless processes own SQLite; controller delivers durable log pages directly. Does not measure WebSocket delivery or UI latency. Pending imports must complete within a page; durable missing-dependency buffering remains a gap.",
};
await Bun.write(join(out, "report.json"), JSON.stringify(report, null, 2));
await Bun.write(
  resolve(here, "../../.hitslop/v1-evidence/engine-placement/replay-latest.txt"),
  out + "\n",
);
console.log(report);
relay.close();
from.free();
base.free();
source.free();
wasm.free();
