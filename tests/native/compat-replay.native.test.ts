// Replays the compatibility corpus (tests/compat) through this build's CLI and helper: every
// saved document of every release reads as that release recorded, renders with its own old
// app, keeps its attachments, takes a CLI edit and reopens to the recorded result; every
// template master still creates documents; and the commands each release's CLI ran still
// mean the same thing. The Rust and Swift corpus tests cover the rest (docs/testing.md).
//   HITSLOP_COMPAT_RELEASE=VERSION  also require a frozen entry for VERSION (the release gate)
//   HITSLOP_COMPAT_INSTALLED=1      install each entry's npm CLI and run it against this helper
import { afterAll, beforeAll, describe, expect, test } from "bun:test";
import { strict as assert } from "node:assert";
import { copyFile, cp, mkdtemp, readdir, realpath, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { exec } from "../../packages/cli/src/process";
import { assertExport, createDocument } from "../../scripts/lib/native";
import {
  assertOutput,
  documents,
  helper,
  documentEngine,
  readJSON,
  releases,
  savedState,
  slop,
  slopJSON,
  type Expected,
  type Scenario,
  type Transcript,
} from "../../scripts/compat/corpus";
import { verifyCandidate, verifyCorpus } from "../../scripts/compat/integrity";
import { fileDigest, sha256, useTestRegistry } from "../../scripts/lib/artifacts";
useTestRegistry();

const required = process.env.HITSLOP_COMPAT_RELEASE;
const installed = process.env.HITSLOP_COMPAT_INSTALLED === "1";
const entries = await Promise.all(
  (await releases()).map(async (entry) => ({
    ...entry,
    documents: await documents(entry.root),
    templates: await readdir(join(entry.root, "templates")),
    transcript: await readJSON<Transcript>(join(entry.root, "cli/transcript.json")),
  })),
);
// Resolved, as the CLI prints the paths it writes (/var is a link on macOS).
let scratch: string;
beforeAll(async () => {
  scratch = await realpath(await mkdtemp(join(tmpdir(), "hitslop-compat-")));
});
afterAll(() => rm(scratch, { recursive: true, force: true }));

test("the corpus has entries to replay", () => {
  expect(entries.length).toBeGreaterThan(0);
});
if (required)
  test(`tests/compat/${required} is captured, frozen and matches this candidate`, async () => {
    const entry = entries.find(({ name }) => name === required);
    assert.ok(entry?.release.frozen, `tests/compat/${required} must be captured and frozen before this release`);
    await verifyCandidate(entry.root, entry.release);
  }, 120_000);

type Run = (args: string[]) => Promise<{ stdout: string; stderr: string; code: number }>;
/** The commands a release's CLI ran, in order, on its own document, through `cli`: the
 * same exit status and, apart from envelope metadata that may grow, the same output. */
async function replayTranscript(root: string, transcript: Transcript, label: string, cli: Run) {
  const document = join(scratch, `${label}.slop`);
  await rm(document, { force: true });
  await copyFile(join(root, "documents", transcript.document + ".slop"), document);
  for (const { args, code, stdout, outputHash } of transcript.commands) {
    const exported = join(scratch, `${label}-export.txt`);
    const resolved = args.map((arg) =>
      arg === "{document}" ? document : arg === "{attachment}" ? join(root, "cli/attachment.txt") : arg === "{output}" ? exported : arg,
    );
    const result = await cli(resolved);
    const command = `${label}: ${args.slice(0, 2).join(" ")}`;
    assert.equal(result.code, code, `${command} exit status: ${result.stderr.trim()}`);
    let output: unknown = result.stdout.trim();
    try {
      output = JSON.parse(result.stdout);
    } catch {}
    if (outputHash) {
      assert.equal(await fileDigest(exported), outputHash, `${command}: exported bytes`);
      output = String(output).replaceAll(exported, "{output}");
      await rm(exported);
    }
    assertOutput(output, stdout, args, `${command} output`);
  }
}

for (const { name, root, release, documents: saved, templates, transcript } of entries)
  describe(`tests/compat/${name} (${release.frozen ? "frozen" : "replaceable"})`, () => {
    test("its files match their recorded hashes", () => verifyCorpus(root, release));

    for (const document of saved)
      test(`${document} reads as recorded, renders with its own app, keeps its attachments and reopens edited`, async () => {
        const copy = join(scratch, `${name}-${document}.slop`);
        const fresh = async () => {
          await rm(copy, { force: true });
          await copyFile(join(root, "documents", document + ".slop"), copy);
        };
        await fresh();
        const expected = (await readJSON<Expected>(join(root, "expected", document + ".json")))!;
        expect(await savedState(copy)).toEqual(expected);
        for (const { id, byteLength } of expected.attachments) {
          const output = join(scratch, id);
          const exported = await slop(["attachments", "export", copy, id, "--output", output]);
          assert.equal(exported.code, 0, `attachment ${id}: ${exported.stderr.trim()}`);
          const bytes = await Bun.file(output).bytes();
          expect(bytes.length).toBe(byteLength);
          expect(sha256(bytes)).toBe(id);
          await rm(output);
        }
        for (const format of ["png", "pdf"] as const) {
          const output = join(scratch, `${document}.${format}`);
          const { code, stderr } = await slop(["export", copy, "--format", format, "--output", output]);
          assert.equal(code, 0, `the old app did not render: ${stderr.trim()}`);
          await assertExport(output, format);
          await rm(output);
        }
        const scenario = await readJSON<Scenario>(join(root, "scenarios", document + ".json"));
        if (scenario) {
          await fresh();
          await slopJSON(["batch", copy, "--ops", JSON.stringify(scenario.ops)]);
          expect((await slopJSON(["get", copy, "--snapshot"])).value).toEqual(scenario.value);
        }
      }, 120_000);

    for (const file of templates)
      test(`template ${file} creates the document its release did`, async () => {
        const output = join(scratch, `${name}-created-${file}`);
        await rm(output, { force: true });
        await createDocument(join(root, "templates", file), output, { engine: documentEngine });
        const initial = await readJSON(join(root, "expected", `new-${file.slice(0, -".slop".length)}.json`));
        expect((await slopJSON(["get", output, "--snapshot"])).value).toEqual(initial);
      }, 120_000);

    if (transcript)
      test(`this build's CLI runs the ${transcript.commands.length} commands its release's CLI ran`, () =>
        replayTranscript(root, transcript, `${name}-transcript`, slop), 120_000);

    if (installed)
      test("the released CLI itself, installed from its own tarballs, runs against this helper", async () => {
        assert.ok(transcript?.commands.length, "no CLI scenarios");
        const installation = join(scratch, `${name}-cli`, "install");
        await cp(join(root, "cli"), join(scratch, `${name}-cli`), { recursive: true });
        const install = await exec([process.execPath, "install", "--frozen-lockfile"], { cwd: installation, inherit: ["stdout", "stderr"] });
        assert.equal(install.code, 0, "the released CLI did not install");
        const metadata = await Bun.file(join(installation, "node_modules/@hitslop/cli/package.json")).json();
        assert.equal(typeof metadata.bin?.slop, "string", "Archived CLI has no public slop executable");
        const executable = join(installation, "node_modules/@hitslop/cli", metadata.bin.slop);
        await replayTranscript(root, transcript, `${name}-installed`, (args) =>
          exec([process.execPath, executable, ...args], { cwd: installation, env: { ...process.env, HITSLOP_NATIVE_CLI: helper }, timeout: 120_000 }),
        );
      }, 300_000);
  });
