import { join } from "node:path";
import { fileDigest } from "../lib/artifacts";
import { releaseNotesNames } from "./notes";

export type ReleaseRecord = {
  commit: string; tag: string; macVersion: string; macBuild: string; run: string;
  attempt: string;
  npmIntegrity: string;
  corpus: { file: string; sha256: string };
  qualified: Record<string, string>;
  artifacts: Record<string, string>;
};

export function validateRecord(record: ReleaseRecord, tag: string, commit: string) {
  if (!/^v\d+\.\d+\.\d+$/.test(tag) || !/^[a-f0-9]{40}$/.test(commit)) throw new Error("Invalid release tag or commit");
  if (record.tag !== tag || record.macVersion !== tag.slice(1) || record.commit !== commit)
    throw new Error("Recorded release is from a different tag or commit");
  if (!/^\d+$/.test(record.macBuild) || !/^\d+$/.test(record.run) || !/^[1-9]\d*$/.test(record.attempt))
    throw new Error("Invalid build number or candidate workflow run");
  if (!/^sha512-[A-Za-z0-9+/]{86}==$/.test(record.npmIntegrity)) throw new Error("Invalid npm integrity record");
  for (const [name, hash] of Object.entries(record.artifacts)) {
    if (!/^[a-zA-Z0-9][a-zA-Z0-9._-]*$/.test(name) || !/^[a-f0-9]{64}$/.test(hash))
      throw new Error("Invalid artifact record");
  }
  for (const name of [`hitslop-${record.macVersion}.tgz`, "hitSlop.dmg", "hitSlop.zip", "appcast.xml", `compat-corpus-${record.macVersion}.tar.gz`, ...Object.values(releaseNotesNames(record.macVersion))])
    if (!record.artifacts[name]) throw new Error(`Release record is missing ${name}`);
  if (record.corpus?.file !== `compat-corpus-${record.macVersion}.tar.gz` || record.artifacts[record.corpus.file] !== record.corpus.sha256)
    throw new Error("Invalid corpus record");
  if (!record.qualified || Object.entries(record.qualified).some(([version, hash]) => !/^\d+\.\d+\.\d+$/.test(version) || !/^[a-f0-9]{64}$/.test(hash) || version === record.macVersion))
    throw new Error("Invalid qualified corpus inventory");
}

/** A candidate may only publish against the exact historical bytes it replayed. On
 * retry its own already-published release is checked separately and excluded here. */
export function assertQualified(qualified: Record<string, string>, published: Record<string, string>) {
  for (const version of new Set([...Object.keys(qualified), ...Object.keys(published)]))
    if (qualified[version] !== published[version]) throw new Error(`Published corpus inventory changed: ${version}; run a new candidate`);
}

export function validateCandidate(record: ReleaseRecord, workflow: { id: number; head_sha: string; event: string; path: string }, jobs: { name: string; conclusion: string | null }[]) {
  if (record.attempt !== "1") throw new Error("A rebuilt candidate requires a new workflow dispatch");
  if (String(workflow.id) !== record.run || workflow.head_sha !== record.commit || workflow.event !== "workflow_dispatch" || workflow.path !== ".github/workflows/macos-release.yml")
    throw new Error("Candidate workflow identity mismatch");
  if (!jobs.some(job => job.name === "Build and test candidate" && job.conclusion === "success"))
    throw new Error("Candidate job did not succeed");
}

export function validateApproval(reviews: { state: string; environments: { name: string }[] }[]) {
  if (!reviews.some(review => review.state === "approved" && review.environments.some(environment => environment.name === "release")))
    throw new Error("This workflow run has no approved release review");
}

export function validatePublication(record: ReleaseRecord, state: {
  npm: { version: string; dist: { integrity: string } }; githubTag: string;
}) {
  if (state.npm.version !== record.macVersion || state.npm.dist.integrity !== record.npmIntegrity || state.githubTag !== record.tag)
    throw new Error(`Incomplete publication ${record.tag}; finish publishing its retained artifacts first`);
}

export async function verifyArtifacts(record: ReleaseRecord, directory: string) {
  for (const [name, hash] of Object.entries(record.artifacts))
    if (await fileDigest(join(directory, name)) !== hash) throw new Error(`Artifact differs from release record: ${name}`);
}

export function advancesBuild(candidate: string, previous: string) {
  if (!/^\d+$/.test(candidate) || !/^\d+$/.test(previous)) throw new Error("Invalid Mac build number");
  return BigInt(candidate) > BigInt(previous);
}

/** The prelaunch Mac releases have appcasts but no unified release record. */
export function appcastBuild(xml: string) {
  const versions = [...xml.matchAll(/<sparkle:version>\s*(\d+)\s*<\/sparkle:version>|sparkle:version="(\d+)"/g)]
    .map(match => BigInt(match[1] ?? match[2]!));
  if (!versions.length) throw new Error("Latest appcast has no Mac build number");
  return versions.reduce((a, b) => a > b ? a : b).toString();
}

/** npm promotion can succeed before GitHub does; a retry must respect both. */
export function preservesLatest(candidate: string, latest: string) {
  // npm can assign latest to the bootstrap package before the first stable release.
  const previous = /^(\d+\.\d+\.\d+)(?:-[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?$/.exec(latest);
  if (!/^\d+\.\d+\.\d+$/.test(candidate) || !previous)
    throw new Error("Expected a stable release candidate and valid npm latest version");
  const a = candidate.split(".").map(BigInt), b = previous[1]!.split(".").map(BigInt);
  for (let index = 0; index < 3; index++) if (a[index] !== b[index]) return a[index]! > b[index]!;
  return true;
}
