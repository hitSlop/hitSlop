// Replays the compatibility corpus (tests/compat) through this build's CLI and helper: every
// saved document of every release reads as that release recorded, renders with its own old
// app, keeps its attachments, takes a CLI edit and reopens to the recorded result; every
// template master still creates documents; and the commands each release's CLI ran still
// mean the same thing. The Rust and Swift corpus tests cover the rest.
// Usage: bun scripts/compat-replay.ts [--release VERSION] [--installed]
//   --release   also require a frozen entry for VERSION (the release gate)
//   --installed install each frozen entry's npm CLI and run it against this helper
import { strict as assert } from "node:assert";
import { exec } from "../packages/cli/src/process";
import { assertExport, createDocument } from "./helper";
import { copyFile, cp, mkdtemp, readdir, realpath, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import {
  documents,
  helper,
  slop,
  slopJSON,
  readJSON,
  releases,
  savedState,
  stable,
  assertOutput,
  type Expected,
  type Scenario,
  type Transcript,
} from "./compat";
import { verifyCandidate, verifyCorpus } from "./compat-integrity";
import { fileDigest, sha256, useTestRegistry } from "./runtime-artifacts";
useTestRegistry();

const flag = (name: string) => process.argv.indexOf(name);
const tag = process.env.HITSLOP_RELEASE_TAG || process.env.GITHUB_REF_NAME;
const required = flag("--release") >= 0 ? process.argv[flag("--release") + 1] : tag?.startsWith("macos-v") ? tag.slice(7) : undefined;
const installed = flag("--installed") >= 0;
const entries = await releases();
assert.ok(entries.length > 0, "tests/compat has no entries to replay");
if (required) {
  const entry = entries.find(({ name }) => name === required);
  assert.ok(entry?.release.frozen, `tests/compat/${required} must be captured and frozen before this release`);
  await verifyCandidate(entry.root, entry.release);
}
// Resolved, as the CLI prints the paths it writes (/var is a link on macOS).
const scratch = await realpath(await mkdtemp(join(tmpdir(), "hitslop-compat-")));
let cases = 0;
try {
  for (const { name, root, release } of entries) {
    await verifyCorpus(root, release);
    console.log(`Replaying tests/compat/${name} (${release.frozen ? "frozen" : "replaceable"}, ${release.commit})`);
    for (const document of await documents(root)) {
      const copy = join(scratch, `${name}-${document}.slop`);
      const fresh = async () => {
        await rm(copy, { force: true });
        await copyFile(join(root, "documents", document + ".slop"), copy);
      };
      await fresh();
      const expected = (await readJSON<Expected>(join(root, "expected", document + ".json")))!;
      assert.deepEqual(await savedState(copy), expected, `${name}/${document}: the saved document reads differently`);
      for (const { id, byteLength } of expected.attachments) {
        const output = join(scratch, id);
        const exported = await slop(["attachments", "export", copy, id, "--output", output]);
        assert.equal(exported.code, 0, `${name}/${document}: attachment ${id}: ${exported.stderr.trim()}`);
        const bytes = await Bun.file(output).bytes();
        assert.equal(bytes.length, byteLength, `${name}/${document}: attachment ${id} size`);
        assert.equal(sha256(bytes), id, `${name}/${document}: attachment ${id} bytes`);
        await rm(output);
      }
      for (const format of ["png", "pdf"] as const) {
        const output = join(scratch, `${document}.${format}`);
        const { code, stderr } = await slop(["export", copy, "--format", format, "--output", output]);
        assert.equal(code, 0, `${name}/${document}: the old app did not render: ${stderr.trim()}`);
        await assertExport(output, format);
        await rm(output);
      }
      const scenario = await readJSON<Scenario>(join(root, "scenarios", document + ".json"));
      if (scenario) {
        await fresh();
        await slopJSON(["batch", copy, "--ops", JSON.stringify(scenario.ops)]);
        const { state } = await slopJSON(["get", copy, "--snapshot"]);
        assert.deepEqual(state.value, scenario.value, `${name}/${document}: the edit did not reopen`);
      }
      cases++;
      console.log(`PASS ${name}/${document}`);
    }
    // Every template master still creates a document that reads as its release's did.
    for (const file of await readdir(join(root, "templates"))) {
      const output = join(scratch, `created-${file}`);
      await rm(output, { force: true });
      await createDocument(join(root, "templates", file), output, { helper }).catch((error) => assert.fail(`${name}/${file}: create failed: ${error.message}`));
      const { state } = await slopJSON(["get", output, "--snapshot"]);
      const initial = await readJSON(join(root, "expected", `new-${file.slice(0, -".slop".length)}.json`));
      assert.deepEqual(state.value, initial, `${name}/${file}: a new document differs from its release's`);
      cases++;
    }
    // The commands this release's CLI ran, in order, on its own document.
    const transcript = await readJSON<Transcript>(join(root, "cli/transcript.json"));
    if (transcript) {
      const copy = join(scratch, `${name}-transcript.slop`);
      await rm(copy, { force: true });
      await copyFile(join(root, "documents", transcript.document + ".slop"), copy);
      for (const { args, code, stdout, outputHash } of transcript.commands) {
        const exported = join(scratch, `${name}-attachment-export.txt`);
        const result = await slop(args.map(arg => arg === "{document}" ? copy : arg === "{attachment}" ? join(root, "cli/attachment.txt") : arg === "{output}" ? exported : arg));
        assert.equal(result.code, code, `${name}: ${args.slice(0, 2).join(" ")} exit status: ${result.stderr.trim()}`);
        let output: unknown = result.stdout.trim();
        try {
          output = stable(JSON.parse(result.stdout), args);
        } catch {}
        if (outputHash) {
          assert.equal(await fileDigest(exported), outputHash);
          output = String(output).replaceAll(exported, "{output}");
          await rm(exported);
        }
        assertOutput(output, stdout, args, `${name}: ${args.slice(0, 2).join(" ")} output`);
      }
      cases++;
      console.log(`PASS ${name}: ${transcript.commands.length} CLI commands`);
    }
    if (installed) {
      // The released CLI itself, installed from its own tarballs, against this helper.
      const project = join(scratch, `${name}-cli`);
      await cp(join(root, "cli"), project, { recursive: true });
      const installation = join(project, "install");
      const install = await exec([process.execPath, "install", "--frozen-lockfile"], { cwd: installation, inherit: ["stdout", "stderr"] });
      assert.equal(install.code, 0, `${name}: the released CLI did not install`);
      const metadata = await Bun.file(join(installation, "node_modules/@hitslop/cli/package.json")).json();
      const bin = metadata.bin?.slop;
      assert.equal(typeof bin, "string", "Archived CLI has no public slop executable");
      const executable = join(installation, "node_modules/@hitslop/cli", bin);
      assert.ok(transcript?.commands.length, `${name}: no CLI scenarios`);
      const copy = join(scratch, `${name}-installed.slop`);
      await copyFile(join(root, "documents", transcript.document + ".slop"), copy);
      for (const { args, code: expectedCode, stdout: expectedOutput, outputHash } of transcript.commands) {
        const exported = join(scratch, `${name}-installed-export.txt`);
        const resolved = args.map(arg => arg === "{document}" ? copy : arg === "{attachment}" ? join(root, "cli/attachment.txt") : arg === "{output}" ? exported : arg);
        const { stdout, stderr, code } = await exec([process.execPath, executable, ...resolved], {
          cwd: installation, env: { ...process.env, HITSLOP_NATIVE_CLI: helper }, timeout: 120_000,
        });
        assert.equal(code, expectedCode, `${name}: archived CLI ${args[0]}: ${stderr.trim()}`);
        let output: unknown = stdout.trim();
        try { output = JSON.parse(stdout); } catch {}
        if (outputHash) {
          assert.equal(await fileDigest(exported), outputHash);
          output = String(output).replaceAll(exported, "{output}");
          await rm(exported);
        }
        assertOutput(output, expectedOutput, args, `${name}: archived CLI ${args[0]}`);
      }
      cases++;
      console.log(`PASS ${name}: the released CLI`);
    }
  }
} finally {
  await rm(scratch, { recursive: true, force: true });
}
assert.ok(cases > 0, "No corpus case ran");
console.log(`Compatibility corpus: ${cases} cases passed`);
