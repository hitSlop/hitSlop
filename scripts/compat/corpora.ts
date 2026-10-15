import { Database } from "bun:sqlite";
import { strict as assert } from "node:assert";
import { cp, mkdir, mkdtemp, readFile, readdir, rename, rm, writeFile, appendFile, realpath, symlink, lstat, chmod } from "node:fs/promises";
import { homedir } from "node:os";
import { join, resolve } from "node:path";
import { repository, sha256, fileDigest } from "../lib/artifacts";
import { archiveFiles, extractArchive } from "../lib/release-archive";
import { assetBytes, releaseInventory, requiredAsset, type PublishedRelease } from "../lib/release-assets";
import { validateRecord, type ReleaseRecord } from "../release/record";
import { verifyCorpus } from "./integrity";
import { atomicJson } from "../lib/verification";
import { corpusRoot, readJSON, releases, type Release } from "./corpus";
import requiredHistory from "./required-history.json";

export type HistoricalRelease = { release: PublishedRelease; record: ReleaseRecord };
export type CorpusInventory = Record<string, string>;
export const corpusCache = () => resolve(process.env.HITSLOP_CORPUS_CACHE ?? join(homedir(), ".cache/hitslop/compat-corpora"));

/** Remembered inventory, the checked-in launch baseline and each release's closure
 * detect disappearing history. Never recapture evidence. */
export function verifyHistory(entries: HistoricalRelease[], known: CorpusInventory = {}): CorpusInventory {
  const inventory: CorpusInventory = {};
  for (const { release, record } of entries) {
    assert.ok(!release.draft && !release.prerelease && release.immutable, `${release.tag_name} must be an immutable stable release`);
    validateRecord(record, release.tag_name, record.commit);
    assert.ok(!inventory[record.macVersion], `Duplicate release: ${record.macVersion}`);
    inventory[record.macVersion] = record.corpus.sha256;
    requiredAsset(release, record.corpus.file);
    requiredAsset(release, `hitslop-${record.macVersion}.tgz`);
  }
  for (const requirements of [known, ...entries.map(entry => entry.record.qualified)])
    for (const [version, hash] of Object.entries(requirements))
      assert.equal(inventory[version], hash, `Published history missing or changed: ${version}`);
  return inventory;
}

export async function publishedHistory(options: {
  list?: () => Promise<PublishedRelease[]>; download?: (id: number) => Promise<Buffer>; required?: CorpusInventory;
} = {}): Promise<HistoricalRelease[]> {
  const all = await (options.list ?? releaseInventory)();
  const entries: HistoricalRelease[] = [];
  for (const release of all.filter(entry => !entry.draft && !entry.prerelease)) {
    const bytes = await (options.download ?? assetBytes)(requiredAsset(release, "release-record.json").id);
    entries.push({ release, record: JSON.parse(bytes.toString()) });
  }
  verifyHistory(entries, options.required ?? requiredHistory);
  return entries;
}

export async function cachedAsset(entry: HistoricalRelease, name: string, cache = corpusCache(), download = assetBytes): Promise<string> {
  const hash = entry.record.artifacts[name];
  assert.match(hash ?? "", /^[a-f0-9]{64}$/, `Unrecorded release asset: ${name}`);
  const file = join(cache, "assets", hash!);
  const existing = await readFile(file).catch(error => { if (error.code !== "ENOENT") throw error; return undefined; });
  if (existing) {
    assert.equal(sha256(existing), hash, `Corrupt cached release asset: ${file}`);
    return file;
  }
  if (process.env.HITSLOP_COMPAT_OFFLINE === "1") throw new Error(`Offline asset not cached: ${name}`);
  const bytes = await download(requiredAsset(entry.release, name).id);
  assert.equal(sha256(bytes), hash, `Release asset hash mismatch: ${name}`);
  await mkdir(join(cache, "assets"), { recursive: true });
  const stage = await mkdtemp(join(cache, "assets", ".download-"));
  try {
    await writeFile(join(stage, "asset"), bytes);
    await rename(join(stage, "asset"), file);
  } finally { await rm(stage, { recursive: true, force: true }); }
  return file;
}

/** Serialize cache writes and snapshot activation across checkouts. SQLite releases
 * this bookkeeping lock on process death; no stale lock-file deletion is needed. */
async function resolutionLease(cache: string) {
  await mkdir(cache, { recursive: true });
  const db = new Database(join(cache, "resolution.sqlite"));
  db.exec("PRAGMA busy_timeout=0");
  const deadline = Date.now() + 120_000;
  while (true) {
    try { db.exec("BEGIN EXCLUSIVE"); break; }
    catch (error) {
      if (!(error instanceof Error) || !/locked|busy/i.test(error.message) || Date.now() > deadline) {
        db.close();
        throw new Error("Cannot prepare compatibility history while another resolver holds the cache lock", { cause: error });
      }
      await Bun.sleep(50);
    }
  }
  return () => { db.exec("ROLLBACK"); db.close(); };
}

type Receipt = { entries: HistoricalRelease[]; inventory: CorpusInventory; offline: boolean };

/** A receipt is an inventory claim, not evidence that the selected files cover it.
 * Bind historical manifests to retained archives and hash every actual replay entry. */
export async function verifyResolvedCorpora(root: string, options: {
  cache?: string; dev?: string; required?: CorpusInventory; history?: HistoricalRelease[];
  list?: () => Promise<PublishedRelease[]>; download?: (id: number) => Promise<Buffer>;
} = {}) {
  const receipt = await readJSON<Receipt>(join(root, ".inventory.json"));
  assert.ok(receipt, "Compatibility root has no verified inventory; resolve corpora first");
  const offline = receipt.offline || process.env.HITSLOP_COMPAT_OFFLINE === "1";
  if (offline && (process.env.CI || process.env.HITSLOP_RELEASE_TAG)) throw new Error("CI and release checks require fresh published history");
  const history = options.history ?? (offline ? receipt.entries : await publishedHistory(options));
  const inventory = verifyHistory(history, options.required ?? requiredHistory);
  const cache = options.cache ?? corpusCache();
  const known = await readJSON<Receipt>(join(cache, "history.json"));
  verifyHistory(history, known?.inventory);
  assert.deepEqual(receipt.inventory, inventory, "Compatibility inventory changed; resolve corpora again");
  assert.deepEqual(verifyHistory(receipt.entries), inventory, "Compatibility receipt inventory mismatch");
  const entries = await releases(root);
  assert.ok(entries.length, "No compatibility corpora to replay");
  assert.deepEqual((await readdir(root)).sort(), [".inventory.json", ...entries.map(entry => entry.name)].sort(), "Unexpected compatibility root entries");
  for (const version of Object.keys(inventory))
    assert.ok(entries.some(entry => entry.name === version), `Published corpus missing from replay: ${version}`);
  const manifests: Record<string, string> = {};
  let candidates = 0;
  for (const entry of entries) {
    const manifest = await readFile(join(entry.root, "release.json"));
    assert.equal(entry.release.release, entry.name, "Corpus entry name differs from its manifest");
    const published = history.find(item => item.record.macVersion === entry.name);
    if (published) {
      const archive = await cachedAsset(published, published.record.corpus.file, cache,
        offline ? async () => { throw new Error("Offline asset not cached"); } : options.download);
      const original = archiveFiles(await readFile(archive)).get("release.json");
      assert.ok(original, "Published archive has no corpus manifest");
      assert.equal(sha256(manifest), sha256(original), `Changed published corpus manifest: ${entry.name}`);
      assert.ok(entry.release.frozen, "Published corpus is not frozen");
      assert.equal(entry.release.commit, published.record.commit);
    } else if (entry.name === "dev") {
      assert.ok(!history.length && !entry.release.frozen, "Dev corpus is only allowed before launch");
      assert.equal(sha256(manifest), await fileDigest(join(options.dev ?? join(repository, "tests/compat/dev"), "release.json")), "Changed prelaunch corpus manifest");
    } else {
      assert.ok(entry.release.frozen && /^\d+\.\d+\.\d+$/.test(entry.name) && ++candidates === 1, "Unexpected unpublished corpus entry");
    }
    await verifyCorpus(entry.root, entry.release);
    manifests[entry.name] = sha256(manifest);
  }
  return { inventory, offline, manifests };
}

/** Reuse extracted bytes only after checking the manifest against the original archive
 * and every file against that manifest. Cache damage fails rather than becoming evidence. */
async function preparedTree(cache: string, key: string, manifest: Buffer, materialize: (stage: string) => Promise<void>) {
  const trees = join(cache, "trees");
  await mkdir(trees, { recursive: true });
  const root = join(trees, key);
  const release: Release = JSON.parse(manifest.toString());
  const verify = async (directory: string) => {
    assert.equal(await fileDigest(join(directory, "release.json")), sha256(manifest), `Corrupt cached corpus manifest: ${directory}`);
    await verifyCorpus(directory, release);
    // Protect shared evidence from accidental writes, including older writable caches.
    // Replay edits only disposable copies, whose permissions it resets explicitly.
    for (const path of ["release.json", ...Object.keys(release.files)]) await chmod(join(directory, path), 0o444);
  };
  const present = await lstat(root).catch(error => { if (error.code !== "ENOENT") throw error; return undefined; });
  if (present) {
    assert.ok(present.isDirectory() && !present.isSymbolicLink(), `Invalid corpus cache directory: ${root}`);
    try { await verify(root); }
    catch (error) { throw new Error(`Corrupt cached corpus tree ${root}; remove this cache tree and run bun run compat:resolve again`, { cause: error }); }
  } else {
    const stage = await mkdtemp(join(trees, ".extract-"));
    try {
      await materialize(stage);
      await verify(stage);
      await rename(stage, root);
    } finally { await rm(stage, { recursive: true, force: true }); }
  }
  return { root, release };
}

/** Snapshot paths never change. Only the convenience pointer for standalone tests moves. */
async function preparedSnapshot(output: string, roots: Record<string, string>, receipt: Receipt) {
  const links = Object.entries(roots).sort(([a], [b]) => a.localeCompare(b, "en", { numeric: true }));
  const encoded = JSON.stringify(receipt);
  const identity = sha256(Buffer.from(JSON.stringify([links, receipt])));
  const snapshots = join(output, "snapshots");
  await mkdir(snapshots, { recursive: true });
  const root = join(snapshots, identity);
  const present = await lstat(root).catch(error => { if (error.code !== "ENOENT") throw error; return undefined; });
  if (present) {
    assert.ok(present.isDirectory() && !present.isSymbolicLink(), `Invalid compatibility snapshot: ${root}`);
    assert.deepEqual((await readdir(root)).sort(), [".inventory.json", ...links.map(([name]) => name)].sort(), `Changed compatibility snapshot entries: ${root}`);
    assert.equal(await readFile(join(root, ".inventory.json"), "utf8"), encoded, `Changed compatibility snapshot: ${root}`);
    for (const [name, directory] of links) assert.equal(await realpath(join(root, name)), await realpath(directory), `Changed corpus link: ${name}`);
  } else {
    const stage = await mkdtemp(join(snapshots, ".prepare-"));
    try {
      for (const [name, directory] of links) await symlink(resolve(directory), join(stage, name), "dir");
      await writeFile(join(stage, ".inventory.json"), encoded);
      await rename(stage, root);
    } finally { await rm(stage, { recursive: true, force: true }); }
  }
  return root;
}

export async function resolveCorpora(options: {
  cache?: string; output?: string; offline?: boolean; candidate?: string; dev?: string;
  list?: () => Promise<PublishedRelease[]>; download?: (id: number) => Promise<Buffer>; required?: CorpusInventory;
} = {}) {
  const cache = resolve(options.cache ?? corpusCache());
  const offline = options.offline ?? process.env.HITSLOP_COMPAT_OFFLINE === "1";
  if (offline && (process.env.CI || process.env.HITSLOP_RELEASE_TAG)) throw new Error("CI and release checks require fresh published history");
  // Discovery may overlap another resolver. Recheck remembered history inside the lock,
  // so a slow older response can never regress a newer activated inventory.
  const discovered = offline ? undefined : await publishedHistory(options);
  const unlock = await resolutionLease(cache);
  try {
    const prior = await readJSON<Receipt>(join(cache, "history.json"));
    const entries = [...(discovered ?? prior?.entries ?? [])].sort((a, b) => a.record.macVersion.localeCompare(b.record.macVersion, "en", { numeric: true }));
    if (offline && !prior) throw new Error("No cached compatibility history; resolve online first");
    const inventory = verifyHistory(entries, prior?.inventory);
    verifyHistory(entries, options.required ?? requiredHistory);
    if (offline) console.log("! OFFLINE compatibility replay: cached history only; does not certify a release");
    const output = resolve(options.output ?? join(repository, ".hitslop/compat"));
    const roots: Record<string, string> = {};
    for (const entry of entries) {
      const archive = await cachedAsset(entry, entry.record.corpus.file, cache, offline ? async () => { throw new Error("Offline asset not cached"); } : options.download);
      const bytes = await readFile(archive);
      const manifest = archiveFiles(bytes).get("release.json");
      assert.ok(manifest, "Published archive has no corpus manifest");
      const tree = await preparedTree(cache, entry.record.corpus.sha256, manifest, stage => extractArchive(bytes, stage));
      assert.ok(tree.release.frozen, "Published corpus is not frozen");
      assert.equal(tree.release.release, entry.record.macVersion);
      assert.equal(tree.release.commit, entry.record.commit);
      roots[entry.record.macVersion] = tree.root;
    }
    const localTree = async (directory: string) => {
      const manifest = await readFile(join(directory, "release.json"));
      await verifyCorpus(directory, JSON.parse(manifest.toString()));
      return preparedTree(cache, `local-${sha256(manifest)}`, manifest, stage => cp(directory, stage, { recursive: true }));
    };
    // Only the resolver supplies the prelaunch fixture. Raw tests require preparation.
    if (!entries.length) roots.dev = (await localTree(options.dev ?? join(repository, "tests/compat/dev"))).root;
    const receipt = { entries, inventory, offline };
    const published = await preparedSnapshot(output, roots, receipt);
    let root = published;
    const candidate = options.candidate ?? process.env.HITSLOP_COMPAT_CANDIDATE;
    if (candidate) {
      const tree = await localTree(candidate);
      assert.ok(tree.release.frozen && /^\d+\.\d+\.\d+$/.test(tree.release.release), "Invalid candidate corpus");
      assert.ok(!inventory[tree.release.release], "Candidate version is already published");
      root = await preparedSnapshot(output, { ...roots, [tree.release.release]: tree.root }, receipt);
    }
    await atomicJson(join(cache, "history.json"), receipt);
    const pointer = join(output, `.current-${crypto.randomUUID()}`);
    try {
      await symlink(published, pointer, "dir");
      await rename(pointer, join(output, "current"));
    } finally { await rm(pointer, { force: true }); }
    process.env.HITSLOP_COMPAT_ROOT = root;
    return { root, inventory, offline, entries };
  } finally { unlock(); }
}

/** Writer resolution shares the exact release inventory used by the replay. */
export async function historicalPackage(version: string): Promise<string> {
  const inventory = await readJSON<{ entries: HistoricalRelease[] }>(join(corpusRoot(), ".inventory.json"));
  const entry = inventory && inventory.entries.find(entry => entry.record.macVersion === version);
  assert.ok(entry, `Resolve published corpora before fetching writer ${version}`);
  return cachedAsset(entry, `hitslop-${version}.tgz`);
}

export async function verifyPreludes(entries: { release: Release }[]) {
  for (const { release } of entries) if (release.frozen)
    for (const [abi, hash] of Object.entries(release.producer.runner ?? {})) {
      assert.match(abi, /^[1-9]\d*$/);
      assert.equal(await fileDigest(join(repository, `crates/hitslop-runner/src/abi/${abi}.generated.js`)), hash, `${release.release}: changed ABI ${abi} command prelude`);
    }
}

if (import.meta.main) {
  const resolved = await resolveCorpora();
  console.log(`Compatibility corpus: ${Object.keys(resolved.inventory).length} published releases (${resolved.root})`);
  if (process.env.GITHUB_ENV) await appendFile(process.env.GITHUB_ENV, `HITSLOP_COMPAT_ROOT=${resolved.root}\n`);
}
