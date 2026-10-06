import { strict as assert } from "node:assert";
import { createHash } from "node:crypto";
import { lstat, readdir, readFile, mkdtemp, rm } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { digest, fileDigest, repository } from "./runtime-artifacts";
import type { Release } from "./compat";

/** Fingerprint producing inputs, excluding the corpus-only commit and build outputs. */
export async function sourceFingerprint(root = repository): Promise<string> {
  const git = Bun.spawn(["git", "ls-files", "--cached", "--others", "--exclude-standard", "-z"], { cwd: root, stdout: "pipe", stderr: "pipe" });
  const [out, error, code] = await Promise.all([new Response(git.stdout).text(), new Response(git.stderr).text(), git.exited]);
  assert.equal(code, 0, error);
  const inputs = [...new Set(out.split("\0"))].filter(path =>
    /^(Cargo\.(toml|lock)|rust-toolchain\.toml|bun\.lock|package\.json|tsconfig[^/]*\.json)$/.test(path) ||
    /^(crates|scripts|apps\/apple|examples\/slops|tests\/abi|tests\/fixtures)\//.test(path) ||
    /^packages\/(?:document|shell|schema|cli)\/(?:src\/|templates\/|skills\/|package\.json$)/.test(path),
  ).sort();
  const hash = createHash("sha256");
  for (const path of inputs) {
    // A tracked file deleted in the working tree is not an input.
    const bytes = await readFile(join(root, path)).catch((error) => {
      if (error.code === "ENOENT") return undefined;
      throw error;
    });
    if (!bytes) continue;
    hash.update(JSON.stringify(path));
    hash.update(createHash("sha256").update(bytes).digest());
  }
  return hash.digest("hex");
}


/** Path + bytes, never tar timestamps or compression metadata. */
export async function archiveDigest(archive: string): Promise<string> {
  const root = await mkdtemp(join(tmpdir(), "hitslop-archive-"));
  try {
    const child = Bun.spawn(["/usr/bin/tar", "-xzf", archive, "-C", root], { stdout: "pipe", stderr: "pipe" });
    const error = await new Response(child.stderr).text();
    assert.equal(await child.exited, 0, error);
    return await digest(join(root, "package"));
  } finally { await rm(root, { recursive: true, force: true }); }
}

export async function corpusFiles(root: string): Promise<Record<string, string>> {
  const files: Record<string, string> = {};
  async function visit(relative: string) {
    const path = join(root, relative), info = await lstat(path);
    assert.ok(!info.isSymbolicLink(), `Symlink in corpus: ${relative}`);
    if (info.isDirectory()) {
      for (const name of (await readdir(path)).sort()) await visit(relative ? `${relative}/${name}` : name);
    } else {
      assert.ok(info.isFile(), `Invalid corpus file: ${relative}`);
      if (relative !== "release.json") files[relative] = await fileDigest(path);
    }
  }
  await visit("");
  return files;
}

export async function verifyCorpus(root: string, release: Release) {
  assert.deepEqual(await corpusFiles(root), release.files, `${release.release}: corpus files changed or are missing`);
  assert.ok(Object.keys(release.storage).length > 0, "No saved documents");
  for (const name of Object.keys(release.storage)) {
    for (const path of [`documents/${name}.slop`, `expected/${name}.json`, `scenarios/${name}.json`])
      assert.ok(release.files[path], `${release.release}: missing ${path}`);
    if (name !== "conformance-anomalies") assert.ok(release.files[`pages/${name}.json`], `Missing page scenario: ${name}`);
  }
  for (const name of ["conformance", "conformance-compacted", "conformance-anomalies", "fixture-checklist", "fixture-scalars", "fixture-collections"])
    assert.ok(release.storage[name], `Missing required conformance case: ${name}`);
  for (const path of ["cli/transcript.json", "cli/install/package.json", "cli/install/bun.lock"])
    assert.ok(release.files[path], `Missing ${path}`);
  for (const [slug, hash] of Object.entries(release.templates))
    assert.equal(await fileDigest(join(root, "templates", `${slug}.slop`)), hash, `Changed template: ${slug}`);
}

/** Only the release being published must match current producing inputs. */
export async function verifyCandidate(root: string, release: Release) {
  assert.equal(await sourceFingerprint(), release.inputs, "Release inputs changed after compatibility capture; capture a new candidate");
  for (const [slug, hash] of Object.entries(release.templates)) {
    if (slug === "conformance" || slug.startsWith("fixture-")) continue;
    assert.equal(await fileDigest(join(repository, "generated/templates", `${slug}.slop`)), hash, `Captured template differs from candidate: ${slug}`);
  }
  for (const [file, hash] of Object.entries(release.archives))
    assert.equal(await archiveDigest(join(repository, "generated/npm", file)), hash, `Captured npm contents differ from candidate: ${file}`);
}
