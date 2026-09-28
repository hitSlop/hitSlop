// One-shot authoring of the launch baseline fixtures for the current runtime contract.
// Checks and tests never run this. Sealed fixtures are immutable: this refuses to
// replace an existing fixture directory. New capabilities get new fixtures.
//
//   bun scripts/v1/author-fixtures.ts
//
// Expected values asserted here are written independently of the engine; the files it
// writes (expected.json, issues.json, scenario.json) become the frozen replay oracles.
import { strict as assert } from "node:assert";
import { createHash } from "node:crypto";
import { cp, mkdir, mkdtemp, readFile, rm, stat, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { LoroDoc, LoroMap, LoroText, type LoroCounter, type LoroMovableList } from "loro-crdt";
import identity from "../../packages/document/src/runtime-identity.json";
import { Document } from "../../packages/document/src/document";
import { fromDescriptor } from "../../packages/document/src/schema";
import { runPeer } from "./compatibility-worker";
import { SQLiteStore } from "../../packages/document/test-support/sqlite";
import { buildProject } from "../../packages/cli/src/build";
import { conformance, initial } from "../../tests/abi/fixture-schema";
import { digest, repository } from "./runtime-artifacts";

const corpus = join(repository, "tests/compatibility");
const prefix = `${identity.runtimeContract}-${identity.runtimeRevision}`;
const json = (value: unknown) => JSON.stringify(value, null, 2) + "\n";
const exists = (path: string) => stat(path).then(() => true, () => false);
/** A mutable deep copy of a frozen snapshot. */
const clone = <T>(value: T): any => JSON.parse(JSON.stringify(value));

/** The hand-written plain consumer app over the shared conformance schema. */
async function plainPackage(root: string, slug: string, title: string) {
  await mkdir(join(root, "assets"), { recursive: true });
  const manifest = {
    $schema: "https://api.hitslop.com/schemas/v1/manifest.schema.json",
    runtime: "hitslop-v1",
    author: { name: "hitSlop" },
    slug,
    title,
    description: "Preserved runtime contract fixture.",
    categories: ["utilities"],
    presentation: { width: 480, height: 480 },
  };
  const { runtimeRevision, ...provenance } = identity;
  const files: Record<string, string> = {
    "manifest.json": json(manifest),
    "state.schema.json": json(conformance.descriptor),
    "initial.json": json(initial),
    "assets/app.js": await readFile(join(repository, "tests/abi/plain/app.js"), "utf8"),
    "assets/app.css": "",
    "assets/theme.json": JSON.stringify({ accent: "#335577" }),
    "assets/runtime.json": JSON.stringify({ ...provenance, minRuntimeRevision: runtimeRevision }),
  };
  for (const [path, text] of Object.entries(files)) await writeFile(join(root, path), text);
}
async function load(root: string) {
  const store = await SQLiteStore.open(root);
  try {
    return await store.load();
  } finally {
    await store.close();
  }
}
const open = async (root: string) =>
  Document.open(conformance, await SQLiteStore.open(root), initial);

/** Run a collaboration script through the replay worker's peer path with this source engine. */
async function verifyCollaboration(document: string, script: { steps: any[]; expected: Record<string, unknown> }) {
  const directory = await mkdtemp(join(tmpdir(), "hitslop-collaboration-"));
  try {
    const runtime = { Document, fromDescriptor };
    const peers = {
      a: { document: join(directory, "a.slop"), exports: join(directory, "a.bin") },
      b: { document: join(directory, "b.slop"), exports: join(directory, "b.bin") },
    };
    for (const peer of Object.values(peers)) {
      await cp(document, peer.document, { recursive: true });
      await rm(join(peer.document, "state/writer.lock"), { force: true });
    }
    const run = async (side: "a" | "b", operations: unknown[], sync = true) => {
      const other = peers[side === "a" ? "b" : "a"];
      return runPeer(runtime, {
        kind: "peer",
        document: peers[side].document,
        imports: sync && (await exists(other.exports)) ? [other.exports] : [],
        operations,
        exportTo: peers[side].exports,
      });
    };
    for (const step of script.steps) await run(step.peer, step.operations, step.sync ?? true);
    const a = await run("a", []);
    const b = await run("b", []);
    assert.equal(a.stateHash, b.stateHash, "peers converge");
    for (const [key, value] of Object.entries(script.expected))
      assert.deepEqual((a.state as any)[key], value, `collaboration ${key}`);
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
}

async function create(name: string, build: (document: string, fixture: string) => Promise<void>, kind: string) {
  const fixture = join(corpus, name);
  if (await exists(fixture)) {
    console.log(`kept sealed ${name}`);
    return;
  }
  const document = join(fixture, "document");
  try {
    await mkdir(document, { recursive: true });
    await build(document, fixture);
    // Ownership residue is not document content.
    await rm(join(document, "state/writer.lock"), { force: true });
    await writeFile(
      join(fixture, "fixture.json"),
      json({
        kind,
        runtimeContract: identity.runtimeContract,
        runtimeRevision: identity.runtimeRevision,
        sha256: await digest(document),
      }),
    );
    console.log(`authored ${name}`);
  } catch (error) {
    await rm(fixture, { recursive: true, force: true });
    throw error;
  }
}

// Every value kind, a checkpoint plus uncheckpointed updates, a scripted scenario
// and a mixed-version collaboration script.
await create(prefix, async (document, fixture) => {
  await plainPackage(document, "runtime-conformance", "Runtime conformance");
  let doc = await open(document);
  const [alpha, beta] = doc.current.rows.map((row) => row.$id);
  const root = doc.current.outline[0]!.$id;
  doc.fields.title.replace("Stored title");
  doc.fields.notes.mark({ start: 0, end: 5 }, "bold", true);
  doc.fields.count.increment(4);
  doc.fields.tags.insert("two");
  doc.fields.rows.move(alpha!, { after: beta! });
  doc.fields.rows.item(beta!).name.replace("Beta ✓");
  doc.fields.cells.put("B2", "2");
  doc.fields.outline.insert({ label: "Leaf 2" }, { parent: root });
  doc.fields.cover.set({ caption: "Cover" });
  await doc.compact();
  doc.fields.level.set(9);
  doc.fields.mode.set("c");
  doc.fields.done.set(true);
  const gamma = doc.fields.rows.insert({ name: "Gamma", done: false }).id;
  await doc.close();
  const stored = await load(document);
  assert(stored.checkpoint && stored.updates.length >= 1, "checkpoint plus log");
  doc = await open(document);
  const state = clone(doc.current);
  await doc.close();
  assert.deepEqual(
    {
      title: state.title,
      bold: state.notes.delta[0],
      count: state.count,
      tags: state.tags,
      rows: state.rows.map((row: any) => [row.name, row.done]),
      cells: state.cells,
      leaves: state.outline[0]!.children.map((node: any) => node.label),
      cover: state.cover,
      scalars: [state.level, state.mode, state.done],
    },
    {
      title: "Stored title",
      bold: { insert: "Hello", attributes: { bold: true } },
      count: 5,
      tags: ["one", "two"],
      rows: [["Beta ✓", true], ["Alpha", false], ["Gamma", false]],
      cells: { A1: "1", B2: "2" },
      leaves: ["Leaf", "Leaf 2"],
      cover: { caption: "Cover" },
      scalars: [9, "c", true],
    },
  );
  await writeFile(join(fixture, "expected.json"), json(state));
  // Independently modeled scenario outcome.
  const expected = clone(state);
  expected.title = "Replayed";
  expected.count += 2;
  const moved = { ...expected.rows.at(-1)!, done: true };
  expected.rows = [moved, ...expected.rows.slice(0, -1)];
  await writeFile(
    join(fixture, "scenario.json"),
    json({
      handles: [
        { path: ["title"], method: "replace", args: ["Replayed"] },
        { path: ["count"], method: "increment", args: [2] },
      ],
      operations: [
        { type: "set", path: ["rows", { id: gamma }, "done"], value: true },
        { type: "move", path: ["rows"], id: gamma, destination: { before: beta } },
      ],
      expected,
    }),
  );
  // Concurrent offline edits with deterministic merges (never two writers on one
  // register), then synced edits. Both peers must converge on this independent model.
  const collaboration = {
    steps: [
      {
        peer: "a",
        sync: false,
        operations: [
          { type: "text.splice", path: ["title"], index: 0, delete: 0, insert: "A:" },
          { type: "increment", path: ["count"], value: 10 },
          { type: "move", path: ["rows"], id: alpha, destination: { after: gamma } },
          { type: "remove", path: ["rows"], id: beta },
          { type: "assign", path: ["cells", { key: "fromA" }], value: "a" },
        ],
      },
      {
        peer: "b",
        sync: false,
        operations: [
          { type: "text.splice", path: ["title"], index: state.title.length, delete: 0, insert: ":B" },
          { type: "increment", path: ["count"], value: 5 },
          { type: "text.replace", path: ["rows", { id: alpha }, "name"], value: "Alpha*" },
          { type: "set", path: ["rows", { id: beta }, "done"], value: false },
          { type: "assign", path: ["cells", { key: "fromB" }], value: "b" },
        ],
      },
      { peer: "a", operations: [{ type: "increment", path: ["count"], value: 1 }] },
      { peer: "b", operations: [{ type: "set", path: ["done"], value: false }] },
    ],
    expected: {
      title: `A:${state.title}:B`,
      count: state.count + 16,
      done: false,
      rows: [
        { $id: gamma, name: "Gamma", done: false },
        { $id: alpha, name: "Alpha*", done: false },
      ],
      cells: { ...state.cells, fromA: "a", fromB: "b" },
    },
  };
  await verifyCollaboration(document, collaboration);
  await writeFile(join(fixture, "collaboration.json"), json(collaboration));
}, "conformance");

// Theme overrides, a referenced attachment and several separately saved updates.
await create(`${prefix}-saved-state`, async (document, fixture) => {
  await plainPackage(document, "saved-state", "Saved state");
  const bytes = new TextEncoder().encode("preserved attachment bytes");
  const id = createHash("sha256").update(bytes).digest("hex");
  await mkdir(join(document, "state/attachments"), { recursive: true });
  await writeFile(join(document, "state/attachments", id), bytes);
  await writeFile(join(document, "state/theme.json"), JSON.stringify({ accent: "#aa3300" }));
  const doc = await open(document);
  doc.fields.cells.put("attachment", id);
  await doc.flush();
  for (const tag of ["x", "y", "z"]) {
    doc.fields.tags.insert(tag);
    await doc.flush();
  }
  await doc.close();
  const stored = await load(document);
  assert(stored.updates.length >= 4, "several uncheckpointed updates");
  const reopened = await open(document);
  assert.deepEqual([reopened.current.cells.attachment, reopened.current.tags], [id, ["one", "x", "y", "z"]]);
  await writeFile(join(fixture, "expected.json"), json(reopened.current));
  await reopened.close();
}, "saved-state");

// Merged anomalies: preserved, flagged with exact issues, never repaired.
await create(`${prefix}-issues`, async (document, fixture) => {
  await plainPackage(document, "merged-issues", "Merged issues");
  const created = await open(document);
  const [alpha] = created.current.rows.map((row) => row.$id);
  await created.close();
  const store = await SQLiteStore.open(document);
  const stored = await store.load();
  const peers = [new LoroDoc(), new LoroDoc()];
  for (const peer of peers) {
    peer.import(stored.checkpoint!);
    (peer.getMap("data").get("count") as LoroCounter).increment(1e308);
    peer.commit();
  }
  peers[0]!.import(peers[1]!.export({ mode: "update" }));
  const data = peers[0]!.getMap("data");
  data.set("cover", "not an object");
  data.set("mode", "zzz");
  data.set("extra", 1);
  const rows = data.get("rows") as LoroMovableList;
  for (const id of [undefined, alpha]) {
    const row = rows.insertContainer(rows.length, new LoroMap());
    if (id) row.set("$id", id);
    row.setContainer("name", new LoroText()).update(id ? "duplicate" : "missing");
    row.set("done", false);
  }
  peers[0]!.commit();
  await store.append(stored.generation, [peers[0]!.export({ mode: "update", from: peers[1]!.oplogVersion() }), peers[1]!.export({ mode: "update" })]);
  await store.close();
  const doc = await open(document);
  // Broken rows stay visible with derived IDs; exactly one row owns the duplicated ID.
  const byName = Object.fromEntries(doc.current.rows.map((row) => [row.name, row.$id]));
  assert.deepEqual(Object.keys(byName).sort(), ["Alpha", "Beta", "duplicate", "missing"]);
  assert.equal([byName.Alpha, byName.duplicate].filter((id) => id === alpha).length, 1);
  const loser = byName.Alpha === alpha ? byName.duplicate : byName.Alpha;
  for (const id of [byName.missing, loser]) assert.match(id!, /^x-[0-9a-z]{24}$/);
  const expectedIssues = [
    { path: [], kind: "unknown-field", detail: "Unknown stored field: extra" },
    { path: ["count"], kind: "invalid", detail: "Expected finite number" },
    { path: ["mode"], kind: "invalid", detail: "Unknown enum value" },
    { path: ["rows", { id: byName.missing }], kind: "identity", detail: "Missing row ID" },
    { path: ["rows", { id: loser }], kind: "identity", detail: "Duplicate row ID" },
    { path: ["cover"], kind: "invalid", detail: "Expected LoroMap" },
  ];
  const sort = (issues: readonly unknown[]) => [...issues].map((i) => JSON.stringify(i)).sort();
  assert.deepEqual(sort(doc.issues), sort(expectedIssues));
  assert.deepEqual([doc.current.count, doc.current.mode, doc.current.cover], [null, "a", undefined]);
  await writeFile(join(fixture, "expected.json"), json(doc.current));
  await writeFile(join(fixture, "issues.json"), json(doc.issues));
  await doc.close();
}, "issues");

// The Svelte adapter as compiled into apps; its sealed bytes pin the adapter's ABI use.
await create(`${prefix}-svelte`, async (document, fixture) => {
  const built = await buildProject(join(repository, "tests/abi/svelte"), join(fixture, "build.slop"));
  await cp(built, document, { recursive: true, force: true });
  await rm(built, { recursive: true, force: true });
  await rm(join(document, ".agents"), { recursive: true, force: true });
  const doc = await open(document);
  await writeFile(join(fixture, "expected.json"), json(doc.current));
  await doc.close();
}, "abi");
