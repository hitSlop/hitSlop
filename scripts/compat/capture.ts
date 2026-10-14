// Captures a compatibility corpus entry from this build: the runtime, not individual slops.
// The conformance app (every ctx member, descriptor kind and a stored command) as built,
// with documents saved through this build's CLI and by its own page, what they
// read as and edits to replay on them; a template for each window kind; and the engine
// that wrote them (the candidate writer).
// Usage: bun run compat:capture RELEASE [--frozen] [--writer PATH]
// Before launch, `dev` is replaceable. A frozen entry is permanent: capture it from the
// release candidate with a clean tree, then commit it before tagging.
import { Database } from "bun:sqlite";
import { strict as assert } from "node:assert";
import { createHash } from "node:crypto";
import { PNG } from "pngjs";
import {
  copyFile,
  mkdir,
  mkdtemp,
  readFile,
  readdir,
  realpath,
  rename,
  rm,
  writeFile,
} from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import {
  EngineProtocol,
  PackageFormat,
  RuntimeABI,
} from "../../packages/hitslop/src/schema/constants";
import {
  corpus,
  documentEngine,
  slop,
  slopJSON,
  readJSON,
  savedState,
  type Expected,
  type ExportGolden,
  type ThemeScenario,
  type CommandProbe,
  type Page,
  type Release,
  type Scenario,
} from "./corpus";
import { prepareNativeFixtures } from "../lib/native-fixtures";
import { corpusFiles, sourceFingerprint, verifyCorpus } from "./integrity";
import {
  digest,
  fileDigest,
  shellDestinations,
  shellFiles,
  useTestRegistry,
  repository,
} from "../lib/artifacts";
import { execute, request } from "../../packages/hitslop/src/cli/engine";
import { exec } from "../../packages/hitslop/src/cli/process";
import { createDocument } from "../lib/native";
import { evaluateStored } from "./commands";
import { resolveWriter } from "./writers";
// The producing tools are built as they ship (the `dist` Cargo profile), as the release
// gate builds them: a candidate's packages then match what was captured.
process.env.HITSLOP_CARGO_PROFILE = "dist";
useTestRegistry();

const [name, ...flags] = process.argv.slice(2);
if (!name || !/^[a-z0-9][a-z0-9.-]*$/.test(name))
  throw new Error("Usage: bun run compat:capture RELEASE [--frozen] [--writer PATH]");
const frozen = flags.includes("--frozen");
const writerIndex = flags.indexOf("--writer");
const suppliedWriter = writerIndex < 0 ? undefined : flags[writerIndex + 1];
if (writerIndex >= 0 && (!suppliedWriter || suppliedWriter.startsWith("--")))
  throw new Error("--writer requires an engine path");
if (
  flags.some(
    (flag, index) =>
      flag !== "--frozen" &&
      flag !== "--writer" &&
      !(writerIndex >= 0 && index === writerIndex + 1),
  )
)
  throw new Error("Unknown capture flag");
if (name !== "dev" && !suppliedWriter)
  throw new Error("Release capture requires --writer generated/engines/darwin-arm64/slop-engine");
if (process.env.HITSLOP_ENGINE)
  throw new Error("Capture uses the engine it builds; remove HITSLOP_ENGINE for capture");
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

// Build the producing tools rather than trusting an existing engine or inventory.
await run([process.execPath, "run", "build"]);
const coreBuildID = await run([documentEngine(), "--build-id"]);
const writerPath = await realpath(suppliedWriter ? resolve(suppliedWriter) : documentEngine());
assert.equal(
  await run([writerPath, "--build-id"]),
  coreBuildID,
  "The supplied writer was built from another core",
);
const writer: Release["writer"] = {
  version: name,
  platform: "darwin-arm64",
  buildId: coreBuildID,
  sha256: await fileDigest(writerPath),
};
// Every CLI write uses the supplied shipped engine, not the checkout's local build.
process.env.HITSLOP_ENGINE = writerPath;
const capturedInputs = await sourceFingerprint();
const stage = await mkdtemp(join(tmpdir(), "hitslop-corpus-stage-"));
// Resolved, as the CLI prints the paths it writes (/var is a link on macOS).
const work = await realpath(await mkdtemp(join(tmpdir(), "hitslop-compat-capture-")));
const root = join(stage, name);
try {
  // Templates: the Svelte conformance app, which exercises every ctx member and descriptor
  // kind, each geometry/window variant and a conformance app declaring host capabilities.
  // The presentation-capabilities template contains the conformance app. A window lives
  // in the stored app, so its template is enough: later builds create, edit and render it.
  const presentation = await prepareNativeFixtures();
  for (const directory of [
    "templates",
    "documents",
    "expected",
    "scenarios",
    "pages",
    "commands",
    "exports",
    "themes",
    "acceptance",
  ])
    await mkdir(join(root, directory), { recursive: true });
  await run([documentEngine(), "--acceptance-vectors", join(root, "acceptance")]);
  const limits: Release["limits"] = JSON.parse(await run([documentEngine(), "--limits"]));
  const templates: Record<string, string> = {
    conformance: join(repository, "generated/abi/owner-svelte.slop"),
  };
  for (const kind of [
    "washer",
    "washer-2x",
    "glass",
    "ellipse",
    "notch",
    "fixed",
    "evenodd",
    "radii",
    "lock-aspect",
    "capabilities",
  ])
    templates[`presentation-${kind}`] = presentation[kind]!;
  for (const [slug, source] of Object.entries(templates))
    await copyFile(source, join(root, "templates", slug + ".slop"));

  // Generic edits derived from a descriptor: one valid write of every kind it declares.
  type Node = { kind: string; [key: string]: any };
  const clamp = (node: Node, x: number) =>
    Math.min(node.max ?? Infinity, Math.max(node.min ?? -Infinity, x));
  function sample(node: Node, round: number): unknown {
    switch (node.kind) {
      case "text":
        return round % 2 ? `Compat ${round} ✓ café 🦊` : `Replay ${round} é 中文`;
      case "boolean":
        return round % 2 === 1;
      case "string":
        return `compat ${round}`.slice(0, node.maxLength ?? 64);
      case "number":
        return clamp(node, round / 4);
      case "integer":
        return Math.round(clamp(node, round));
      case "enum":
        return node.values[round % node.values.length];
      case "counter":
        return 0;
      case "optional":
        return sample(node.inner, round);
      case "object":
        return Object.fromEntries(
          Object.entries(node.properties as Record<string, Node>)
            .filter(([, child]) => child.kind !== "optional")
            .map(([key, child]) => [key, sample(child, round)]),
        );
      case "list":
        return [];
      case "record":
        return {};
    }
    throw new Error(`Unknown kind ${node.kind}`);
  }
  const scalar = (node: Node) =>
    ["boolean", "string", "number", "integer", "enum"].includes(node.kind);
  function edits(node: Node, value: any, round: number, path: unknown[] = []): unknown[] {
    const ops: unknown[] = [];
    for (const [key, child] of Object.entries(node.properties as Record<string, Node>).sort(
      ([a], [b]) => a.localeCompare(b),
    )) {
      // Attachment IDs remain reachable in both CLI and page-saved replay scenarios.
      if (path.length === 0 && ["attachment", "photo", "files"].includes(key)) continue;
      const at = [...path, key];
      const current = value?.[key];
      if (child.kind === "text" || scalar(child))
        ops.push({ type: "set", path: at, value: sample(child, round) });
      else if (child.kind === "counter") ops.push({ type: "increment", path: at, by: round + 1 });
      else if (child.kind === "optional") {
        if (child.inner.kind !== "object")
          ops.push({ type: "set", path: at, value: sample(child.inner, round) });
        else if (current === undefined)
          ops.push({ type: "set", path: at, value: sample(child.inner, round) });
      } else if (child.kind === "object") ops.push(...edits(child, current, round, at));
      else if (child.kind === "list" && child.item.kind === "object") {
        ops.push({
          type: "insert",
          path: at,
          id: `compat-${round}`,
          value: sample(child.item, round),
        });
        const rows = (current ?? []) as { $id: string }[];
        if (rows.length >= 2)
          ops.push({ type: "move", path: at, id: rows[0]!.$id, at: { after: rows.at(-1)!.$id } });
      } else if (child.kind === "list")
        ops.push({ type: "insert", path: at, value: sample(child.item, round), index: 0 });
      else if (child.kind === "record")
        ops.push({
          type: "set",
          path: [...at, `compat-${round}`],
          value: sample(child.value, round),
        });
    }
    return ops;
  }
  const schemaOf = async (document: string) =>
    (await execute({ method: "schema", file: document })).schema as Node;
  const valueOf = async (document: string) => (await slopJSON(["get", document])) as unknown;
  const batch = (document: string, ops: unknown[]) =>
    slopJSON(["batch", document, "--ops", JSON.stringify(ops)]);

  // Initial values and saved/scenario results below are derived by the captured writer.
  // They freeze its behavior; the app's own check() calls and explicit probes provide
  // independent contract assertions.
  // What a new document of each template holds: its initial snapshot.
  for (const slug of Object.keys(templates)) {
    const created = join(work, `new-${slug}.slop`);
    await createDocument(join(root, "templates", slug + ".slop"), created, {
      engine: documentEngine(),
    });
    await writeFile(
      join(root, "expected", `new-${slug}.json`),
      JSON.stringify(await valueOf(created), null, 2) + "\n",
    );
    await rm(created);
  }
  // The conformance document after two closed editing sessions (agent edits, counter
  // increments from two writers, a snapshot plus saved updates), theme overrides/reset and
  // referenced attachments of every classified type; and a compacted copy.
  const documents = join(root, "documents");
  const conformance = join(documents, "conformance.slop");
  await createDocument(join(root, "templates", "conformance.slop"), conformance, {
    engine: documentEngine(),
  });
  const schema = await schemaOf(conformance);
  for (const round of [1, 2])
    await batch(conformance, edits(schema, await valueOf(conformance), round));
  const theme = await slopJSON(["theme", "get", conformance]);
  assert.ok(
    Object.keys(theme.defaults).length >= 3,
    "Conformance needs at least three theme tokens",
  );
  const themeScenario: ThemeScenario = { steps: [] };
  for (const edit of [{ values: { accent: "#123456", ink: "#654321" } }, { reset: "ink" }]) {
    await slopJSON(
      edit.values
        ? ["theme", "set", conformance, "--values", JSON.stringify(edit.values)]
        : ["theme", "reset", conformance, "--token", edit.reset!],
    );
    const { overrides, effective } = await slopJSON(["theme", "get", conformance]);
    themeScenario.steps.push({ ...edit, state: { overrides, effective } });
  }
  assert.deepEqual(themeScenario.steps[0]!.state.overrides, { accent: "#123456", ink: "#654321" });
  assert.deepEqual(themeScenario.steps[1]!.state.overrides, { accent: "#123456" });
  assert.equal(themeScenario.steps[1]!.state.effective.ink, theme.defaults.ink);
  const text = join(work, "attachment.txt");
  await writeFile(text, "Compatibility corpus attachment ✓\n");
  const ref = await slopJSON(["attachments", "ref", text]);
  await slopJSON([
    "apply",
    conformance,
    "--attach",
    text,
    "--op",
    JSON.stringify({ type: "set", path: ["attachment"], value: ref.id }),
  ]);
  await rm(text);
  // A second attachment of a media type the host sniffs from its bytes.
  const image = join(work, "photo.png");
  await copyFile(join(repository, "tests/abi/owner-svelte/media/swatch.png"), image);
  const photo = await slopJSON(["attachments", "ref", image]);
  await slopJSON([
    "apply",
    conformance,
    "--attach",
    image,
    "--op",
    JSON.stringify({ type: "set", path: ["photo"], value: photo.id }),
  ]);
  await rm(image);
  // Passive-signature fixtures freeze the classifier independently of app-asset filenames.
  // Complete PNG/JPEG/GIF/WebP files also exercise the host's actual image decoders.
  const ftyp = (major: string, compatible = "") => {
    const bytes = Buffer.alloc(16 + compatible.length);
    bytes.writeUInt32BE(bytes.length);
    bytes.write("ftyp", 4);
    bytes.write(major, 8);
    bytes.write(compatible, 16);
    return bytes;
  };
  const media = join(repository, "tests/abi/owner-svelte/media");
  const fixtures: [string, string, Uint8Array][] = [
    ["png", "image/png", await readFile(join(media, "swatch.png"))],
    ["jpeg", "image/jpeg", await readFile(join(media, "sample.jpg"))],
    ["gif", "image/gif", await readFile(join(media, "sample.gif"))],
    ["webp", "image/webp", await readFile(join(media, "sample.webp"))],
    ["avif", "image/avif", ftyp("avif")],
    ["heic", "image/heic", ftyp("heic", "mif1")],
    ["wav", "audio/wav", Buffer.from("RIFFxxxxWAVE")],
    ["mp3-id3", "audio/mpeg", Buffer.from("ID3")],
    ["mp3-frame", "audio/mpeg", Buffer.from([0xff, 0xfb, 0x90])],
    ["m4a", "audio/mp4", ftyp("M4A ", "isom")],
    ["ogg", "audio/ogg", Buffer.from("OggS")],
    ["flac", "audio/flac", Buffer.from("fLaC")],
    ["mp4", "video/mp4", ftyp("isom", "mp42")],
    ["mov", "video/quicktime", ftyp("qt  ")],
    [
      "webm",
      "video/webm",
      Buffer.from([0x1a, 0x45, 0xdf, 0xa3, 0x87, 0x42, 0x82, 0x84, ...Buffer.from("webm")]),
    ],
    ["pdf", "application/pdf", Buffer.from("%PDF-1.7")],
    ["binary", "application/octet-stream", Buffer.from([0, 1, 2, 3])],
    ["late-avif", "image/avif", ftyp("isom", "mp42".repeat(2048) + "avif")],
  ];
  assert.equal(new Set(fixtures.map(([, mime]) => mime)).size, 16);
  assert.ok(
    schema.properties.files?.kind === "list" && schema.properties.files.item.kind === "string",
    "F1 conformance schema must declare files: s.list(s.string())",
  );
  // Include the text/photo refs too: the page's contractTest replaces attachment later.
  const ids: string[] = [ref.id, photo.id];
  for (const [label, mime, bytes] of fixtures) {
    const file = join(work, label + ".blob");
    await writeFile(file, bytes);
    const reference = await slopJSON(["attachments", "ref", file]);
    assert.equal(reference.mimeType, mime, `Attachment classifier changed for ${label}`);
    ids.push(reference.id);
    await slopJSON([
      "apply",
      conformance,
      "--attach",
      file,
      "--op",
      JSON.stringify({ type: "set", path: ["files"], value: [...ids] }),
    ]);
    await rm(file);
  }
  const attached = await slopJSON(["attachments", "list", conformance]);
  assert.equal(attached.length, new Set(ids).size, "Capture lost a referenced attachment");
  const compacted = join(documents, "conformance-compacted.slop");
  await copyFile(conformance, compacted);
  await run([
    "cargo",
    "run",
    "-q",
    "--locked",
    "-p",
    "hitslop-core",
    "--features",
    "storage",
    "--example",
    "compat_checkpoint",
    "--",
    compacted,
  ]);

  // What each document reads as, and an edit to replay on it with its result.
  const names = (await readdir(documents))
    .filter((n) => n.endsWith(".slop"))
    .map((n) => n.slice(0, -5))
    .sort();
  const scratch = join(work, "Document.slop");
  async function record(document: string) {
    const path = join(documents, document + ".slop");
    const expected: Expected = await savedState(path);
    await writeFile(
      join(root, "expected", document + ".json"),
      JSON.stringify(expected, null, 2) + "\n",
    );
    const ops = edits(await schemaOf(path), expected.value, 3);
    await rm(scratch, { recursive: true, force: true });
    await copyFile(path, scratch);
    await batch(scratch, ops);
    const { value } = await slopJSON(["get", scratch, "--snapshot"]);
    const scenario: Scenario = { ops, value };
    await writeFile(
      join(root, "scenarios", document + ".json"),
      JSON.stringify(scenario, null, 2) + "\n",
    );
    await writeFile(
      join(root, "themes", document + ".json"),
      JSON.stringify(themeScenario, null, 2) + "\n",
    );
    // Golden content is derived by the build being captured, not an independent oracle.
    // The app's own contractTest checks its edits; these goldens freeze rendered content.
    const golden = {} as ExportGolden;
    // Keep the render bytes as inspectable corpus artifacts as well as their content summary.
    for (const format of ["png", "pdf"]) {
      const output = join(root, "exports", `${document}.${format}`);
      await slop(["export", path, "--format", format, "--output", output]).then(
        ({ code, stderr }) => {
          if (code) throw new Error(stderr);
        },
      );
      if (format === "png") {
        const { width, height, data } = PNG.sync.read(await readFile(output));
        golden.png = { width, height, rgbaSha256: createHash("sha256").update(data).digest("hex") };
        assert.ok(width > 0 && height > 0, "Empty PNG export");
        let swatchPixels = 0;
        for (let i = 0; i < data.length; i += 4)
          if (data[i] === 51 && data[i + 1] === 85 && data[i + 2] === 119 && data[i + 3] === 255)
            swatchPixels++;
        assert.ok(swatchPixels >= 256, "Export did not draw the attached RGB swatch");
      } else {
        // PDFKit is also the native replay's text reader; no external PDF tool is required.
        golden.pdf = JSON.parse(
          await run([
            "swift",
            "-module-cache-path",
            join(work, "swift-cache"),
            "-e",
            `
import Foundation
import PDFKit
let document = PDFDocument(url: URL(fileURLWithPath: CommandLine.arguments[1]))!
let pages = (0..<document.pageCount).map { index -> [String: Any] in
  let page = document.page(at: index)!, bounds = page.bounds(for: .mediaBox)
  return ["width": bounds.width, "height": bounds.height, "text": page.string ?? ""]
}
print(String(data: try JSONSerialization.data(withJSONObject: ["pages": pages]), encoding: .utf8)!)
`,
            output,
          ]),
        );
        assert.ok(
          golden.pdf.pages.length > 0 && golden.pdf.pages.some((page) => page.text.length > 0),
          "PDF export has no extractable text",
        );
        const text = golden.pdf.pages.map((page) => page.text).join("\n");
        assert.ok(text.includes("中文"), "Export lost packaged-font CJK text");
        assert.ok(!text.includes("Hidden in capture"), "Export rendered an export-hidden node");
      }
    }
    await writeFile(
      join(root, "exports", document + ".json"),
      JSON.stringify(golden, null, 2) + "\n",
    );
  }
  for (const document of names) {
    await record(document);
    // The conformance app's own contract test is its page scenario.
    const page: Page = { script: "contractTest", value: null };
    await writeFile(join(root, "pages", document + ".json"), JSON.stringify(page, null, 2) + "\n");
  }

  // Storage shapes the documents cover.
  const storage: Release["storage"] = {};
  function measure(document: string) {
    const database = new Database(join(documents, document + ".slop"), { readonly: true });
    // The history's first row is its snapshot; the rest are updates saved after it.
    const checkpoint = database
      .query("SELECT length(bytes) AS n FROM history ORDER BY seq LIMIT 1")
      .get() as { n: number };
    const history = database.query("SELECT count(*) AS n FROM history").get() as { n: number };
    database.close();
    storage[document] = { checkpointBytes: checkpoint.n, updates: history.n - 1 };
  }
  for (const document of names) measure(document);
  if (!Object.values(storage).some(({ updates }) => updates > 0))
    throw new Error("No document keeps saved updates past its checkpoint");

  const version = async (command: string[]) => (await run(command)).split("\n")[0]!;
  const lock = await readFile(join(repository, "Cargo.lock"), "utf8");
  const commit = (await run(["git", "rev-parse", "HEAD"])) + (dirty ? "-dirty" : "");
  const release: Release = {
    release: name,
    frozen: false,
    commit,
    captured: new Date().toISOString(),
    markers: {
      packageFormat: PackageFormat,
      runtimeABI: RuntimeABI,
      storage: Number(
        (await readFile(join(repository, "crates/hitslop-core/src/file/mod.rs"), "utf8")).match(
          /const STORAGE_VERSION: i64 = (\d+);/,
        )![1],
      ),
      layout: Number(
        (await readFile(join(repository, "crates/hitslop-core/src/document/mod.rs"), "utf8")).match(
          /pub const LAYOUT: i64 = (\d+);/,
        )![1],
      ),
      protocol: EngineProtocol.version,
    },
    sourceInputs: capturedInputs,
    producer: {
      coreBuildID,
      shell: await digest(shellDestinations.app, shellFiles),
      // The command prelude this release ran; a frozen entry freezes it (scripts/build/runner.ts).
      runner: {
        [RuntimeABI]: await fileDigest(
          join(repository, `crates/hitslop-runner/src/abi/${RuntimeABI}.generated.js`),
        ),
      },
    },
    files: {},
    limits,
    storageSchemas: Object.fromEntries(
      await Promise.all(
        ["crates/hitslop-core/src/file/storage-1.sql"].map(async (path) => [
          path,
          await fileDigest(join(repository, path)),
        ]),
      ),
    ),
    templates: Object.fromEntries(
      await Promise.all(
        Object.keys(templates).map(async (slug) => [
          slug,
          await fileDigest(join(root, "templates", slug + ".slop")),
        ]),
      ),
    ),
    writer,
    toolchain: {
      rust: await version(["rustc", "--version"]),
      bun: Bun.version,
      swift: await version(["swift", "--version"]),
      macos: await version(["sw_vers", "-productVersion"]),
      loro: lock.match(/name = "loro"\nversion = "([^"]+)"/)![1]!,
      svelte: JSON.parse(
        await readFile(join(repository, "node_modules/svelte/package.json"), "utf8"),
      ).version,
    },
    // Noon UTC is the same date in nearly every time zone a later run may use.
    clock: (() => {
      const today = new Date();
      return Date.UTC(today.getUTCFullYear(), today.getUTCMonth(), today.getUTCDate(), 12);
    })(),
    storage,
  };
  await writeFile(join(root, "release.json"), JSON.stringify(release, null, 2) + "\n");

  // Each probe starts from the same saved state, so refusal/no-op probes can prove
  // that they leave it intact. Expectations below are contract checks, not writer output.
  const probes = [
    { name: "bump", args: { by: 2 }, channel: "evaluate", expected: { ok: true } },
    { name: "shapes", args: {}, channel: "evaluate", expected: { ok: true } },
    {
      name: "arguments",
      args: { boolean: true, string: "default", number: 0.5, integer: 7, enum: "todo" },
      channel: "evaluate",
      expected: { ok: true },
    },
    {
      name: "arguments",
      args: {
        boolean: false,
        string: "explicit",
        number: 0.75,
        integer: 8,
        enum: "done",
        optionalBoolean: true,
        optionalString: "optional",
        optionalNumber: 0.25,
        optionalInteger: 9,
        optionalEnum: "done",
      },
      channel: "evaluate",
      expected: { ok: true },
    },
    { name: "arguments", args: {}, channel: "call", expected: { ok: true } },
    { name: "nothing", args: {}, channel: "evaluate", expected: { ok: true } },
    { name: "decline", args: {}, channel: "evaluate", expected: { ok: false, refused: true } },
    // Argument validation belongs to the owner, before the restricted evaluator runs.
    {
      name: "bump",
      args: { by: "bad" },
      channel: "call",
      expected: { ok: false, code: "rejected", errorIncludes: "Invalid arguments" },
    },
  ] satisfies Omit<CommandProbe, "now" | "seed" | "evaluated" | "value">[];
  const commandProbes = Object.fromEntries(names.map((document) => [document, probes]));
  for (const [slug, probes] of Object.entries(commandProbes)) {
    const scenarios: CommandProbe[] = [];
    for (const probe of probes) {
      const original = join(documents, slug + ".slop"),
        copy = join(work, "command.slop");
      await rm(copy, { force: true });
      await copyFile(original, copy);
      const input = { name: probe.name, args: probe.args, now: release.clock, seed: [1, 2, 3, 4] };
      const state = await slopJSON(["get", copy, "--snapshot"]);
      // evaluateStored must admit ok:false only when this probe explicitly expects it.
      // The expectation is passed as metadata for replay, outside the authored arguments.
      const evaluationInput = { ...input, expected: probe.expected };
      const evaluated =
        probe.channel === "call"
          ? await request(
              { method: "call", documentPath: copy, command: probe.name, args: probe.args },
              { binary: documentEngine() },
            )
          : await evaluateStored(copy, state, evaluationInput);
      assert.equal(evaluated.ok, probe.expected.ok, `${slug}: ${probe.name} outcome`);
      if ("refused" in probe.expected) assert.equal(evaluated.refused, probe.expected.refused);
      if ("code" in probe.expected) assert.equal(evaluated.code, probe.expected.code);
      if ("errorIncludes" in probe.expected)
        assert.ok(evaluated.error.includes(probe.expected.errorIncludes));
      if (probe.channel === "evaluate" && evaluated.ok && evaluated.intents.length)
        await batch(copy, evaluated.intents);
      const value = await valueOf(copy);
      if (!evaluated.ok)
        assert.deepEqual(value, state.value, "Refused command changed the document");
      const result = evaluated.result;
      if (evaluated.ok && probe.name === "bump") assert.equal(result, state.value.hits + 2);
      if (evaluated.ok && probe.name === "nothing") assert.equal(result, null);
      if (evaluated.ok && probe.name === "shapes") {
        assert.deepEqual(
          [...new Set<string>(evaluated.intents.map((intent: { type: string }) => intent.type))].sort((a, b) => a.localeCompare(b)),
          ["clear", "increment", "insert", "move", "remove", "set"],
        );
        assert.equal(result.now, release.clock);
        assert.ok(result.random >= 0 && result.random < 1);
      }
      if (evaluated.ok && probe.name === "arguments")
        assert.deepEqual(
          result,
          "optionalBoolean" in probe.args
            ? [false, "explicit", 0.75, 8, "done", true, "optional", 0.25, 9, "done"]
            : [true, "default", 0.5, 7, "todo", null, null, null, null, null],
        );
      scenarios.push({ ...probe, ...input, evaluated, value });
    }
    await writeFile(
      join(root, "commands", slug + ".json"),
      JSON.stringify(scenarios, null, 2) + "\n",
    );
  }

  // The old apps' own edits: the native corpus test records what each page scenario saves.
  const swift = await exec(
    [process.execPath, "scripts/verify.ts", "--no-build", "swift", "--filter", "CompatCorpusTests"],
    {
      cwd: repository,
      inherit: ["stdout", "stderr"],
      env: { ...process.env, HITSLOP_COMPAT_RECORD: name, HITSLOP_COMPAT_ROOT: stage, TZ: "UTC" },
    },
  );
  if (swift.code) throw new Error("Recording page scenarios failed");
  for (const document of await readdir(join(root, "pages"))) {
    const page = JSON.parse(await readFile(join(root, "pages", document), "utf8")) as Page;
    if (page.value === null) throw new Error(`No page result recorded for ${document}`);
  }
  // The documents the old apps' pages saved, recorded like the CLI-written ones.
  for (const document of names) {
    const saved = `${document}.page`;
    if (!(await Bun.file(join(documents, saved + ".slop")).exists()))
      throw new Error(`No page-saved document for ${document}`);
    await record(saved);
    measure(saved);
  }
  if ((await sourceFingerprint()) !== capturedInputs)
    throw new Error("Producing inputs changed during capture");
  assert.equal(await fileDigest(writerPath), writer.sha256, "Writer changed during capture");
  await resolveWriter(writer, { local: writerPath });
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
