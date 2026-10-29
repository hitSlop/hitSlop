import { expect, test } from "bun:test";
import { mkdir, mkdtemp, readFile, rm, writeFile, symlink, realpath, stat, chmod, cp } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { exec, run } from "../../packages/tinys/src/cli/process";
import { fileDigest, repository, sha256 } from "../../scripts/lib/artifacts";
import { archiveFiles } from "../../scripts/lib/release-archive";
import { corpusFiles, verifyCorpus } from "../../scripts/compat/integrity";
import { publishedHistory, resolveCorpora, verifyHistory, verifyResolvedCorpora, type HistoricalRelease } from "../../scripts/compat/corpora";
import { copyCorpusDocument, releases, type Release } from "../../scripts/compat/corpus";
import { resolveWriter, verifyPackedWriter } from "../../scripts/compat/writers";

test("corpus discovery permits a missing optional directory but surfaces filesystem errors", async () => {
  const root = await mkdtemp(join(tmpdir(), "tinys-corpus-discovery-"));
  try {
    expect(await releases(join(root, "missing"))).toEqual([]);
    const file = join(root, "not-a-directory");
    await writeFile(file, "invalid corpus root");
    await expect(releases(file)).rejects.toMatchObject({ code: "ENOTDIR" });
  } finally { await rm(root, { recursive: true, force: true }); }
});

async function fixture(root: string, version = "1.0.0", id = 1) {
  const directory = join(root, `original-${version}`);
  for (const dir of ["documents", "expected", "scenarios", "pages", "acceptance"]) await mkdir(join(directory, dir), { recursive: true });
  for (const name of ["conformance", "conformance-compacted"]) {
    await writeFile(join(directory, `documents/${name}.tiny`), "minimal archive fixture");
    await writeFile(join(directory, `expected/${name}.json`), JSON.stringify({ appDigest: "a".repeat(64) }));
    await writeFile(join(directory, `scenarios/${name}.json`), "{}");
    await writeFile(join(directory, `pages/${name}.json`), "{}");
  }
  await writeFile(join(directory, "acceptance/vector.json"), "{}");
  const schema = "crates/tinys-core/src/file/storage-1.sql";
  const release: Release = { release: version, frozen: true, commit: "c".repeat(40), captured: "2026-10-15",
    markers: { packageFormat: 1, runtimeABI: 1, storage: 1, layout: 1, protocol: 1 },
    producer: { coreBuildID: "core" }, files: await corpusFiles(directory), storageSchemas: { [schema]: await fileDigest(join(repository, schema)) },
    limits: [{ name: "example", value: "1", kind: "Max" }], templates: {}, templateAppDigests: {},
    writer: { version, platform: "darwin-arm64", sha256: "a".repeat(64), buildId: "core" }, toolchain: {}, clock: 1,
    storage: { conformance: { checkpointBytes: 1, updates: 0 }, "conformance-compacted": { checkpointBytes: 1, updates: 0 } } };
  await writeFile(join(directory, "release.json"), JSON.stringify(release));
  const archive = join(root, `corpus-${version}.tar.gz`);
  await run(["tar", "--format=ustar", "-czf", archive, "-C", directory, "."], { env: { ...process.env, COPYFILE_DISABLE: "1" } });
  const bytes = await readFile(archive);
  const artifacts: Record<string, string> = {};
  for (const name of [`hitslop-tinys-${version}.tgz`, "tinys.dmg", "tinys.zip", "appcast.xml", `tinys-${version}.md`, "release-notes.md"]) artifacts[name] = "a".repeat(64);
  artifacts[`compat-corpus-${version}.tar.gz`] = sha256(bytes);
  const entry: HistoricalRelease = { release: { id, tag_name: `v${version}`, draft: false, prerelease: false, immutable: true,
    assets: [{ id, name: "release-record.json" }, { id: id + 1, name: `compat-corpus-${version}.tar.gz` }, { id: id + 2, name: `hitslop-tinys-${version}.tgz` }] },
    record: { commit: release.commit, tag: `v${version}`, macVersion: version, macBuild: "1", run: "1", attempt: "1", npmIntegrity: "sha512-" + "a".repeat(86) + "==", qualified: {},
      corpus: { file: `compat-corpus-${version}.tar.gz`, sha256: sha256(bytes) }, artifacts } };
  return { entry, bytes, directory };
}

test("published archives are verified, replayed from cache and cannot silently disappear", async () => {
  const root = await mkdtemp(join(tmpdir(), "tinys-corpora-"));
  const previous = process.env.TINYS_COMPAT_ROOT;
  try {
    const { entry, bytes } = await fixture(root);
    let downloads = 0;
    const options = { required: {}, cache: join(root, "cache"), output: join(root, "replays"), list: async () => [entry.release], download: async (id: number) => {
      if (id === 1) return Buffer.from(JSON.stringify(entry.record));
      downloads++;
      return bytes;
    } };
    const first = await resolveCorpora(options);
    const replay = await releases();
    expect(replay.map(entry => entry.name)).toEqual(["1.0.0"]);
    await verifyCorpus(replay[0]!.root, replay[0]!.release);
    const saved = join(first.root, "1.0.0/documents/conformance.tiny");
    expect((await stat(saved)).mode & 0o222).toBe(0);
    if (process.getuid?.() !== 0) await expect(writeFile(saved, "accidental write")).rejects.toThrow();
    const editable = join(root, "editable.tiny");
    await copyCorpusDocument(saved, editable);
    await writeFile(editable, "edited copy");
    expect(await readFile(saved, "utf8")).toBe("minimal archive fixture");
    const manifest = join(first.root, "1.0.0/release.json");
    const modified = (await stat(manifest)).mtimeMs;
    // Existing writable caches are protected when next resolved, without recapture.
    await chmod(saved, 0o644);
    const second = await resolveCorpora(options);
    expect((await stat(saved)).mode & 0o222).toBe(0);
    expect(second.root).toBe(first.root);
    expect(await realpath(join(options.output, "current"))).toBe(await realpath(first.root));
    expect((await stat(manifest)).mtimeMs).toBe(modified);
    expect(downloads).toBe(1);
    if (!process.env.CI) {
      const offline = await resolveCorpora({ ...options, offline: true, list: async () => { throw new Error("no network"); } });
      expect(offline.offline).toBe(true);
      expect((await verifyResolvedCorpora(offline.root, { ...options, list: async () => { throw new Error("no network"); } })).offline).toBe(true);
    }
    await expect(resolveCorpora({ ...options, list: async () => [] })).rejects.toThrow("history missing or changed");
    const cached = join(root, "cache/assets", entry.record.corpus.sha256);
    await writeFile(cached, "tampered archive");
    await expect(resolveCorpora(options)).rejects.toThrow("Corrupt cached release asset");
    await rm(cached);
    await expect(resolveCorpora({ ...options, download: async id => id === 1 ? Buffer.from(JSON.stringify(entry.record)) : Buffer.from("wrong bytes") })).rejects.toThrow("hash mismatch");
  } finally {
    if (previous === undefined) delete process.env.TINYS_COMPAT_ROOT; else process.env.TINYS_COMPAT_ROOT = previous;
    await rm(root, { recursive: true, force: true });
  }
});

test("drafts do not break PR replay; published history must be immutable and transitively complete", async () => {
  const root = await mkdtemp(join(tmpdir(), "tinys-history-"));
  try {
    const { entry } = await fixture(root);
    expect(await publishedHistory({ required: {}, list: async () => [{ ...entry.release, draft: true }], download: async () => { throw new Error("draft downloaded"); } })).toEqual([]);
    expect(() => verifyHistory([{ ...entry, release: { ...entry.release, immutable: false } }])).toThrow("immutable");
    expect(() => verifyHistory([{ ...entry, record: { ...entry.record, qualified: { "0.9.0": "b".repeat(64) } } }])).toThrow("history missing or changed");
    expect(() => verifyHistory([{ ...entry, release: { ...entry.release, assets: [] } }])).toThrow("asset");
  } finally { await rm(root, { recursive: true, force: true }); }
});

test("a new product lineage can bootstrap without published history", async () => {
  expect(await publishedHistory({ required: {}, list: async () => [] })).toEqual([]);
});

test("a required first release cannot disappear on a cold machine", async () => {
  const root = await mkdtemp(join(tmpdir(), "tinys-required-history-"));
  try {
    const { entry } = await fixture(root);
    const options = { required: { "1.0.0": entry.record.corpus.sha256 },
      download: async () => Buffer.from(JSON.stringify(entry.record)) };
    expect(await publishedHistory({ required: {}, list: async () => [] })).toEqual([]);
    expect(await publishedHistory({ ...options, list: async () => [entry.release] })).toHaveLength(1);
    await expect(publishedHistory({ ...options, list: async () => [] })).rejects.toThrow("history missing or changed");
    await expect(publishedHistory({ ...options, list: async () => [{ ...entry.release, draft: true }] })).rejects.toThrow("history missing or changed");
    await expect(publishedHistory({ ...options, required: { "1.0.0": "b".repeat(64) }, list: async () => [entry.release] })).rejects.toThrow("history missing or changed");
  } finally { await rm(root, { recursive: true, force: true }); }
});

test.each(["identity", "partial root"])("replay %s includes the candidate and authentic complete history", async (scenario) => {
  const root = await mkdtemp(join(tmpdir(), "tinys-replay-identity-"));
  const previous = process.env.TINYS_COMPAT_ROOT;
  try {
    const old = await fixture(root);
    const candidate = await fixture(root, "2.0.0", 4);
    const options = { required: {}, cache: join(root, "cache"), output: join(root, "replays"), list: async () => [old.entry.release],
      download: async (id: number) => id === 1 ? Buffer.from(JSON.stringify(old.entry.record)) : old.bytes };
    const published = await resolveCorpora(options);
    const a = await resolveCorpora({ ...options, candidate: candidate.directory });
    const manifest = JSON.parse(await readFile(join(candidate.directory, "release.json"), "utf8"));
    manifest.clock++;
    await writeFile(join(candidate.directory, "release.json"), JSON.stringify(manifest));
    const b = await resolveCorpora({ ...options, candidate: candidate.directory });
    if (scenario === "identity") {
      const first = await verifyResolvedCorpora(a.root, options);
      expect(first).not.toEqual(await verifyResolvedCorpora(published.root, options));
      expect(first).not.toEqual(await verifyResolvedCorpora(b.root, options));
      return;
    }

    const partial = join(root, "partial");
    await mkdir(partial);
    await cp(join(a.root, ".inventory.json"), join(partial, ".inventory.json"));
    await symlink(await realpath(join(a.root, "2.0.0")), join(partial, "2.0.0"));
    await expect(verifyResolvedCorpora(partial, options)).rejects.toThrow("missing");

    // A self-consistent replacement manifest is still not the released evidence.
    await cp(join(published.root, "1.0.0"), join(partial, "1.0.0"), { recursive: true, dereference: true });
    const historicalManifest = join(partial, "1.0.0/release.json");
    const altered = JSON.parse(await readFile(historicalManifest, "utf8"));
    altered.clock++;
    await chmod(historicalManifest, 0o600);
    await writeFile(historicalManifest, JSON.stringify(altered));
    await expect(verifyResolvedCorpora(partial, options)).rejects.toThrow("manifest");

    // A stale receipt also cannot hide a release published since preparation.
    candidate.entry.record.qualified = { "1.0.0": old.entry.record.corpus.sha256 };
    await expect(verifyResolvedCorpora(published.root, { ...options, list: async () => [old.entry.release, candidate.entry.release],
      download: async id => Buffer.from(JSON.stringify(id === 1 ? old.entry.record : candidate.entry.record)) })).rejects.toThrow("inventory");
  } finally {
    if (previous === undefined) delete process.env.TINYS_COMPAT_ROOT; else process.env.TINYS_COMPAT_ROOT = previous;
    await rm(root, { recursive: true, force: true });
  }
});

test("candidate snapshots are isolated and cache corruption cannot replace the current corpus", async () => {
  const root = await mkdtemp(join(tmpdir(), "tinys-snapshots-"));
  const previous = process.env.TINYS_COMPAT_ROOT;
  try {
    const old = await fixture(root);
    const candidate = await fixture(root, "2.0.0", 4);
    const options = { required: {}, cache: join(root, "cache"), output: join(root, "replays"), list: async () => [old.entry.release],
      download: async (id: number) => id === 1 ? Buffer.from(JSON.stringify(old.entry.record)) : old.bytes };
    const published = await resolveCorpora(options);
    const prepared = await resolveCorpora({ ...options, candidate: candidate.directory });
    expect(prepared.root).not.toBe(published.root);
    expect(await realpath(join(options.output, "current"))).toBe(await realpath(published.root));
    expect(await readFile(join(prepared.root, "2.0.0/release.json"), "utf8")).toBe(await readFile(join(candidate.directory, "release.json"), "utf8"));
    await expect(stat(join(published.root, "2.0.0"))).rejects.toThrow();

    await writeFile(join(candidate.directory, "documents/conformance.tiny"), "incomplete capture");
    await expect(resolveCorpora({ ...options, candidate: candidate.directory })).rejects.toThrow();
    expect(await realpath(join(options.output, "current"))).toBe(await realpath(published.root));

    const damaged = join(published.root, "1.0.0/documents/conformance.tiny");
    await chmod(damaged, 0o600);
    await writeFile(damaged, "damaged cache");
    await expect(resolveCorpora(options)).rejects.toThrow("Corrupt cached corpus tree");
    expect(await realpath(join(options.output, "current"))).toBe(await realpath(published.root));
  } finally {
    if (previous === undefined) delete process.env.TINYS_COMPAT_ROOT; else process.env.TINYS_COMPAT_ROOT = previous;
    await rm(root, { recursive: true, force: true });
  }
});

test("concurrent resolution reuses snapshots and a delayed response cannot regress history", async () => {
  const root = await mkdtemp(join(tmpdir(), "tinys-concurrent-corpora-"));
  const previous = process.env.TINYS_COMPAT_ROOT;
  let releaseOld!: () => void;
  const oldResponse = new Promise<void>(resolve => { releaseOld = resolve; });
  try {
    const first = await fixture(root, "1.9.0");
    const second = await fixture(root, "1.10.0", 4);
    second.entry.record.qualified = { "1.9.0": first.entry.record.corpus.sha256 };
    const options = { required: {}, cache: join(root, "cache"), output: join(root, "replays"),
      list: async () => [first.entry.release], download: async (id: number) => {
        if (id === 1) return Buffer.from(JSON.stringify(first.entry.record));
        if (id === 4) return Buffer.from(JSON.stringify(second.entry.record));
        return id === 2 ? first.bytes : second.bytes;
      } };
    const [a, b] = await Promise.all([resolveCorpora(options), resolveCorpora(options)]);
    expect(a.root).toBe(b.root);
    const delayed = resolveCorpora({ ...options, list: async () => { await oldResponse; return [first.entry.release]; } });
    const latest = await resolveCorpora({ ...options, list: async () => [second.entry.release, first.entry.release] });
    releaseOld();
    await expect(delayed).rejects.toThrow("history missing or changed");
    expect(await realpath(join(options.output, "current"))).toBe(await realpath(latest.root));
    expect(latest.entries.map(entry => entry.record.macVersion)).toEqual(["1.9.0", "1.10.0"]);
    expect((await releases()).map(entry => entry.name)).toEqual(["1.9.0", "1.10.0"]);
  } finally {
    releaseOld();
    if (previous === undefined) delete process.env.TINYS_COMPAT_ROOT; else process.env.TINYS_COMPAT_ROOT = previous;
    await rm(root, { recursive: true, force: true });
  }
});

test("release archive extraction refuses symlinks before writing files", async () => {
  const root = await mkdtemp(join(tmpdir(), "tinys-archive-"));
  try {
    await mkdir(join(root, "source"));
    await symlink("../../outside", join(root, "source/link"));
    await run(["tar", "--format=ustar", "-czf", join(root, "unsafe.tar.gz"), "-C", join(root, "source"), "."], { env: { ...process.env, COPYFILE_DISABLE: "1" } });
    const bytes = await readFile(join(root, "unsafe.tar.gz"));
    expect(() => archiveFiles(bytes)).toThrow("links or special entries");
  } finally { await rm(root, { recursive: true, force: true }); }
});

test("historical writers use the exact retained tarball and reject changed cached bytes", async () => {
  const root = await mkdtemp(join(tmpdir(), "tinys-retained-writer-"));
  try {
    const binary = join(root, "package/engine/darwin-arm64/tiny-engine");
    await mkdir(join(root, "package/engine/darwin-arm64"), { recursive: true });
    await writeFile(binary, "#!/bin/sh\necho original-core\n");
    const tarball = join(root, "writer.tgz");
    await run(["tar", "-czf", tarball, "-C", root, "package"]);
    const writer: Release["writer"] = { version: "1.0.0", platform: "darwin-arm64", sha256: await fileDigest(binary), buildId: "original-core" };
    await verifyPackedWriter(tarball, writer);
    await expect(verifyPackedWriter(tarball, { ...writer, sha256: "0".repeat(64) })).rejects.toThrow("differs from the captured writer");
    const options = { cache: join(root, "cache"), local: join(root, "missing"), package: async () => tarball };
    const resolved = await resolveWriter(writer, options);
    expect(await fileDigest(resolved!)).toBe(writer.sha256);
    await writeFile(resolved!, "replaced writer");
    await expect(resolveWriter(writer, options)).rejects.toThrow("Corrupt cached writer");
  } finally { await rm(root, { recursive: true, force: true }); }
});

test("capture refuses an existing explicit destination before building or replacing anything", async () => {
  const root = await mkdtemp(join(tmpdir(), "tinys-capture-destination-"));
  try {
    await writeFile(join(root, "keep"), "existing output");
    const result = await exec([process.execPath, "scripts/compat/capture.ts", "2.0.0", "--writer", "unused-engine", "--out", root], { cwd: repository, timeout: 10_000, env: { ...process.env, TINYS_ENGINE: undefined } });
    expect(result.code).not.toBe(0);
    expect(result.stderr).toContain("Capture destination already exists");
    expect(result.stdout).not.toContain("Capture: scripts/build");
    expect(await readFile(join(root, "keep"), "utf8")).toBe("existing output");
  } finally { await rm(root, { recursive: true, force: true }); }
});
