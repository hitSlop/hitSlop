/** Resumable publication. Run only in the serialized release workflow. Artifacts are
 * immutable; the registry and GitHub, rather than a local success flag, record progress. */
import { createHash } from "node:crypto";
import { mkdir, readFile, writeFile, appendFile } from "node:fs/promises";
import { join } from "node:path";
import { exec, run } from "../../packages/hitslop/src/cli/process";
import { fileDigest } from "../lib/artifacts";
import { advancesBuild, appcastBuild, preservesLatest, validateRecord, verifyArtifacts, type ReleaseRecord } from "./record";

const directory = "dist/macos";
const releaseTag = process.env.GITHUB_REF_NAME;
if (!releaseTag || !/^v\d+\.\d+\.\d+$/.test(releaseTag)) throw new Error("Expected a vX.Y.Z release tag");
const tag = releaseTag;
const version = tag.slice(1);
const releaseCommit = process.env.GITHUB_SHA;
if (!releaseCommit) throw new Error("GITHUB_SHA is required");
const commit = releaseCommit;
const repository = process.env.GITHUB_REPOSITORY;
if (!repository) throw new Error("GITHUB_REPOSITORY is required");
const gh = (args: string[]) => run(["gh", ...args]);
async function remoteRelease() {
  const result = await exec(["gh", "api", `repos/${repository}/releases/tags/${tag}`]);
  if (result.code) {
    if (result.stderr.includes("HTTP 404")) return undefined;
    throw new Error(result.stderr);
  }
  return JSON.parse(result.stdout) as { assets: { name: string; id: number; digest?: string }[] };
}
async function asset(id: number) {
  // Preserve binary bytes: `run` is intentionally a text-only interface.
  const child = Bun.spawn(["gh", "api", "-H", "Accept: application/octet-stream", `repos/${repository}/releases/assets/${id}`], { stdout: "pipe", stderr: "pipe" });
  const [bytes, error, code] = await Promise.all([new Response(child.stdout).arrayBuffer(), new Response(child.stderr).text(), child.exited]);
  if (code) throw new Error(error);
  return Buffer.from(bytes);
}
async function check(record: ReleaseRecord) {
  validateRecord(record, tag, commit);
  await verifyArtifacts(record, directory);
}
async function record() { return JSON.parse(await readFile(join(directory, "release-record.json"), "utf8")) as ReleaseRecord; }
async function mayPromote(candidate: ReleaseRecord) {
  const npmLatest = await fetch("https://registry.npmjs.org/hitslop/latest");
  if (npmLatest.ok) {
    const published = await npmLatest.json() as { version: string };
    if (!preservesLatest(version, published.version)) return false;
  } else if (npmLatest.status !== 404) throw new Error(`Cannot check npm latest: ${npmLatest.status}`);
  const latest = await exec(["gh", "api", `repos/${repository}/releases/latest`]);
  if (latest.code) {
    if (latest.stderr.includes("HTTP 404")) return true;
    throw new Error(latest.stderr);
  }
  const release = JSON.parse(latest.stdout);
  if (release.tag_name === tag) return true;
  const recorded = release.assets.find((a: { name: string }) => a.name === "release-record.json");
  if (recorded) return advancesBuild(candidate.macBuild, JSON.parse((await asset(recorded.id)).toString()).macBuild);
  const appcast = release.assets.find((a: { name: string }) => a.name === "appcast.xml");
  if (!appcast) throw new Error("Latest release has no build record or appcast; establish its build number before promotion");
  return advancesBuild(candidate.macBuild, appcastBuild((await asset(appcast.id)).toString()));
}

const action = process.argv[2];
await mkdir(directory, { recursive: true });
if (action === "resume") {
  const remote = await remoteRelease();
  const manifest = remote?.assets.find(a => a.name === "release-record.json");
  let resumed = false;
  if (remote && manifest) {
    const bytes = await asset(manifest.id);
    const saved: ReleaseRecord = JSON.parse(bytes.toString());
    validateRecord(saved, tag, commit);
    const names = [...Object.keys(saved.artifacts), "SHA256SUMS"];
    const assets = names.map(name => remote.assets.find(asset => asset.name === name));
    if (assets.every(asset => asset !== undefined)) {
      await writeFile(join(directory, "release-record.json"), bytes);
      for (const entry of assets) await writeFile(join(directory, entry.name), await asset(entry.id));
    } else {
      // Recover interrupted uploads from the complete candidate retained before staging.
      await gh(["run", "download", saved.run, "--name", "release-candidate", "--dir", directory]);
      if (!bytes.equals(await readFile(join(directory, "release-record.json")))) throw new Error("Candidate record differs from GitHub");
    }
    await check(saved);
    resumed = true;
  } else {
    // A run can stop after retaining the candidate but before creating its draft.
    const runID = process.env.GITHUB_RUN_ID;
    if (!runID || !/^\d+$/.test(runID)) throw new Error("GITHUB_RUN_ID is required");
    const artifacts = JSON.parse(await gh(["api", "--paginate", "--slurp", `repos/${repository}/actions/runs/${runID}/artifacts`]));
    const candidate = artifacts.flatMap((page: { artifacts: { name: string; expired: boolean }[] }) => page.artifacts).find((a: { name: string }) => a.name === "release-candidate");
    if (candidate) {
      if (candidate.expired) throw new Error("The retained release candidate expired; recover its exact artifacts before resuming");
      await gh(["run", "download", runID, "--name", "release-candidate", "--dir", directory]);
      await check(await record());
      resumed = true;
    }
  }
  if (process.env.GITHUB_OUTPUT) await appendFile(process.env.GITHUB_OUTPUT, `resumed=${resumed}\n`);
} else if (action === "stage") {
  const saved = await record();
  await check(saved);
  let remote = await remoteRelease();
  if (!remote) {
    const notes = join(directory, "release-notes.md");
    const baseline = version === "1.0.0"
      ? "\nThis is the first supported release of the new hitSlop document format. Earlier Mac builds and documents were prelaunch experiments and are unsupported; recreate those documents from the included templates. Documents written by this release remain supported by future hitSlop versions.\n"
      : "";
    await writeFile(notes, `hitSlop ${version} for Apple silicon Macs and hitslop ${version} for authoring and document commands.\n\nSigned and notarized by Apple. Requires macOS 15.2 or newer.\n\nInstall the matching CLI and SDK with \`bun install -g hitslop@${version}\`. The single \`hitslop\` package replaces \`@hitslop/cli\`, \`@hitslop/document\`, and \`@hitslop/schema\`.\n${baseline}`);
    await gh(["release", "create", tag, "--draft", "--verify-tag", "--title", `hitSlop ${version}`, "--notes-file", notes]);
    remote = await remoteRelease();
  }
  if (!remote) throw new Error("Draft release was not found after creation");
  for (const name of ["release-record.json", ...Object.keys(saved.artifacts), "SHA256SUMS"]) {
    const existing = remote.assets.find(a => a.name === name);
    if (existing) {
      const expected = await fileDigest(join(directory, name));
      const actual = existing.digest?.startsWith("sha256:") ? existing.digest.slice(7) : createHash("sha256").update(await asset(existing.id)).digest("hex");
      if (actual !== expected) throw new Error(`Existing GitHub artifact differs: ${name}`);
    } else await gh(["release", "upload", tag, join(directory, name)]);
  }
} else if (action === "npm") {
  const saved = await record();
  await check(saved);
  const tarball = join(directory, `hitslop-${version}.tgz`);
  const integrity = "sha512-" + createHash("sha512").update(await readFile(tarball)).digest("base64");
  const response = await fetch(`https://registry.npmjs.org/hitslop/${version}`);
  if (response.ok) {
    const published = await response.json() as { dist: { integrity: string } };
    if (published.dist.integrity !== integrity) throw new Error("npm already has different bytes for this version");
  } else if (response.status === 404) {
    // A unique tag makes resuming an older run unable to roll back npm's latest tag.
    await run(["npm", "publish", tarball, "--access", "public", "--tag", `release-${version}`]);
  } else throw new Error(`Cannot check npm publication: ${response.status}`);
} else if (action === "promote") {
  const saved = await record();
  await check(saved);
  const promote = await mayPromote(saved);
  if (promote) {
    await run(["npm", "dist-tag", "add", `hitslop@${version}`, "latest"]);
    // Publishing this release exposes its recorded appcast at releases/latest.
    await gh(["release", "edit", tag, "--draft=false", "--latest"]);
  }
  if (process.env.GITHUB_OUTPUT) await appendFile(process.env.GITHUB_OUTPUT, `promoted=${promote}\n`);
} else throw new Error("Expected resume, stage, npm or promote");
