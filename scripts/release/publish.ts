/** Publication consumes one retained candidate. Each step reads remote state, verifies
 * existing bytes and resumes without rebuilding, recapturing or replacing an asset. */
import { createHash } from "node:crypto";
import { mkdir, readFile, writeFile, appendFile } from "node:fs/promises";
import { join } from "node:path";
import { exec, run } from "../../packages/hitslop/src/cli/process";
import { fileDigest, sha256 } from "../lib/artifacts";
import { assetBytes, requiredAsset, releaseRepository, type PublishedRelease } from "../lib/release-assets";
import { publishedHistory, verifyHistory } from "../compat/corpora";
import { assertQualified, preservesLatest, validateApproval, validateCandidate, validateRecord, verifyArtifacts, type ReleaseRecord } from "./record";
import { checkReleaseHistory } from "./preflight";
import { releaseNotesNames } from "./notes";
import { ensureDraftRelease, findRelease } from "./github";

const directory = "dist/macos";
const tag = process.env.HITSLOP_RELEASE_TAG!;
const commit = process.env.GITHUB_SHA!;
if (!/^v\d+\.\d+\.\d+$/.test(tag ?? "") || !commit) throw new Error("Expected HITSLOP_RELEASE_TAG and GITHUB_SHA");
if (process.env.GITHUB_REPOSITORY?.toLowerCase() !== releaseRepository.toLowerCase()) throw new Error("Unexpected release repository");
const version = tag.slice(1);
const gh = (args: string[]) => run(["gh", ...args]);
const api = (path: string) => gh(["api", `repos/${releaseRepository}/${path}`]).then(text => JSON.parse(text));
const remoteRelease = () => findRelease(tag);
async function record(): Promise<ReleaseRecord> { return JSON.parse(await readFile(join(directory, "release-record.json"), "utf8")); }
async function check() {
  const saved = await record();
  validateRecord(saved, tag, commit);
  if (saved.run !== process.env.GITHUB_RUN_ID || !process.env.HITSLOP_CANDIDATE_SHA || await fileDigest(join(directory, "release-record.json")) !== process.env.HITSLOP_CANDIDATE_SHA)
    throw new Error("Candidate record differs from the approved candidate job");
  const workflow = await api(`actions/runs/${saved.run}`);
  const jobs = await api(`actions/runs/${saved.run}/attempts/${saved.attempt}/jobs?per_page=100`);
  validateCandidate(saved, workflow, jobs.jobs);
  // Candidates are created only on attempt 1, so a review cannot authorize rebuilt bytes.
  validateApproval(await api(`actions/runs/${saved.run}/approvals`));
  await verifyArtifacts(saved, directory);
  return saved;
}
async function compareRemote(remote: PublishedRelease, saved: ReleaseRecord) {
  for (const name of ["release-record.json", ...Object.keys(saved.artifacts), "SHA256SUMS"]) {
    const asset = requiredAsset(remote, name);
    const expected = await fileDigest(join(directory, name));
    const actual = asset.digest?.startsWith("sha256:") ? asset.digest.slice(7) : sha256(await assetBytes(asset.id));
    if (actual !== expected) throw new Error(`Existing GitHub artifact differs: ${name}`);
  }
}
async function qualification(saved: ReleaseRecord) {
  const entries = await publishedHistory();
  // A retry after locking our own release must still verify that exact record.
  const own = entries.find(entry => entry.record.macVersion === version);
  if (own) await compareRemote(own.release, saved);
  const inventory = verifyHistory(entries);
  delete inventory[version];
  assertQualified(saved.qualified, inventory);
  await checkReleaseHistory(version, saved.macBuild, saved);
}

const action = process.argv[2];
await mkdir(directory, { recursive: true });
if (action === "restore") {
  const remote = await remoteRelease();
  const manifest = remote?.assets.find(asset => asset.name === "release-record.json");
  let restored = false;
  if (remote && manifest) {
    const bytes = await assetBytes(manifest.id);
    if (sha256(bytes) !== process.env.HITSLOP_CANDIDATE_SHA) throw new Error("Published candidate record mismatch");
    const saved: ReleaseRecord = JSON.parse(bytes.toString());
    validateRecord(saved, tag, commit);
    const names = [...Object.keys(saved.artifacts), "SHA256SUMS"];
    if (names.every(name => remote.assets.some(asset => asset.name === name))) {
      await writeFile(join(directory, "release-record.json"), bytes);
      for (const name of names) await writeFile(join(directory, name), await assetBytes(requiredAsset(remote, name).id));
      restored = true;
    }
  }
  if (!restored) {
    const id = process.env.HITSLOP_CANDIDATE_ID;
    if (!/^\d+$/.test(id ?? "")) throw new Error("Candidate artifact ID is required");
    const artifact = await api(`actions/artifacts/${id}`);
    if (artifact.expired || String(artifact.workflow_run.id) !== process.env.GITHUB_RUN_ID || artifact.workflow_run.head_sha !== commit)
      throw new Error("Candidate artifact expired or belongs to another run/commit");
    await gh(["run", "download", process.env.GITHUB_RUN_ID!, "--name", artifact.name, "--dir", directory]);
  }
  await check();
} else if (action === "stage") {
  const saved = await check();
  await qualification(saved);
  const ref = await exec(["gh", "api", `repos/${releaseRepository}/git/ref/tags/${tag}`]);
  if (ref.code) {
    if (!ref.stderr.includes("HTTP 404")) throw new Error(ref.stderr);
    await gh(["api", "--method", "POST", `repos/${releaseRepository}/git/refs`, "-f", `ref=refs/tags/${tag}`, "-f", `sha=${commit}`]);
  } else if (JSON.parse(ref.stdout).object.sha !== commit) throw new Error("Existing release tag points to a different commit");
  const remote = await ensureDraftRelease(tag, join(directory, releaseNotesNames(version).github));
  for (const name of ["release-record.json", ...Object.keys(saved.artifacts), "SHA256SUMS"]) {
    const existing = remote.assets.find(asset => asset.name === name);
    if (existing) {
      if (sha256(await assetBytes(existing.id)) !== await fileDigest(join(directory, name))) throw new Error(`Existing GitHub artifact differs: ${name}`);
    } else await gh(["release", "upload", tag, join(directory, name)]);
  }
  await compareRemote((await remoteRelease())!, saved);
} else if (action === "lock") {
  const saved = await check();
  const remote = await remoteRelease();
  if (!remote) throw new Error("Stage the complete draft first");
  await compareRemote(remote, saved);
  if (remote.draft) await gh(["release", "edit", tag, "--draft=false", "--latest=false"]);
  if (!(await remoteRelease())?.immutable) throw new Error("Published release is not immutable; enable immutable releases before publication");
} else if (action === "npm") {
  const saved = await check();
  const remote = await remoteRelease();
  if (!remote?.immutable || remote.draft) throw new Error("Permanent release evidence must be public before npm publication");
  await compareRemote(remote, saved);
  const tarball = join(directory, `hitslop-${version}.tgz`);
  const integrity = "sha512-" + createHash("sha512").update(await readFile(tarball)).digest("base64");
  if (integrity !== saved.npmIntegrity) throw new Error("Retained npm integrity differs from the release record");
  const response = await fetch(`https://registry.npmjs.org/hitslop/${version}`);
  if (response.ok) {
    const published = await response.json() as { dist: { integrity: string } };
    if (published.dist.integrity !== integrity) throw new Error("npm already has different bytes for this version");
  } else if (response.status === 404) {
    await run(["npm", "publish", tarball, "--access", "public", "--tag", `release-${version}`]);
  } else throw new Error(`Cannot check npm publication: ${response.status}`);
} else if (action === "promote") {
  const saved = await check();
  await qualification(saved);
  const response = await fetch("https://registry.npmjs.org/hitslop/latest");
  if (response.ok && !preservesLatest(version, (await response.json() as { version: string }).version)) throw new Error("Cannot move npm latest backwards");
  if (!response.ok && response.status !== 404) throw new Error(`Cannot check npm latest: ${response.status}`);
  await run(["npm", "dist-tag", "add", `hitslop@${version}`, "latest"]);
  await gh(["release", "edit", tag, "--latest"]);
  if (process.env.GITHUB_OUTPUT) await appendFile(process.env.GITHUB_OUTPUT, "promoted=true\n");
} else if (action === "check") {
  await qualification(await check());
} else throw new Error("Expected restore, check, stage, lock, npm or promote");
