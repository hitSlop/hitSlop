// Captures a compatibility corpus entry from this build: the conformance app (every ctx
// member and descriptor kind), the conformance fixtures and a few shipped templates, as
// built; documents saved through this build's CLI and helper; what they read as; edits to
// replay on them; and the commands a CLI of this release runs, with what they print.
// Usage: bun run compat:capture RELEASE [--frozen] [--templates slug,slug]
// The templates default to the shipped ones (`examples/slops/bundled.json`).
// Before launch, `dev` is replaceable. A frozen entry is permanent: capture it from the
// release candidate with a clean tree, then commit it before tagging.
import { Database } from "bun:sqlite";
import { copyFile, mkdir, mkdtemp, readFile, readdir, realpath, rename, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { HelperProtocol, PackageFormat, RuntimeABI } from "../../packages/schema/src/constants";
import {
  corpus,
  helper,
  slop,
  slopJSON,
  readJSON,
  savedState,
  stable,
  type Expected,
  type Page,
  type Release,
  type Scenario,
  type Transcript,
} from "./corpus";
import { prepareNativeFixtures } from "../lib/native-fixtures";
import { builtTemplates } from "../templates/discover";
import { archiveDigest, corpusFiles, sourceFingerprint, verifyCorpus } from "./integrity";
import { appAsset, digest, fileDigest, sha256, shellDestinations, shellFiles, useTestRegistry, repository } from "../lib/artifacts";
import { engine, pack } from "../../packages/cli/src/engine";
import { coreBuildId } from "../../packages/cli/src/core";
import { exec } from "../../packages/cli/src/process";
import { createDocument, debugHelper } from "../lib/helper";
useTestRegistry();

const [name, ...flags] = process.argv.slice(2);
if (!name || !/^[a-z0-9][a-z0-9.-]*$/.test(name)) throw new Error("Usage: bun run compat:capture RELEASE [--frozen]");
const frozen = flags.includes("--frozen");
if (helper !== debugHelper)
  throw new Error("Capture uses the helper it builds; remove HITSLOP_NATIVE_CLI for capture");
const chosen = flags.includes("--templates")
  ? flags[flags.indexOf("--templates") + 1]!.split(",")
  : (JSON.parse(await readFile(join(repository, "examples/slops/bundled.json"), "utf8")) as string[]);
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
await run([process.execPath, "run", "build:templates"]);
if ((await coreBuildId()) !== await run([helper, "--core-build"])) throw new Error("Capture helper and authoring core differ");
const capturedInputs = await sourceFingerprint();
const stage = await mkdtemp(join(tmpdir(), "hitslop-corpus-stage-"));
// Resolved, as the CLI prints the paths it writes (/var is a link on macOS).
const work = await realpath(await mkdtemp(join(tmpdir(), "hitslop-compat-capture-")));
const root = join(stage, name);
try {
// Templates: the chosen shipped templates as built, the hand-written conformance fixtures
// (packed from their stages) and the Svelte conformance app, which exercises every ctx
// member and descriptor kind.
await prepareNativeFixtures();
const { templates: built } = await builtTemplates();
for (const directory of ["templates", "documents", "expected", "scenarios", "pages", "cli"])
  await mkdir(join(root, directory), { recursive: true });
const templates: Record<string, string> = {};
for (const slug of chosen) {
  if (!built.some((t) => t.slug === slug && t.bundled)) throw new Error(`Not a shipped template: ${slug}`);
  templates[slug] = join(repository, "generated/templates", slug + ".slop");
}
for (const fixture of await readdir(join(repository, "tests/fixtures")))
  templates[`fixture-${fixture}`] = join(repository, "tests/fixtures", fixture, "document");
templates.conformance = join(repository, "generated/abi/owner-svelte.slop");
for (const [slug, source] of Object.entries(templates)) {
  const template = join(root, "templates", slug + ".slop");
  if (source.endsWith(".slop")) await copyFile(source, template);
  else await pack(source, template);
}

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
const schemaOf = async (document: string) => JSON.parse(await engine(["schema", document])) as Node;
/** The app's module, read from the file outside the core. */
const appOf = (document: string) => appAsset(document, "app.js");
const valueOf = async (document: string) => (await slopJSON(["get", document])) as unknown;
const batch = (document: string, ops: unknown[]) => slopJSON(["batch", document, "--ops", JSON.stringify(ops)]);

// Documents: each package's document after two closed editing sessions (agent edits,
// counter contributions from two writers, checkpoint plus saved updates) and a theme
// change; the conformance document also holds an attachment.
const documents = join(root, "documents");
const pageScripts: Record<string, Page["script"]> = {};
const actions: Record<string, NonNullable<Page["actions"]>> = {
  "fixture-scalars": [{ selector: 'input[type="range"]', value: "0.8" }, { selector: "select", value: "CAD" }],
  "fixture-collections": [{ selector: "textarea", value: "Collection edit ✓" }],
  "quick-checklist": [{ selector: '[aria-label="Checklist title"]', value: "Checklist edited ✓" }],
};
for (const slug of Object.keys(templates)) {
  const document = join(documents, slug + ".slop");
  await createDocument(join(root, "templates", slug + ".slop"), document, { helper });
  // What a new document of this release's template holds: its initial checkpoint.
  await writeFile(join(root, "expected", `new-${slug}.json`), JSON.stringify(await valueOf(document), null, 2) + "\n");
  const schema = await schemaOf(document);
  for (const round of [1, 2]) await batch(document, edits(schema, await valueOf(document), round));
  const theme = await slopJSON(["theme", "get", document]);
  const [token, color] = Object.entries(theme.defaults as Record<string, string>)[0] ?? [];
  if (token) await slopJSON(["theme", "set", document, "--values", JSON.stringify({ [token]: color === "#123456" ? "#654321" : "#123456" })]);
  if (slug === "conformance") {
    const file = join(work, "attachment.txt");
    await writeFile(file, "Compatibility corpus attachment ✓\n");
    const ref = await slopJSON(["attachments", "ref", file]);
    await slopJSON(["apply", document, "--attach", file, "--op", JSON.stringify({ type: "set", path: ["attachment"], value: ref.id })]);
    await rm(file);
  }
  const app = appOf(document);
  pageScripts[slug] = app.includes("contractTest") ? "contractTest" : "actions";
}
// Storage shapes: a compacted (history-trimmed) checkpoint.
for (const slug of ["conformance", "quick-checklist"]) {
  const compacted = join(documents, `${slug}-compacted.slop`);
  await copyFile(join(documents, slug + ".slop"), compacted);
  await run(["cargo", "run", "-q", "--locked", "-p", "hitslop-core", "--features", "storage", "--example", "compat_checkpoint", "--", compacted]);
  pageScripts[`${slug}-compacted`] = pageScripts[slug]!;
}

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
  const { state } = await slopJSON(["get", scratch, "--snapshot"]);
  const scenario: Scenario = { ops, value: state.value };
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
  const script = pageScripts[document]!;
  const selected = actions[document.replace(/-compacted$/, "")];
  if (script === "actions" && !selected) throw new Error(`Add an explicit page action for ${document} before capture`);
  const page: Page = { script, ...(selected ? { actions: selected } : {}), value: null };
  await writeFile(join(root, "pages", document + ".json"), JSON.stringify(page, null, 2) + "\n");
}

// The commands a CLI of this release runs, and what they print.
const attachmentBytes = "Archived CLI attachment ✓\n";
await writeFile(join(root, "cli/attachment.txt"), attachmentBytes);
const attachmentID = sha256(attachmentBytes);
const transcript: Transcript = { document: "conformance", commands: [] };
await rm(scratch, { recursive: true, force: true });
await copyFile(join(documents, "conformance.slop"), scratch);
for (const args of [
  ["get", "{document}"],
  ["get", "{document}", "--snapshot"],
  ["apply", "{document}", "--op", JSON.stringify({ type: "set", path: ["label"], value: "From the CLI" })],
  ["batch", "{document}", "--ops", JSON.stringify([{ type: "increment", path: ["hits"], by: 4 }, { type: "insert", path: ["colors"], value: "#abcdef" }])],
  ["apply", "{document}", "--op", JSON.stringify({ type: "set", path: ["count"], value: 1000 })],
  ["theme", "get", "{document}"],
  ["theme", "set", "{document}", "--values", JSON.stringify({ accent: "#204060" })],
  ["attachments", "ref", "{attachment}"],
  ["apply", "{document}", "--attach", "{attachment}", "--op", JSON.stringify({ type: "set", path: ["attachment"], value: attachmentID })],
  ["attachments", "export", "{document}", attachmentID, "--output", "{output}"],
  ["attachments", "list", "{document}"],
  ["get", "{document}"],
]) {
  const exported = join(work, "attachment-export.txt");
  const result = await slop(args.map((arg) => arg === "{document}" ? scratch : arg === "{attachment}" ? join(root, "cli/attachment.txt") : arg === "{output}" ? exported : arg));
  let stdout: unknown = result.stdout.trim();
  try {
    stdout = stable(JSON.parse(result.stdout), args);
  } catch {}
  if (args.includes("{output}")) {
    stdout = String(stdout).replaceAll(exported, "{output}");
    transcript.commands.push({ args, code: result.code, stdout, outputHash: await fileDigest(exported) });
  } else transcript.commands.push({ args, code: result.code, stdout });
}
await writeFile(join(root, "cli/transcript.json"), JSON.stringify(transcript, null, 2) + "\n");
// The npm packages of this release, which the release gate installs and runs.
await run([process.execPath, "scripts/build/packages.ts"]);
const archives: Record<string, string> = {};
const npmPackages: Record<string, string> = {};
for (const pkg of ["schema", "document", "cli"]) {
  const metadata = JSON.parse(await readFile(join(repository, "packages", pkg, "package.json"), "utf8"));
  const file = `hitslop-${pkg}-${metadata.version}.tgz`;
  await copyFile(join(repository, "generated/npm", file), join(root, "cli", file));
  archives[file] = await archiveDigest(join(root, "cli", file));
  npmPackages[metadata.name] = `../${file}`;
}
const install = join(root, "cli/install");
await mkdir(install);
await writeFile(join(install, "package.json"), JSON.stringify({ private: true, dependencies: { "@hitslop/cli": npmPackages["@hitslop/cli"] }, overrides: npmPackages }));
await run([process.execPath, "install"], install);
await rm(join(install, "node_modules"), { recursive: true, force: true });

// Storage shapes the documents cover.
const storage: Release["storage"] = {};
function measure(document: string) {
  const database = new Database(join(documents, document + ".slop"), { readonly: true });
  const checkpoint = database.query("SELECT length(bytes) AS n FROM checkpoint").get() as { n: number };
  const updates = database.query("SELECT count(*) AS n FROM updates").get() as { n: number };
  database.close();
  storage[document] = { checkpointBytes: checkpoint.n, updates: updates.n };
}
for (const document of names) measure(document);
if (!Object.values(storage).some(({ updates }) => updates > 0)) throw new Error("No document keeps saved updates past its checkpoint");

const version = async (command: string[]) => (await run(command)).split("\n")[0]!;
const lock = await readFile(join(repository, "Cargo.lock"), "utf8");
const release: Release = {
  release: name,
  frozen: false,
  commit: (await run(["git", "rev-parse", "HEAD"])) + (dirty ? "-dirty" : ""),
  captured: new Date().toISOString(),
  markers: {
    packageFormat: PackageFormat,
    runtimeABI: RuntimeABI,
    storage: Number((await readFile(join(repository, "crates/hitslop-core/src/file.rs"), "utf8")).match(/const STORAGE_VERSION: i64 = (\d+);/)![1]),
    layout: Number((await readFile(join(repository, "crates/hitslop-core/src/lib.rs"), "utf8")).match(/pub const LAYOUT: i64 = (\d+);/)![1]),
    protocol: HelperProtocol.version,
  },
  inputs: capturedInputs,
  producer: { coreBuildID: await run([helper, "--core-build"]), shell: await digest(shellDestinations.app, shellFiles) },
  files: {},
  templates: Object.fromEntries(await Promise.all(Object.keys(templates).map(async slug => [slug, await fileDigest(join(root, "templates", slug + ".slop"))]))),
  archives,
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
