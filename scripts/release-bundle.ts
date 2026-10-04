/** Retain the exact tested npm artifacts with a Mac release. */
import { mkdir, readFile, cp, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { fileDigest, repository, verifyShellCopies } from "./runtime-artifacts";
import { releases } from "./compat";
import { verifyCandidate, verifyCorpus } from "./compat-integrity";
import { enginePlatforms } from "./core-build";

const project = await readFile(join(repository, "apps/apple/project.yml"), "utf8");
const version = project.match(/MARKETING_VERSION: "([^"]+)"/)?.[1];
const build = project.match(/CURRENT_PROJECT_VERSION: "([^"]+)"/)?.[1];
if (!version || !build || process.env.GITHUB_REF_NAME !== `macos-v${version}`)
  throw new Error("Release tag must match the Apple project marketing version");
const packageVersions: Record<string, string> = {};
for (const name of ["schema", "document", "cli"]) {
  const metadata = await Bun.file(join(repository, "packages", name, "package.json")).json();
  packageVersions[name] = metadata.version;
  for (const dependency of name === "cli" ? ["document", "schema"] : name === "document" ? ["schema"] : []) {
    const target = await Bun.file(join(repository, "packages", dependency, "package.json")).json();
    if (metadata.dependencies[`@hitslop/${dependency}`] !== target.version)
      throw new Error(`Package dependency mismatch: ${name} -> ${dependency}`);
  }
}
if (process.argv.includes("--preflight")) process.exit(0);
const entry = (await releases()).find(entry => entry.name === version && entry.release.frozen);
if (!entry) throw new Error("Missing frozen release corpus");
await verifyCorpus(entry.root, entry.release);
await verifyCandidate(entry.root, entry.release);
const shell = await verifyShellCopies();
const output = join(repository, "dist/macos");
await mkdir(output, { recursive: true });
const retained: string[] = [];
for (const name of ["schema", "document", "cli"]) {
  const file = `hitslop-${name}-${packageVersions[name]}.tgz`;
  await cp(join(entry.root, "cli", file), join(output, file));
  retained.push(file);
}
// The published CLI carries the file engine for every supported platform, each built from
// the producing candidate's commit and core.
const cliPackage = join(output, `hitslop-cli-${packageVersions.cli}.tgz`);
const listing = Bun.spawn(["/usr/bin/tar", "-tzf", cliPackage], { stdout: "pipe" });
const packed = await new Response(listing.stdout).text();
if (await listing.exited) throw new Error("Cannot list the CLI package");
for (const platform of enginePlatforms) {
  if (!packed.includes(`package/engine/${platform}/slop-engine`))
    throw new Error(`The CLI package has no file engine for ${platform}; capture with the engines workflow's artifacts in generated/engines`);
  const read = Bun.spawn(["/usr/bin/tar", "-xOzf", cliPackage, `package/engine/${platform}/engine.json`], { stdout: "pipe" });
  const recorded = JSON.parse((await new Response(read.stdout).text()) || "{}");
  if ((await read.exited) || recorded.commit !== entry.release.commit || recorded.buildId !== entry.release.producer.coreBuildID)
    throw new Error(`The CLI package's ${platform} engine was not built from the release candidate (${entry.release.commit}, core ${entry.release.producer.coreBuildID})`);
}
const git = Bun.spawn(["git", "rev-parse", "HEAD"], { cwd: repository, stdout: "pipe" });
const commit = (await new Response(git.stdout).text()).trim();
if (await git.exited) throw new Error("Cannot identify release commit");
const hashes: Record<string, string> = {};
for (const file of retained) hashes[file] = await fileDigest(join(output, file));
await writeFile(join(output, "release-record.json"), JSON.stringify({ commit, tag: process.env.GITHUB_REF_NAME, macVersion: version, macBuild: build, packageVersions, shell, artifacts: hashes }, null, 2) + "\n");
hashes["release-record.json"] = await fileDigest(join(output, "release-record.json"));
const sums = await readFile(join(output, "SHA256SUMS"), "utf8");
const prior = sums.split("\n").filter(line => line && !Object.keys(hashes).some(file => line.endsWith(`  ${file}`)));
await writeFile(join(output, "SHA256SUMS"), prior.join("\n") + "\n" + Object.entries(hashes).map(([file, hash]) => `${hash}  ${file}\n`).join(""));
console.log(`Retained tested artifacts for ${commit}`);
