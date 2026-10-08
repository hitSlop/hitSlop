import { applyOps } from "./patch";
// Boundary coverage: generated WASM API + Swift/UniFFI, independently checked
// against literal values. Cross-binding byte replay catches transport/FFI defects.
import assert from "node:assert/strict";
import { mkdir } from "node:fs/promises";
import { join, resolve } from "node:path";
import { createHash } from "node:crypto";
const root = import.meta.dir;
const out = resolve(root, "../../.hitslop/v1-evidence/hitslop-core");
await mkdir(out, { recursive: true });
const fixture = await Bun.file(join(root, "fixtures/checklist.json")).json();
const schema = JSON.stringify(fixture.schema);
const wasmPath = join(root, "dist/wasm/hitslop_core_wasm_spike_bg.wasm");
const { default: init, WasmDocument } = await import(join(root, "dist/wasm/hitslop_core_wasm_spike.js"));
await init({ module_or_path: await Bun.file(wasmPath).arrayBuffer() });
const view = (d: any) => JSON.parse(d.snapshot());
const receipts: string[] = [];
for (const item of fixture.scenarios) {
  const doc = WasmDocument.create(schema, JSON.stringify(item.initial ?? fixture.initial));
  try {
    const before = view(doc);
    const version = doc.version();
    const seed = doc.checkpoint();
    const intents = item.intents.map((op: any) => op.base === "$current" ? { ...op, base: version } : op);
    const batch = JSON.stringify({ intents });
    if (item.error) {
      assert.throws(() => doc.apply(batch), (e: unknown) => String(e).includes(item.error), item.name);
      assert.deepEqual(view(doc), before, `${item.name}: state/version/publication changed after rejection`);
    } else {
      const accepted = JSON.parse(doc.apply(batch));
      const after = view(doc);
      assert.deepEqual(after.value, item.after, item.name);
      assert.equal(accepted.patch.session, before.session);
      assert.equal(accepted.patch.previous, before.sequence);
      assert.equal(accepted.patch.sequence, before.sequence + 1);
      const patched = applyOps(before.value, accepted.patch.ops);
      assert.deepEqual(patched, item.after, `${item.name}: patch mismatch`);
      assert.deepEqual(accepted.patch.issues, after.issues);
      const incremental = WasmDocument.open(schema, seed);
      const full = WasmDocument.open(schema, doc.checkpoint());
      try {
        incremental.import_updates(doc.export_since(version));
        assert.deepEqual(view(incremental).value, item.after, `${item.name}: update replay`);
        assert.deepEqual(view(full).value, item.after, `${item.name}: checkpoint replay`);
        assert.equal(incremental.version(), doc.version());
      } finally { incremental.free(); full.free(); }
    }
    receipts.push(item.name);
  } finally { doc.free(); }
}

const wasm = WasmDocument.create(schema, JSON.stringify(fixture.initial));
const seed = wasm.checkpoint();
const version = wasm.version();
wasm.apply(JSON.stringify({ intents: fixture.scenarios[0].intents }));
const transferPath = join(out, "wasm-to-native.json");
await Bun.write(transferPath, JSON.stringify({ checkpoint: Buffer.from(seed).toString("base64"), update: Buffer.from(wasm.export_since(version)).toString("base64"), expected: fixture.scenarios[0].after }));
const swiftPath = join(root, ".build/release/core-conformance");
const swift = Bun.spawn([swiftPath, join(root, "fixtures/checklist.json"), transferPath], { stdout: "pipe", stderr: "pipe" });
const [stdout, stderr, code] = await Promise.all([new Response(swift.stdout).text(), new Response(swift.stderr).text(), swift.exited]);
assert.equal(code, 0, stderr);
const native = JSON.parse(stdout);
assert.deepEqual(native.scenarios, receipts);
const reopened = WasmDocument.open(schema, Buffer.from(native.checkpoint, "base64"));
try {
  reopened.import_updates(Buffer.from(native.update, "base64"));
  // Compare with the independent fixture, not merely the native engine's opinion.
  assert.deepEqual(view(reopened).value, fixture.scenarios[0].after);
} finally { reopened.free(); wasm.free(); }

async function hash(path: string) { return createHash("sha256").update(new Uint8Array(await Bun.file(path).arrayBuffer())).digest("hex"); }
const report = {
  milestone: "binding-and-semantics-foundation", s1Passed: false,
  date: new Date().toISOString(), platform: `${process.platform}/${process.arch}`,
  fixturesPerBinding: receipts.length, nativeWasmByteReplay: "both directions passed",
  scenarios: receipts,
  hashes: { wasm: await hash(wasmPath), swiftExecutable: await hash(swiftPath), cargoLock: await hash(join(root, "Cargo.lock")), fixtures: await hash(join(root, "fixtures/checklist.json")) },
  bytes: { wasm: Bun.file(wasmPath).size, swiftExecutable: Bun.file(swiftPath).size },
  pending: ["full conformance", "system IME manual gate", "production writer routing/crash durability", "signing/CI"],
  integrationEvidence: "See engine-placement --rust evidence and NATIVE-OWNER.md; this runner proves bindings only",
};
await Bun.write(join(out, "results.json"), JSON.stringify(report, null, 2) + "\n");
console.log(JSON.stringify(report, null, 2));
