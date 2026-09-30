/** Retain the exact tested npm artifacts with a Mac release. */
import { createHash } from "node:crypto";
import { mkdir, readFile, cp, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { repository, verifyShellCopies } from "./runtime-artifacts";

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
const shell = await verifyShellCopies();
const output = join(repository, "dist/macos");
await mkdir(output, { recursive: true });
const retained: string[] = [];
for (const name of ["schema", "document", "cli"]) {
  const file = `hitslop-${name}-${packageVersions[name]}.tgz`;
  await cp(join(repository, "generated/npm", file), join(output, file));
  retained.push(file);
}
const git = Bun.spawn(["git", "rev-parse", "HEAD"], { cwd: repository, stdout: "pipe" });
const commit = (await new Response(git.stdout).text()).trim();
if (await git.exited) throw new Error("Cannot identify release commit");
const hashes: Record<string, string> = {};
for (const file of retained) hashes[file] = createHash("sha256").update(await readFile(join(output, file))).digest("hex");
await writeFile(join(output, "release-record.json"), JSON.stringify({ commit, tag: process.env.GITHUB_REF_NAME, macVersion: version, macBuild: build, packageVersions, shell, artifacts: hashes }, null, 2) + "\n");
hashes["release-record.json"] = createHash("sha256").update(await readFile(join(output, "release-record.json"))).digest("hex");
const sums = await readFile(join(output, "SHA256SUMS"), "utf8");
const prior = sums.split("\n").filter(line => line && !Object.keys(hashes).some(file => line.endsWith(`  ${file}`)));
await writeFile(join(output, "SHA256SUMS"), prior.join("\n") + "\n" + Object.entries(hashes).map(([file, hash]) => `${hash}  ${file}\n`).join(""));
console.log(`Retained tested artifacts for ${commit}`);
