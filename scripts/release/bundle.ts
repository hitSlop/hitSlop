/** Retain the tested npm artifacts (`bun run packages:pack`, `generated/npm`) with a Mac
 * release. Each package's file engines must come from the candidate the corpus captured. */
import { mkdir, readFile, cp, writeFile, appendFile } from "node:fs/promises";
import { join } from "node:path";
import { createHash } from "node:crypto";
import { fileDigest, repository, verifyShellCopies } from "../lib/artifacts";
import { releases } from "../compat/corpus";
import { verifyResolvedCorpora } from "../compat/corpora";
import { verifyCorpus } from "../compat/integrity";
import { verifyPackedWriter } from "../compat/writers";
import { enginePlatforms } from "../build/core";
import { projectVersion } from "./preflight";
import { validateRecord, type ReleaseRecord } from "./record";
import { exec, run } from "../../packages/tinys/src/cli/process";

const { version, build, tag } = await projectVersion();
await verifyResolvedCorpora(process.env.TINYS_COMPAT_ROOT!);
const entry = (await releases()).find(entry => entry.name === version && entry.release.frozen);
if (!entry) throw new Error("Missing frozen release corpus");
await verifyCorpus(entry.root, entry.release);
await verifyShellCopies();
const output = join(repository, "dist/macos");
await mkdir(output, { recursive: true });
const retained: string[] = [];
const file = `hitslop-tinys-${version}.tgz`;
await cp(join(repository, "generated/npm", file), join(output, file));
retained.push(file);
const commit = (await run(["git", "rev-parse", "HEAD"], { cwd: repository, failure: "Cannot identify release commit" })).trim();
// The published CLI carries the file engine for every supported platform, each built from
// the producing candidate's commit and core.
const cliPackage = join(output, `hitslop-tinys-${version}.tgz`);
await verifyPackedWriter(cliPackage, entry.release.writer);
const packed = await run(["/usr/bin/tar", "-tzf", cliPackage], { failure: "Cannot list the CLI package" });
for (const platform of enginePlatforms) {
  if (!packed.includes(`package/engine/${platform}/tiny-engine`))
    throw new Error(`The CLI package has no file engine for ${platform}; capture with the engines workflow's artifacts in generated/engines`);
  const read = await exec(["/usr/bin/tar", "-xOzf", cliPackage, `package/engine/${platform}/engine.json`]);
  const recorded = JSON.parse(read.stdout || "{}");
  if (read.code || recorded.commit !== commit || recorded.buildId !== entry.release.producer.coreBuildID)
    throw new Error(`The CLI package's ${platform} engine was not built from the release candidate (${entry.release.commit}, core ${entry.release.producer.coreBuildID})`);
}
if (commit !== process.env.GITHUB_SHA || entry.release.commit !== commit) throw new Error("Candidate commit mismatch");
const corpusFile = `compat-corpus-${version}.tar.gz`;
await run(["tar", "--format=ustar", "-czf", join(output, corpusFile), "-C", entry.root, "."], { env: { ...process.env, COPYFILE_DISABLE: "1" } });
retained.push(corpusFile);
const receipt = await Bun.file(join(process.env.TINYS_COMPAT_ROOT!, ".inventory.json")).json();
if (receipt.offline) throw new Error("Offline history cannot qualify a release");
const hashes: Record<string, string> = {};
for (const line of (await readFile(join(output, "SHA256SUMS"), "utf8")).trim().split("\n")) {
  const [hash, file] = line.split("  ");
  if (hash && file && file !== "release-record.json") hashes[file] = hash;
}
for (const file of retained) hashes[file] = await fileDigest(join(output, file));
const record: ReleaseRecord = { commit, run: process.env.GITHUB_RUN_ID!, attempt: process.env.GITHUB_RUN_ATTEMPT!, tag, macVersion: version, macBuild: build,
  npmIntegrity: "sha512-" + createHash("sha512").update(await readFile(cliPackage)).digest("base64"),
  corpus: { file: corpusFile, sha256: hashes[corpusFile]! }, qualified: receipt.inventory, artifacts: hashes };
validateRecord(record, tag, commit);
await writeFile(join(output, "release-record.json"), JSON.stringify(record, null, 2) + "\n");
if (process.env.GITHUB_OUTPUT) await appendFile(process.env.GITHUB_OUTPUT, `record-sha=${await fileDigest(join(output, "release-record.json"))}\n`);
hashes["release-record.json"] = await fileDigest(join(output, "release-record.json"));
const sums = await readFile(join(output, "SHA256SUMS"), "utf8");
const prior = sums.split("\n").filter(line => line && !Object.keys(hashes).some(file => line.endsWith(`  ${file}`)));
await writeFile(join(output, "SHA256SUMS"), prior.join("\n") + "\n" + Object.entries(hashes).map(([file, hash]) => `${hash}  ${file}\n`).join(""));
console.log(`Retained tested artifacts for ${commit}`);
