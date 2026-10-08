import { join } from "node:path";
import { fileDigest } from "../lib/artifacts";

export type ReleaseRecord = {
  commit: string; tag: string; macVersion: string; macBuild: string; run: string;
  artifacts: Record<string, string>;
};

export function validateRecord(record: ReleaseRecord, tag: string, commit: string) {
  if (record.tag !== tag || record.macVersion !== tag.slice(1) || record.commit !== commit)
    throw new Error("Recorded release is from a different tag or commit");
  if (!/^\d+$/.test(record.macBuild) || !/^\d+$/.test(record.run))
    throw new Error("Invalid build number or candidate workflow run");
  for (const [name, hash] of Object.entries(record.artifacts)) {
    if (!/^[a-zA-Z0-9][a-zA-Z0-9._-]*$/.test(name) || !/^[a-f0-9]{64}$/.test(hash))
      throw new Error("Invalid artifact record");
  }
  for (const name of [`hitslop-${record.macVersion}.tgz`, "hitSlop.dmg", "hitSlop.zip", "appcast.xml"])
    if (!record.artifacts[name]) throw new Error(`Release record is missing ${name}`);
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
  if (![candidate, latest].every(version => /^\d+\.\d+\.\d+$/.test(version)))
    throw new Error("Expected stable npm release versions");
  const a = candidate.split(".").map(BigInt), b = latest.split(".").map(BigInt);
  for (let index = 0; index < 3; index++) if (a[index] !== b[index]) return a[index]! > b[index]!;
  return true;
}
