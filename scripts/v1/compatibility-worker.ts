// Reuses compiled runtime modules; historical artifacts run as isolated batches.
// Never imports the source document engine. Harness contract for every sealed
// runtime's index.js: initialize, fromDescriptor and Document (open, current, issues,
// fields, applyAll, importUpdates, exportUpdates, flush, compact, close).
import { strict as assert } from "node:assert";
import { join, resolve } from "node:path";
import { pathToFileURL } from "node:url";
import { createHash } from "node:crypto";
import { SQLiteStore } from "../../packages/document/test-support/sqlite";

export function canonical(value: unknown): string {
  if (Array.isArray(value)) return `[${value.map(canonical).join(",")}]`;
  if (value !== null && typeof value === "object")
    return `{${Object.entries(value)
      .sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0))
      .map(([key, item]) => `${JSON.stringify(key)}:${canonical(item)}`)
      .join(",")}}`;
  return JSON.stringify(value);
}

const loaded = new Map<string, Promise<any>>();

export function loadRuntime(root: string) {
  root = resolve(root);
  let runtime = loaded.get(root);
  if (!runtime) {
    runtime = initializeRuntime(root);
    loaded.set(root, runtime);
  }
  return runtime;
}

async function initializeRuntime(root: string) {
  const loro = await import(pathToFileURL(join(root, "loro/index.js")).href);
  await loro.default({
    module_or_path: await Bun.file(join(root, "loro/loro_wasm_bg.wasm")).bytes(),
  });
  const runtime = await import(pathToFileURL(join(root, "index.js")).href);
  await runtime.initialize();
  return runtime;
}

export type RuntimeCase = {
  kind?: "replay";
  document: string;
  expected?: string;
  /** Exact expected issues for anomalous stored state. */
  issues?: string;
  scenario?: string;
  phase?: string;
};
/** One collaboration step: import the other peer's history, edit, save, reopen, export. */
export type PeerCase = {
  kind: "peer";
  document: string;
  imports: string[];
  operations: unknown[];
  exportTo: string;
};
export type RuntimeResult = { state: unknown; stateHash: string; issues?: unknown };
const hash = (value: unknown) => createHash("sha256").update(canonical(value)).digest("hex");
async function openDocument(runtime: any, root: string) {
  const descriptor = await Bun.file(join(root, "state.schema.json")).json();
  const initial = await Bun.file(join(root, "initial.json")).json();
  const definition = runtime.fromDescriptor(descriptor);
  return () => SQLiteStore.open(root).then((store) => runtime.Document.open(definition, store, initial));
}

export async function runPeer(runtime: any, input: PeerCase): Promise<RuntimeResult> {
  const root = resolve(input.document);
  try {
    const open = await openDocument(runtime, root);
    let doc = await open();
    try {
      for (const path of input.imports) doc.importUpdates(await Bun.file(path).bytes());
      if (input.operations.length) doc.applyAll(input.operations);
      await doc.flush();
      await Bun.write(input.exportTo, doc.exportUpdates());
      const state = doc.current;
      const issues = doc.issues;
      await doc.close();
      doc = await open();
      assert.deepEqual(doc.current, state);
      return { state, stateHash: hash({ state, issues }), issues };
    } finally {
      await doc.close();
    }
  } catch (error) {
    throw new Error(`Peer ${input.document}: ${error}`, { cause: error });
  }
}

export async function runCase(runtime: any, input: RuntimeCase): Promise<RuntimeResult> {
  const { document, expected: expectedPath, scenario: scenarioPath, phase = "read" } = input;
  const root = resolve(document);
  try {
    const open = await openDocument(runtime, root);
    let doc = await open();
    try {
      if (expectedPath && expectedPath !== "-")
        assert.deepEqual(doc.current, await Bun.file(expectedPath).json());
      if (input.issues) assert.deepEqual(doc.issues, await Bun.file(input.issues).json());
      if (scenarioPath && scenarioPath !== "-") {
        const scenario = await Bun.file(scenarioPath).json();
        for (const edit of scenario.handles ?? []) {
          let handle = doc.fields;
          for (const field of edit.path) handle = handle[field];
          handle[edit.method](...edit.args);
        }
        if (scenario.operations?.length) doc.applyAll(scenario.operations);
        assert.deepEqual(doc.current, scenario.expected);
      }
      if (phase === "checkpoint") await doc.compact();
      else await doc.flush();
      const expected = doc.current;
      await doc.close();
      doc = await open();
      assert.deepEqual(doc.current, expected);
      if (input.issues) assert.deepEqual(doc.issues, await Bun.file(input.issues).json());
      return { state: doc.current, stateHash: hash(doc.current) };
    } finally {
      await doc.close();
    }
  } catch (error) {
    throw new Error(`Document ${document}, ${phase}: ${error}`, { cause: error });
  }
}

if (import.meta.main) {
  const runtimePath = process.argv[2];
  assert(runtimePath, "runtime path is required");
  const cases: (RuntimeCase | PeerCase)[] = await Bun.stdin.json();
  const runtime = await loadRuntime(runtimePath);
  const results: RuntimeResult[] = [];
  for (const input of cases) {
    // A stalled case must not hang an entire historical batch.
    const timeout = setTimeout(() => {
      console.error(`Document ${input.document}, ${input.kind === "peer" ? "peer" : (input.phase ?? "read")}: timed out`);
      process.exit(1);
    }, 30_000);
    try {
      results.push(input.kind === "peer" ? await runPeer(runtime, input) : await runCase(runtime, input));
    } finally {
      clearTimeout(timeout);
    }
  }
  console.log(JSON.stringify(results));
}
