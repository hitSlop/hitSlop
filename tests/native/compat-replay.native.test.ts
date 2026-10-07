// Replays the compatibility corpus (tests/compat) through this build's CLI and helper: every
// saved document of every release reads as that release recorded, renders with its own old
// app, keeps its attachments, takes a CLI edit and reopens to the recorded result; every
// template master still creates documents. Old files are checked, not old programs. The Rust
// and Swift corpus tests cover the rest (docs/testing.md).
//   HITSLOP_COMPAT_RELEASE=VERSION  also require a frozen entry for VERSION (the release gate)
import { afterAll, beforeAll, describe, expect, test } from "bun:test";
import { strict as assert } from "node:assert";
import { copyFile, mkdtemp, readdir, realpath, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { assertExport, createDocument } from "../../scripts/lib/native";
import {
  documents,
  documentEngine,
  readJSON,
  releases,
  savedState,
  slop,
  slopJSON,
  type Expected,
  type Scenario,
} from "../../scripts/compat/corpus";
import { verifyCandidate, verifyCorpus } from "../../scripts/compat/integrity";
import { sha256, useTestRegistry } from "../../scripts/lib/artifacts";
import { evaluateStored, type CommandScenario } from "../../scripts/compat/commands";
useTestRegistry();

const required = process.env.HITSLOP_COMPAT_RELEASE;
const entries = await Promise.all(
  (await releases()).map(async (entry) => ({
    ...entry,
    documents: await documents(entry.root),
    templates: await readdir(join(entry.root, "templates")),
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

for (const { name, root, release, documents: saved, templates } of entries)
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
        const command = await readJSON<CommandScenario>(join(root, "commands", document + ".json"));
        if (command) {
          await fresh();
          const state = await slopJSON(["get", copy, "--snapshot"]);
          const evaluated = await evaluateStored(copy, state, command);
          expect(evaluated).toEqual(command.evaluated);
          await slopJSON(["batch", copy, "--ops", JSON.stringify(evaluated.intents)]);
          expect(await slopJSON(["get", copy])).toEqual(command.value);
          // The same stored program through the owner, as an agent or page calls it: the
          // owner checks its arguments before evaluating, and a valid call edits.
          await fresh();
          const unread = await slop(["call", copy, command.name, "--args", JSON.stringify({ unexpected: true })]);
          expect(unread.code).not.toBe(0);
          expect(unread.stderr + unread.stdout).toContain("Invalid arguments");
          expect((await slopJSON(["get", copy, "--snapshot"])).value).toEqual(state.value);
          await slopJSON(["call", copy, command.name, "--args", JSON.stringify(command.args)]);
          expect((await slopJSON(["get", copy, "--snapshot"])).value).not.toEqual(state.value);
        }
      }, 120_000);

    for (const file of templates)
      test(`template ${file} creates the document its release did`, async () => {
        const output = join(scratch, `${name}-created-${file}`);
        await rm(output, { force: true });
        await createDocument(join(root, "templates", file), output, { engine: documentEngine() });
        const initial = await readJSON(join(root, "expected", `new-${file.slice(0, -".slop".length)}.json`));
        expect((await slopJSON(["get", output, "--snapshot"])).value).toEqual(initial);
      }, 120_000);
  });
