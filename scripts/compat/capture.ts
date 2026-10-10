// Captures a compatibility corpus entry from this build: the runtime, not individual slops.
// The conformance app (every ctx member, descriptor kind and a stored command) as built,
// with documents saved through this build's CLI and helper and by its own page, what they
// read as and edits to replay on them; a template for each window kind; and the engine
// that wrote them (the candidate writer).
// Usage: bun run compat:capture RELEASE [--frozen]
// Before launch, `dev` is replaceable. A frozen entry is permanent: capture it from the
// release candidate with a clean tree, then commit it before tagging.
import { Database } from "bun:sqlite";
import { copyFile, mkdir, mkdtemp, readFile, readdir, realpath, rename, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { HelperProtocol, PackageFormat, RuntimeABI } from "../../packages/hitslop/src/schema/constants";
import {
  corpus,
  helper,
  documentEngine,
  slop,
  slopJSON,
  readJSON,
  savedState,
  type Expected,
  type Page,
  type Release,
  type Scenario,
} from "./corpus";
import { prepareNativeFixtures } from "../lib/native-fixtures";
import { corpusFiles, sourceFingerprint, verifyCorpus } from "./integrity";
import { digest, fileDigest, sha256, shellDestinations, shellFiles, useTestRegistry, repository } from "../lib/artifacts";
import { execute } from "../../packages/hitslop/src/cli/engine";
import { exec } from "../../packages/hitslop/src/cli/process";
import { createDocument, debugHelper } from "../lib/native";
import { evaluateStored } from "./commands";
// The producing tools are built as they ship (the `dist` Cargo profile), as the release
// gate builds them: a candidate's packages then match what was captured.
process.env.HITSLOP_CARGO_PROFILE = "dist";
useTestRegistry();

const [name, ...flags] = process.argv.slice(2);
if (!name || !/^[a-z0-9][a-z0-9.-]*$/.test(name)) throw new Error("Usage: bun run compat:capture RELEASE [--frozen]");
const frozen = flags.includes("--frozen");
if (helper !== debugHelper)
  throw new Error("Capture uses the helper it builds; remove HITSLOP_NATIVE_CLI for capture");
const destination = join(corpus, name);
const previous = await readJSON<Release>(join(destination, "release.json"));
if (previous?.frozen) throw new Error(`tests/compat/${name} is frozen; it is never recaptured`);
const run = async (command: string[], cwd = repository) => {
  if (command[0] === process.execPath) console.log(`Capture: ${command.slice(1).join(" ")}`);
  const { stdout, stderr, code } = await exec(command, { cwd });
  if (code) throw new Error(`${command.join(" ")} failed: ${stdout.trim()}\n${stderr.trim()}`);
  return stdout.trim();
};
const dirty = (await run(["git", "status", "--porcelain"])) !== "";
if (frozen && dirty) throw new Error("Capture a frozen entry from a clean release candidate");

// Build the producing tools rather than trusting an existing helper or inventory.
await run([process.execPath, "run", "build"]);
if (await run([documentEngine(), "--build-id"]) !== await run([helper, "--core-build"])) throw new Error("Capture helper and authoring core differ");
const capturedInputs = await sourceFingerprint();
const stage = await mkdtemp(join(tmpdir(), "hitslop-corpus-stage-"));
// Resolved, as the CLI prints the paths it writes (/var is a link on macOS).
const work = await realpath(await mkdtemp(join(tmpdir(), "hitslop-compat-capture-")));
const root = join(stage, name);
try {
// Templates: the Svelte conformance app, which exercises every ctx member and descriptor
// kind, and one per window kind (1× and 2× PNG skins, glass, a transparent ellipse and a
// path shape). A window lives in the stored app, so its template is enough: later builds
// create, edit and render a document from it.
const presentation = await prepareNativeFixtures();
for (const directory of ["templates", "documents", "expected", "scenarios", "pages", "commands", "engine/darwin-arm64"])
  await mkdir(join(root, directory), { recursive: true });
const templates: Record<string, string> = { conformance: join(repository, "generated/abi/owner-svelte.slop") };
for (const kind of ["washer", "washer-2x", "glass", "ellipse", "notch"]) templates[`presentation-${kind}`] = presentation[kind]!;
for (const [slug, source] of Object.entries(templates)) await copyFile(source, join(root, "templates", slug + ".slop"));

// Generic edits derived from a descriptor: one valid write of every kind it declares.
type Node = { kind: string; [key: string]: any };
const clamp = (node: Node, x: number) => Math.min(node.max ?? Infinity, Math.max(node.min ?? -Infinity, x));
function sample(node: Node, round: number): unknown {
  switch (node.kind) {
    case "text": return round % 2 ? `Compat ${round} ✓ café 🦊` : `Replay ${round} é 中文`;
    case "boolean": return round % 2 === 1;
    case "string": return `compat ${round}`.slice(0, node.maxLength ?? 64);
    case "number": return clamp(node, round / 4);
    case "integer": return Math.round(clamp(node, round));
    case "enum": return node.values[round % node.values.length];
    case "counter": return 0;
    case "optional": return sample(node.inner, round);
    case "object":
      return Object.fromEntries(
        Object.entries(node.properties as Record<string, Node>)
          .filter(([, child]) => child.kind !== "optional")
          .map(([key, child]) => [key, sample(child, round)]),
      );
    case "list": return [];
    case "record": return {};
  }
  throw new Error(`Unknown kind ${node.kind}`);
}
const scalar = (node: Node) => ["boolean", "string", "number", "integer", "enum"].includes(node.kind);
function edits(node: Node, value: any, round: number, path: unknown[] = []): unknown[] {
  const ops: unknown[] = [];
  for (const [key, child] of Object.entries(node.properties as Record<string, Node>).sort(([a], [b]) => a.localeCompare(b))) {
    const at = [...path, key];
    const current = value?.[key];
    if (child.kind === "text" || scalar(child)) ops.push({ type: "set", path: at, value: sample(child, round) });
    else if (child.kind === "counter") ops.push({ type: "increment", path: at, by: round + 1 });
    else if (child.kind === "optional") {
      if (child.inner.kind !== "object") ops.push({ type: "set", path: at, value: sample(child.inner, round) });
      else if (current === undefined) ops.push({ type: "set", path: at, value: sample(child.inner, round) });
    } else if (child.kind === "object") ops.push(...edits(child, current, round, at));
    else if (child.kind === "list" && child.item.kind === "object") {
      ops.push({ type: "insert", path: at, id: `compat-${round}`, value: sample(child.item, round) });
      const rows = (current ?? []) as { $id: string }[];
      if (rows.length >= 2) ops.push({ type: "move", path: at, id: rows[0]!.$id, at: { after: rows.at(-1)!.$id } });
    } else if (child.kind === "list") ops.push({ type: "insert", path: at, value: sample(child.item, round), index: 0 });
    else if (child.kind === "record") ops.push({ type: "set", path: [...at, `compat-${round}`], value: sample(child.value, round) });
  }
  return ops;
}
const schemaOf = async (document: string) => (await execute({ method: "schema", file: document })).schema as Node;
const valueOf = async (document: string) => (await slopJSON(["get", document])) as unknown;
const batch = (document: string, ops: unknown[]) => slopJSON(["batch", document, "--ops", JSON.stringify(ops)]);

// What a new document of each template holds: its initial snapshot.
for (const slug of Object.keys(templates)) {
  const created = join(work, `new-${slug}.slop`);
  await createDocument(join(root, "templates", slug + ".slop"), created, { engine: documentEngine() });
  await writeFile(join(root, "expected", `new-${slug}.json`), JSON.stringify(await valueOf(created), null, 2) + "\n");
  await rm(created);
}
// The conformance document after two closed editing sessions (agent edits, counter
// increments from two writers, a snapshot plus saved updates), a theme change and two
// attachments; and a compacted (history-trimmed) copy.
const documents = join(root, "documents");
const conformance = join(documents, "conformance.slop");
await createDocument(join(root, "templates", "conformance.slop"), conformance, { engine: documentEngine() });
const schema = await schemaOf(conformance);
for (const round of [1, 2]) await batch(conformance, edits(schema, await valueOf(conformance), round));
const theme = await slopJSON(["theme", "get", conformance]);
const [token, color] = Object.entries(theme.defaults as Record<string, string>)[0] ?? [];
if (token) await slopJSON(["theme", "set", conformance, "--values", JSON.stringify({ [token]: color === "#123456" ? "#654321" : "#123456" })]);
const text = join(work, "attachment.txt");
await writeFile(text, "Compatibility corpus attachment ✓\n");
const ref = await slopJSON(["attachments", "ref", text]);
await slopJSON(["apply", conformance, "--attach", text, "--op", JSON.stringify({ type: "set", path: ["attachment"], value: ref.id })]);
await rm(text);
// A second attachment of a media type the host sniffs from its bytes.
const image = join(work, "photo.png");
await copyFile(join(repository, "tests/abi/owner-svelte/media/swatch.png"), image);
const photo = await slopJSON(["attachments", "ref", image]);
await slopJSON(["apply", conformance, "--attach", image, "--op", JSON.stringify({ type: "set", path: ["photo"], value: photo.id })]);
await rm(image);
const compacted = join(documents, "conformance-compacted.slop");
await copyFile(conformance, compacted);
await run(["cargo", "run", "-q", "--locked", "-p", "hitslop-core", "--features", "storage", "--example", "compat_checkpoint", "--", compacted]);

// What each document reads as, and an edit to replay on it with its result.
const names = (await readdir(documents)).filter((n) => n.endsWith(".slop")).map((n) => n.slice(0, -5)).sort();
const scratch = join(work, "Document.slop");
async function record(document: string) {
  const path = join(documents, document + ".slop");
  const expected: Expected = await savedState(path);
  await writeFile(join(root, "expected", document + ".json"), JSON.stringify(expected, null, 2) + "\n");
  const ops = edits(await schemaOf(path), expected.value, 3);
  await rm(scratch, { recursive: true, force: true });
  await copyFile(path, scratch);
  await batch(scratch, ops);
  const { value } = await slopJSON(["get", scratch, "--snapshot"]);
  const scenario: Scenario = { ops, value };
  await writeFile(join(root, "scenarios", document + ".json"), JSON.stringify(scenario, null, 2) + "\n");
  // The old app must render its saved document.
  for (const format of ["png", "pdf"]) {
    const output = join(work, `render.${format}`);
    await slop(["export", path, "--format", format, "--output", output]).then(({ code, stderr }) => { if (code) throw new Error(stderr); });
    await rm(output);
  }
}
for (const document of names) {
  await record(document);
  // The conformance app's own contract test is its page scenario.
  const page: Page = { script: "contractTest", value: null };
  await writeFile(join(root, "pages", document + ".json"), JSON.stringify(page, null, 2) + "\n");
}

// The candidate writer: the engine that wrote these documents, which later builds run to
// write more (`crates/hitslop-core/tests/compat_writers.rs`). Not the shipped binary: the
// release replays the corpus with the engine in its final tarball as well.
const writerPath = join(root, "engine/darwin-arm64/slop-engine");
await copyFile(documentEngine(), writerPath);
const writer = { buildId: await run([writerPath, "--build-id"]), commit: "", sha256: await fileDigest(writerPath) };

// Storage shapes the documents cover.
const storage: Release["storage"] = {};
function measure(document: string) {
  const database = new Database(join(documents, document + ".slop"), { readonly: true });
  // The history's first row is its snapshot; the rest are updates saved after it.
  const checkpoint = database.query("SELECT length(bytes) AS n FROM history ORDER BY seq LIMIT 1").get() as { n: number };
  const history = database.query("SELECT count(*) AS n FROM history").get() as { n: number };
  database.close();
  storage[document] = { checkpointBytes: checkpoint.n, updates: history.n - 1 };
}
for (const document of names) measure(document);
if (!Object.values(storage).some(({ updates }) => updates > 0)) throw new Error("No document keeps saved updates past its checkpoint");

const version = async (command: string[]) => (await run(command)).split("\n")[0]!;
const lock = await readFile(join(repository, "Cargo.lock"), "utf8");
const commit = (await run(["git", "rev-parse", "HEAD"])) + (dirty ? "-dirty" : "");
writer.commit = commit;
const release: Release = {
  release: name,
  frozen: false,
  commit,
  captured: new Date().toISOString(),
  markers: {
    packageFormat: PackageFormat,
    runtimeABI: RuntimeABI,
    storage: Number((await readFile(join(repository, "crates/hitslop-core/src/file/mod.rs"), "utf8")).match(/const STORAGE_VERSION: i64 = (\d+);/)![1]),
    layout: Number((await readFile(join(repository, "crates/hitslop-core/src/document/mod.rs"), "utf8")).match(/pub const LAYOUT: i64 = (\d+);/)![1]),
    protocol: HelperProtocol.version,
  },
  inputs: capturedInputs,
  producer: {
    coreBuildID: await run([helper, "--core-build"]),
    shell: await digest(shellDestinations.app, shellFiles),
    // The command prelude this release ran; a frozen entry freezes it (scripts/build/runner.ts).
    runner: { [RuntimeABI]: await fileDigest(join(repository, `crates/hitslop-runner/src/abi/${RuntimeABI}.generated.js`)) },
  },
  files: {},
  acceptance: Object.fromEntries(await Promise.all([
    `crates/hitslop-core/src/app/package_format_${PackageFormat}.rs`,
    "crates/hitslop-core/src/file/storage-1.sql",
  ].map(async path => [path, await fileDigest(join(repository, path))]))),
  templates: Object.fromEntries(await Promise.all(Object.keys(templates).map(async slug => [slug, await fileDigest(join(root, "templates", slug + ".slop"))]))),
  writer,
  toolchain: {
    rust: await version(["rustc", "--version"]),
    bun: Bun.version,
    swift: await version(["swift", "--version"]),
    macos: await version(["sw_vers", "-productVersion"]),
    loro: lock.match(/name = "loro"\nversion = "([^"]+)"/)![1]!,
    svelte: JSON.parse(await readFile(join(repository, "node_modules/svelte/package.json"), "utf8")).version,
  },
  // Noon UTC is the same date in nearly every time zone a later run may use.
  clock: (() => {
    const today = new Date();
    return Date.UTC(today.getUTCFullYear(), today.getUTCMonth(), today.getUTCDate(), 12);
  })(),
  storage,
};
await writeFile(join(root, "release.json"), JSON.stringify(release, null, 2) + "\n");

// A stored command program, replayed by later builds with this clock and seed.
const commandProbes = { conformance: { name: "bump", args: { by: 2 } } };
for (const [slug, probe] of Object.entries(commandProbes)) {
  const original = join(documents, slug + ".slop"), copy = join(work, "command.slop");
  await rm(copy, { force: true });
  await copyFile(original, copy);
  const input = { ...probe, now: release.clock, seed: [1, 2, 3, 4] };
  const state = await slopJSON(["get", copy, "--snapshot"]);
  const evaluated = await evaluateStored(copy, state, input);
  await batch(copy, evaluated.intents);
  const value = await valueOf(copy);
  await writeFile(join(root, "commands", slug + ".json"), JSON.stringify({ ...input, evaluated, value }, null, 2) + "\n");
}

// The old apps' own edits: the native corpus test records what each page scenario saves.
const swift = await exec([process.execPath, "scripts/verify.ts", "--no-build", "swift", "--filter", "CompatCorpusTests"], {
  cwd: repository,
  inherit: ["stdout", "stderr"],
  env: { ...process.env, HITSLOP_COMPAT_RECORD: name, HITSLOP_COMPAT_ROOT: stage, TZ: "UTC" },
});
if (swift.code) throw new Error("Recording page scenarios failed");
for (const document of await readdir(join(root, "pages"))) {
  const page = JSON.parse(await readFile(join(root, "pages", document), "utf8")) as Page;
  if (page.value === null) throw new Error(`No page result recorded for ${document}`);
}
// The documents the old apps' pages saved, recorded like the CLI-written ones.
for (const document of names) {
  const saved = `${document}.page`;
  if (!(await Bun.file(join(documents, saved + ".slop")).exists())) throw new Error(`No page-saved document for ${document}`);
  await record(saved);
  measure(saved);
}
if (await sourceFingerprint() !== capturedInputs) throw new Error("Producing inputs changed during capture");
release.files = await corpusFiles(root);
release.frozen = frozen;
await verifyCorpus(root, release);
await writeFile(join(root, "release.json"), JSON.stringify(release, null, 2) + "\n");
await mkdir(corpus, { recursive: true });
await rm(destination, { recursive: true, force: true });
await rename(root, destination);
console.log(`Captured ${resolve(destination)}`);
} finally {
  await rm(stage, { recursive: true, force: true });
  await rm(work, { recursive: true, force: true });
}
