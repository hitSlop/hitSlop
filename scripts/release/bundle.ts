/** Retain the tested npm artifacts (`bun run packages:pack`, `generated/npm`) with a Mac
 * release. Each package's file engines must come from the candidate the corpus captured. */
import { mkdir, readFile, cp, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { fileDigest, repository, verifyShellCopies } from "../lib/artifacts";
import { releases } from "../compat/corpus";
import { verifyCandidate, verifyCorpus } from "../compat/integrity";
import { enginePlatforms } from "../build/core";
import { exec, run } from "../../packages/hitslop/src/cli/process";

const project = await readFile(join(repository, "apps/apple/project.yml"), "utf8");
const version = project.match(/MARKETING_VERSION: "([^"]+)"/)?.[1];
const build = project.match(/CURRENT_PROJECT_VERSION: "([^"]+)"/)?.[1];
if (!version || !build || process.env.GITHUB_REF_NAME !== `v${version}`)
  throw new Error("Release tag must match the Apple project marketing version");
const metadata = await Bun.file(join(repository, "packages/hitslop/package.json")).json();
if (metadata.version !== version) throw new Error("Mac and npm release versions differ");
const packageVersions = { hitslop: version };
if (process.argv.includes("--preflight")) process.exit(0);
const entry = (await releases()).find(entry => entry.name === version && entry.release.frozen);
if (!entry) throw new Error("Missing frozen release corpus");
await verifyCorpus(entry.root, entry.release);
await verifyCandidate(entry.root, entry.release);
const shell = await verifyShellCopies();
const output = join(repository, "dist/macos");
await mkdir(output, { recursive: true });
const retained: string[] = [];
const file = `hitslop-${version}.tgz`;
await cp(join(repository, "generated/npm", file), join(output, file));
retained.push(file);
const commit = (await run(["git", "rev-parse", "HEAD"], { cwd: repository, failure: "Cannot identify release commit" })).trim();
// The published CLI carries the file engine for every supported platform, each built from
// the producing candidate's commit and core.
const cliPackage = join(output, `hitslop-${version}.tgz`);
const packed = await run(["/usr/bin/tar", "-tzf", cliPackage], { failure: "Cannot list the CLI package" });
for (const platform of enginePlatforms) {
  if (!packed.includes(`package/engine/${platform}/slop-engine`))
    throw new Error(`The CLI package has no file engine for ${platform}; capture with the engines workflow's artifacts in generated/engines`);
  const read = await exec(["/usr/bin/tar", "-xOzf", cliPackage, `package/engine/${platform}/engine.json`]);
  const recorded = JSON.parse(read.stdout || "{}");
  if (read.code || recorded.commit !== commit || recorded.buildId !== entry.release.producer.coreBuildID)
    throw new Error(`The CLI package's ${platform} engine was not built from the release candidate (${entry.release.commit}, core ${entry.release.producer.coreBuildID})`);
}
const hashes: Record<string, string> = {};
for (const line of (await readFile(join(output, "SHA256SUMS"), "utf8")).trim().split("\n")) {
  const [hash, file] = line.split("  ");
  if (hash && file && file !== "release-record.json") hashes[file] = hash;
}
for (const file of retained) hashes[file] = await fileDigest(join(output, file));
await writeFile(join(output, "release-record.json"), JSON.stringify({ commit, run: process.env.GITHUB_RUN_ID, tag: process.env.GITHUB_REF_NAME, macVersion: version, macBuild: build, packageVersions, shell, artifacts: hashes }, null, 2) + "\n");
hashes["release-record.json"] = await fileDigest(join(output, "release-record.json"));
const sums = await readFile(join(output, "SHA256SUMS"), "utf8");
const prior = sums.split("\n").filter(line => line && !Object.keys(hashes).some(file => line.endsWith(`  ${file}`)));
await writeFile(join(output, "SHA256SUMS"), prior.join("\n") + "\n" + Object.entries(hashes).map(([file, hash]) => `${hash}  ${file}\n`).join(""));
console.log(`Retained tested artifacts for ${commit}`);
