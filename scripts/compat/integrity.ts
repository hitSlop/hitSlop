import { strict as assert } from "node:assert";
import { createHash } from "node:crypto";
import { lstat, readdir, readFile } from "node:fs/promises";
import { join } from "node:path";
import { fileDigest, repository } from "../lib/artifacts";
import { readJSON, type Expected, type Release } from "./corpus";

/** Fingerprint producing inputs, excluding the corpus-only commit and build outputs. */
export async function sourceFingerprint(root = repository): Promise<string> {
  const git = Bun.spawn(["git", "ls-files", "--cached", "--others", "--exclude-standard", "-z"], {
    cwd: root,
    stdout: "pipe",
    stderr: "pipe",
  });
  const [out, error, code] = await Promise.all([
    new Response(git.stdout).text(),
    new Response(git.stderr).text(),
    git.exited,
  ]);
  assert.equal(code, 0, error);
  const inputs = [...new Set(out.split("\0"))]
    .filter(
      (path) =>
        /^(Cargo\.(toml|lock)|rust-toolchain\.toml|bun\.lock|package\.json|tsconfig[^/]*\.json)$/.test(
          path,
        ) ||
        /^(crates|scripts|apps\/apple|tests\/abi|tests\/apps|tests\/presentation)\//.test(path) ||
        /^packages\/(?:hitslop)\/(?:src\/|generated\/|acceptance\/|templates\/|skills\/|package\.json$)/.test(
          path,
        ),
    )
    .sort();
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

export async function corpusFiles(root: string): Promise<Record<string, string>> {
  const files: Record<string, string> = {};
  async function visit(relative: string) {
    const path = join(root, relative),
      info = await lstat(path);
    assert.ok(!info.isSymbolicLink(), `Symlink in corpus: ${relative}`);
    if (info.isDirectory()) {
      for (const name of (await readdir(path)).sort())
        await visit(relative ? `${relative}/${name}` : name);
    } else {
      assert.ok(info.isFile(), `Invalid corpus file: ${relative}`);
      if (relative !== "release.json") files[relative] = await fileDigest(path);
    }
  }
  await visit("");
  return files;
}

export async function verifyCorpus(root: string, release: Release) {
  assert.ok(
    release.storageSchemas && Object.keys(release.storageSchemas).length >= 1,
    "Missing storage schema records",
  );
  if (release.frozen) {
    for (const [path, hash] of Object.entries(release.storageSchemas)) {
      assert.match(
        path,
        /^crates\/hitslop-core\/src\/file\/storage-\d+\.sql$/,
        `Not a storage schema: ${path}`,
      );
      assert.equal(
        await fileDigest(join(repository, path)),
        hash,
        `Changed released storage schema: ${path}`,
      );
    }
  }
  assert.ok(
    Array.isArray(release.limits) && release.limits.length > 0,
    "Missing open limits; recapture the pre-launch dev entry",
  );
  const limitNames = new Set<string>();
  for (const limit of release.limits) {
    assert.ok(!limitNames.has(limit.name), `Duplicate limit: ${limit.name}`);
    limitNames.add(limit.name);
    assert.ok(
      ["Max", "Min", "Exact"].includes(limit.kind),
      `Invalid limit direction: ${limit.name}`,
    );
    assert.ok(
      typeof limit.value === "string" &&
        /^(0|[1-9][0-9]*)$/.test(limit.value) &&
        BigInt(limit.value) <= 0xffffffffffffffffn,
      `Invalid limit value: ${limit.name}`,
    );
  }
  assert.ok(
    Object.keys(release.files).some((path) => /^acceptance\/[^/]+\.json$/.test(path)),
    "Missing acceptance vectors; recapture the pre-launch dev entry",
  );
  assert.deepEqual(
    await corpusFiles(root),
    release.files,
    `${release.release}: corpus files changed or are missing`,
  );
  assert.ok(Object.keys(release.storage).length > 0, "No saved documents");
  for (const name of Object.keys(release.storage)) {
    for (const path of [
      `documents/${name}.slop`,
      `expected/${name}.json`,
      `scenarios/${name}.json`,
    ])
      assert.ok(release.files[path], `${release.release}: missing ${path}`);
    // A document a page saved (`NAME.page`) was the result of NAME's page scenario.
    if (!name.endsWith(".page"))
      assert.ok(release.files[`pages/${name}.json`], `Missing page scenario: ${name}`);
    const expected = await readJSON<Expected>(join(root, "expected", `${name}.json`));
    assert.match(expected?.appDigest ?? "", /^[a-f0-9]{64}$/, `Missing appDigest: ${name}`);
  }
  for (const name of ["conformance", "conformance-compacted"])
    assert.ok(release.storage[name], `Missing required conformance case: ${name}`);
  assert.ok(release.writer, `${release.release}: no candidate writer`);
  assert.equal(release.writer.platform, "darwin-arm64", "Invalid writer platform");
  assert.equal(release.writer.version, release.release, "Writer version differs from entry");
  assert.match(release.writer.sha256, /^[a-f0-9]{64}$/, "Invalid writer SHA-256");
  assert.match(release.sourceInputs, /^[a-f0-9]{64}$/, "Missing source fingerprint");
  assert.ok(
    !Object.keys(release.files).some((path) => path.startsWith("engine/")),
    "Writer binaries belong in the hash cache, not the corpus",
  );
  assert.equal(
    release.writer.buildId,
    release.producer.coreBuildID,
    `${release.release}: the writer was built from another core`,
  );
  assert.deepEqual(
    Object.keys(release.templateAppDigests ?? {}).sort(),
    Object.keys(release.templates).sort(),
    "Missing template appDigest records",
  );
  for (const [slug, hash] of Object.entries(release.templates)) {
    assert.match(
      release.templateAppDigests[slug]!,
      /^[a-f0-9]{64}$/,
      `Invalid template appDigest: ${slug}`,
    );
    assert.equal(
      await fileDigest(join(root, "templates", `${slug}.slop`)),
      hash,
      `Changed template: ${slug}`,
    );
  }
}

/** Only the release being published must match current producing inputs. */
export async function verifyCandidate(
  release: Pick<Release, "sourceInputs" | "templates">,
  candidateRoot = repository,
) {
  assert.equal(
    await sourceFingerprint(candidateRoot),
    release.sourceInputs,
    "Release inputs changed after compatibility capture; capture a new candidate",
  );
  for (const [slug, hash] of Object.entries(release.templates)) {
    if (slug === "conformance") continue;
    const file = join(
      candidateRoot,
      "generated/presentation",
      `${slug.slice("presentation-".length)}.slop`,
    );
    assert.equal(await fileDigest(file), hash, `Captured template differs from candidate: ${slug}`);
  }
}
