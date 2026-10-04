# Single-file slops

Status: implemented 2026-10-03 on branch `single-file` (uncommitted; the bug fixes are
`db8926d9`). Phases 1–4 done. Deviations from the plan below: the engine has `--build-id`
rather than `--protocol` (it ships with the CLI it serves); the forbidden-name rule from
`package-check.ts` was dropped (a file has no top level to keep clean); `slop schema`
and a new `slop inspect` read files through the engine on any platform, and the helper's
`schema` command was removed. Found while implementing: Apple's SQLite never writes again
through a connection whose file was renamed (`SQLITE_IOERR_VNODE`); the store reconnects
once the file is back. Open: Quick Look checked at runtime in the sandbox, npm engine
platform packages. Pre-launch, so the format can change without migrations
([compatibility](../AGENTS.md#compatibility)).

## Summary

A `.slop` becomes **one SQLite file** instead of a macOS directory package. The file holds the
built app, its schema, initial values and theme, the document's Loro state, theme overrides,
attachments and artwork. It is not a folder. A document received from someone is the live
document, edited in place.

The package's four JSON files become typed columns of one immutable `app` row. Only the compiled
app keeps paths, because `slop://app/assets/...` URLs must keep working. A native Rust binary,
`slop-engine` (the roadmap's "Next" engine, started early), validates and writes the file. Bun and
Vite still compile Svelte and evaluate `schema.ts`, `initial.ts` and `theme.ts`. A spike comes
first.

Sync and collaboration don't depend on this change; see [Sync](#sync-out-of-scope-shaped-by-this).
This change makes the unit that travels well defined.

## Today

```text
Recipe.slop/                       macOS package (LSTypeIsPackage)
  manifest.json                    TypeBox-validated; carries packageFormat and runtimeABI
  state.schema.json                root object descriptor
  initial.json                     creation-only values
  assets/app.js, app.css, theme.json, fonts, images, skin
  .agents/skills/hitslop-document/SKILL.md   copy of the generic CLI skill
  QuickLook/Preview.png, Icon.png  refreshed asynchronously after close (SlopAssetRefreshQueue)
  state/document.sqlite            document(doc_id, theme) · checkpoint(schema_key, bytes) · updates
  state/attachments/<sha256>       written by Swift, with `.pending` staging
  state/writer.lock, host.lock     flock ownership, live-socket discovery
```

Code that exists because the package is a directory:

- `HitSlopCore/SlopPackage.swift` (376 lines): directory walk, top-level allowlist, forbidden
  names, symlink and alias rejection, entry and byte budgets, `.agents` and `QuickLook` folder
  rules, `validateState`.
- `packages/cli/src/package-check.ts` (86 lines): a TypeScript copy of those rules for
  `slop build`.
- `HitSlopDocument/SchemeHandler.swift`: path normalization against the file system, alias
  rejection, an allowlist (`state.schema.json`, `initial.json`, `assets/`), no-follow reads.
- `HitSlopCore/SlopAttachments.swift`: `lstat` checks, link counts, staged writes.
- `HitSlopDocument/SlopDuplicator.swift`: copies each entry, then the SQLite backup, then
  attachments one by one.
- Users unpack downloads before placing a template in `~/.hitslop/templates`.

## The file

```sql
-- PRAGMA application_id = 0x48534C50 ("HSLP", unchanged)
-- PRAGMA user_version   = 1   the storage version: these tables; migrated forward under the writer lock

-- What the author built. Written once by `pack`; identical in a template and its documents.
CREATE TABLE app(
  id             INTEGER PRIMARY KEY CHECK(id=1),
  package_format INTEGER NOT NULL,  -- which rules validate this app; fixed at build, never migrated
  runtime_abi    INTEGER NOT NULL,  -- what the app expects of ctx
  manifest       TEXT NOT NULL,     -- JSON, TypeBox-validated: slug, title, author, categories, presentation
  descriptor     TEXT NOT NULL,     -- the document schema; `slop schema` prints it
  initial        TEXT NOT NULL,     -- a new document's values
  theme          TEXT NOT NULL      -- declared colors and their defaults
);
-- The compiled app, served at slop://app/assets/<path>.
CREATE TABLE assets(path TEXT PRIMARY KEY, bytes BLOB NOT NULL);
-- Finder, catalog and Quick Look artwork. Built with the app; refreshed after a document closes.
CREATE TABLE artwork(name TEXT PRIMARY KEY CHECK(name IN ('preview','icon')), png BLOB NOT NULL);

-- The document. A template has no rows in these tables.
CREATE TABLE document(id INTEGER PRIMARY KEY CHECK(id=1), doc_id TEXT NOT NULL, theme TEXT NOT NULL DEFAULT '{}');
CREATE TABLE checkpoint(id INTEGER PRIMARY KEY CHECK(id=1), schema_key TEXT NOT NULL, bytes BLOB NOT NULL);
CREATE TABLE updates(seq INTEGER PRIMARY KEY, bytes BLOB NOT NULL);
CREATE TABLE attachments(id TEXT PRIMARY KEY, bytes BLOB NOT NULL);  -- id = sha256 of bytes
```

### Markers

The markers stay separate, because they have different lifecycles. `package_format` is an
immutable fact about how the app was built, and it chooses which rules validate it. The storage
version (`user_version`) describes tables and moves forward when the writer migrates them.
Merging them would let a storage migration change the rules an old app is judged by.
`package_format` and `runtime_abi` move out of the manifest JSON into columns, so they're
checked before the manifest is parsed. The document layout (`meta.layout` in Loro) and the
helper protocol are unchanged.

### The schema binding stays

`checkpoint.schema_key` and the `same_schema` check on load stay. Putting the descriptor in the
same file doesn't prove the saved state was written under it, and future app upgrades
(replacing `app` and `assets`) need exactly this record.

### Outside the file, never part of the document

- **Writer lock and discovery** in a registry, `<live>/<dev>-<ino>.lock` and `.json`. Rust owns
  all of it.
  - `<live>` is `~/.hitslop/live`, derived from the account's home (`getpwuid`), not `$HOME`. The
    app, the helper and the engine all use the same folder, even under `sudo`.
  - Permissions: 0700 for the folder, 0600 for its files.
  - `HITSLOP_LIVE_DIR` is honored only in test builds.
  - The lock is an flock on the registry file, never on the database. Closing any second
    descriptor on a SQLite file drops SQLite's own POSIX locks.
  - Lock files are never unlinked, as today. A stale discovery file is removed only by the
    process that holds the lock.
  - Acquiring: open the `.slop` without following links, `fstat` it for the key, take the lock,
    then re-`stat` the path to confirm it still names the same file.
- **`Recipe.slop-journal`** exists only while a save commits (DELETE journaling, as today). That's
  why we don't move to WAL, which keeps `-wal` and `-shm` beside the document while it's open.
- **The Finder custom icon:** an extended attribute on the file.

### Renames and links

Renaming or moving an open document is still the `Moved` failure (`store.rs:63–65, 466–470`).
SQLite derives the journal's name from the path, so writing after a rename is unsafe even
though the inode-keyed lock survives. Seamless moves would be separate work. A path with more
than one hard link (`st_nlink > 1`) is refused for writing, the rule attachments already
follow.

### Why columns and not files

Only the compiled app needs paths. The manifest, descriptor, initial values and theme each have
one validator and one reader, so they're fields of a record. A file table such as `sqlar` would
bring back the allowlists and path rules this change deletes.

## What goes away

- **Package files.** The four JSON files, and the `.agents/` copy of the generic skill (agents
  have the global skill plus `slop schema` and `get --snapshot`). Also the
  `references/app-guide.md` slot, which nothing writes today.
- **Directory defenses when opening a document:** the allowlist, forbidden names, folder rules,
  and the symlink, alias and entry-walk checks. That's most of `SlopPackage.swift`, all of
  `package-check.ts`, and the scheme handler's file-system checks.
  - `pack` still reads a stage folder, so containment, symlink, count and size checks live on
    in the engine, once.
  - The scheme handler still normalizes URL paths (percent-decode, reject dot segments) before
    an exact key lookup.
- **The page's fetch of `/state.schema.json`.** In native, that's the only package file the page
  fetches (`packages/shell/src/boot.ts:43–49`; manifest, initial and theme are browser-preview
  only). The host's `config` reply carries the descriptor instead, and the scheme handler serves
  `assets` only.
- **Attachment file code** (`.pending` staging, `lstat` checks). A transaction replaces it.
- **The template unpack step**, and `SlopDuplicator`'s per-entry copy, which becomes one online
  backup.

Not going away:
- `packages/cli/src/core.ts`: `slop dev` uses it through `metadata-worker.ts`;
- `checkpoint.schema_key`;
- the `packageFormat` marker.

## Pros

1. **One file goes anywhere.** Mail, Slack, Discord, Drive, AirDrop, Windows and Linux file
   systems, iOS Files and share sheets. A directory package only survives channels that know
   about Apple packages.
2. **The received file is the document.** Unlike a zip, SQLite edits in place, and each save
   writes only the changed pages. There's no import step and no "the zip in Downloads vs. the
   real copy". This is why SQLite beats a wrapped bundle (zip, EPUB, ODT), per sqlite.org's
   application-file-format essay.
3. **Pre-launch is the only cheap moment.** After launch, a directory format would have to be
   readable forever.
4. **Less code, one owner per rule.** Package rules exist once, in Rust, instead of in Swift and
   TypeScript. Opening a document has no paths to traverse. Duplicate is one backup.
5. **The CLI reads it.** `slop schema` and `slop inspect` read the immutable `app` row directly,
   with no lock, so they work on Linux through the engine. Stock tools work too:
   `sqlite3 Recipe.slop 'select descriptor from app'`.
6. **Atomic app changes later.** Additive app upgrades, Remix and "Open with another view" can
   replace `app` and `assets` in one transaction, checked against the schema binding, instead of
   swapping files non-atomically.
7. **Attachments travel with the document.** References and bytes are in one file, so Duplicate
   and Share carry everything. Cleaning up unreferenced attachments is still deferred: references
   live inside Loro values, and undo can bring them back.
8. **Code identity.** All executable content is the `app` row plus `assets`, so a hash of that
   canonical content is "this slop's code". It's the natural key for sync ("same app"), signing,
   and permissions granted to specific code (see `archive/docs/SlopCSP.md`).

## Cons and costs

1. **A rewrite at the storage boundary.**
   - Every package reader in Swift, the helper, the CLI build, about a dozen scripts, 19 Swift
     test files, and the compatibility corpus.
   - Mostly deletion, but it touches release-critical paths (open, save, close, duplicate,
     catalog).
2. **A native binary for authoring.** `slop build` needs `slop-engine`. Authoring still needs no
   Mac app (2026-10-02). npm platform packages are a release task that must land before the
   next CLI publish.
3. **Lock and discovery move out of the document.** The AGENTS.md "never unlink `writer.lock`"
   rule is rewritten for the registry, a new place that must be correct (rules above).
4. **Reads and saves share one file.** With DELETE journaling, a commit (with fullfsync) briefly
   blocks readers, so asset loads during autosave can wait. The spike measures this.
5. **Time Machine copies whole files.** An edited document is backed up whole: up to about
   180 MiB at today's limits (50 MiB app + 100 MiB attachments + 32 MiB Loro). Today only
   `document.sqlite` changes. Local snapshots are copy-on-write, but backups to the backup disk
   are not.
6. **Copying needs care.** A Finder copy during a commit, or of a crashed document whose hot
   journal is left behind, can be inconsistent. Inside a package the journal travelled with the
   database. The app's Share operation (Phase 2) is the safe path. A raw copy is consistent only
   for a closed document with no hot journal.
7. **Renaming an open document stays a failure** (`Moved`), as it is for packages today.
8. **Artwork needs the writer lease after close.** Refreshed artwork is a database write, so the
   refresh takes the lease and skips when the document is busy or has changed.
9. **Recipients need a Quick Look extension** to see previews, because a row in a database
   isn't something Finder shows. This change adds one.
10. **Less browsable.** You can't `cat` the manifest or open `assets/` in Finder; you use
    `slop inspect` or `sqlite3`. Authors keep their source project, so this mainly affects
    received slops.
11. **Opening untrusted SQLite files needs strict checks** (Phase 1). Today `is_new` checks only
    the application ID, `user_version` and the table count.
12. **iPhone (later).** Files in other Files providers need file coordination for the journal
    sidecar. The Mac app isn't sandboxed (`hitSlop.entitlements` is empty), so the Mac is
    unaffected.

## Alternatives considered

| Option | Why not |
|---|---|
| Keep the package and zip it on share | The zip is only for sending: receiving needs an import step and leaves two copies. iPhone and non-Apple channels stay second-class. Keeps all the directory code. |
| A folder holding `document.sqlite` + blobs | Still a folder, so still needs zipping to send. It loses the single-file benefit and keeps the lock files in the document. |
| Map package files into `sqlar` | Brings directory problems (allowlists, path rules) into the file. Only `assets` needs paths. |
| Inline the app with `vite-plugin-singlefile` | It makes one `index.html`, but the shell owns the page and an app is a module exporting `mount`. Our build already emits one `app.js` and one `app.css` (`vite.ts:121–134`). Inlining assets as `data:` adds about a third to their size, parses everything up front, and breaks what the CSP and the plugin can't load from `data:`: media, workers and worklets. The SQLite file is already the single file. |
| Pack with SQLite compiled to WASM | Toolchain risk on pinned wasm-bindgen and rusqlite, and a bigger WASM. It could only pack: WASM in Bun can't take the file locks closed-document commands need. |
| Pack with `bun:sqlite` | TypeScript would write the rows of a format Rust owns: a second writer. |
| Merge `packageFormat` into `user_version` | Different lifecycles (see [Markers](#markers)). |
| Turso or SQLite Sync for device sync | They replicate rows with their own conflict rules and would duplicate Loro. Turso would replace the platform SQLite; SQLite Sync is Elastic License 2.0. Sync contents, never the file. |

## Decisions (2026-10-03)

- A slop is one SQLite file. Templates are the same file with no document rows.
- Typed `app` columns plus `assets`, not mapped files.
- Rust builds the file with a native `slop-engine pack`. No WASM packing.
- The markers stay separate: `user_version` for storage, plus `app.package_format` and
  `app.runtime_abi`. An earlier merge decision was reversed after review.
- The schema binding stays.
- A Quick Look extension is in scope, so recipients see previews.
- Share is an explicit operation.
- Spike before building.

## Approach

### Phase 0: spike

**Part A: Rust and Bun only, in a throwaway worktree.** Record results in
`docs/evidence/single-file-spike-<date>.json`. Whatever passes becomes the start of Phase 1.

1. **Engine skeleton.**
   - `crates/slop-engine` (a binary using `hitslop-core` with `storage`) with
     `pack <stage> <out>`.
   - A Bun script compiles `examples/slops/quick-checklist` into a stage (`app.json` with
     evaluated JSON, `assets/`, artwork) and packs it.
   - **Pass:** it opens through the store, and two packs give the same canonical app content (a
     hash of the `app` row plus sorted assets; file bytes may differ). Record the binary size
     for macOS and Linux.
2. **Reads beside saves.** A `crates/hitslop-core/examples` binary runs save jobs at autosave
   cadence (150 ms debounce, fullfsync, a 1,000-row document) while a second long-lived
   read-only connection reads assets.
   - Record p50, p95 and max for both asset reads and save latency.
   - **Pass:** asset read p95 under 16 ms, with save latency no worse than
     `docs/evidence/edit-latency-2026-10-01.json`.
3. **Blobs at the limits.**
   - Time and peak memory to insert and read a 10 MiB attachment.
   - For a file at every limit: open time including the layout checks and `quick_check`, and
     duplicate (backup) time.
   - Decide from these numbers whether `quick_check` runs on every open or never.
4. **Failure cases.**
   - A commit that fails, and a full disk: a small `hdiutil` disk image. Ownership and edits
     must be kept, as today.
   - Destination collisions for create and duplicate.
   - Concurrent readers during backup.
5. **Lock registry.**
   - The registry flock and SQLite's locks coexist in one process.
   - A second process is refused.
   - A rename while open fails with `Moved`.
   - Hard links are refused for writing.
   - A stale discovery file is cleaned up only under the lock.
   - Reopening after a crash recovers the hot journal.
6. **Hostile files.**
   - An extra trigger, view, table or index is refused.
   - An oversized row is refused by the `length()` checks before it's loaded.
   - Wrong row shapes are refused: two `app` rows, or a template with updates.

**Part B: one thin native slice**, after Part A passes. Build a template with the engine, then
in the app: open it in WebKit, edit, attach a blob, close, duplicate, reopen. Measure window open
time against `docs/evidence/release-window-measurements-2026-09-30.json` (under 1 s at 1,000
rows). Re-run `PagePolicyProbeTests` against the SQLite-backed handler: workers, worklets and
ranged media must behave as they do from files.

### Phase 0 Part A results (2026-10-03)

Done in the throwaway worktree `../hitslop-spike-single-file` (branch `spike/single-file`,
crate `crates/slop-engine`). Evidence: `docs/evidence/single-file-spike-2026-10-03.json`.
All of Part A passes, with one real finding.

**What works:**
- **Pack is reproducible.** Quick Checklist, built by today's CLI, packs to the same canonical
  hash and the same bytes twice: 228 KB, against a 204 KB directory. The engine binary is
  3.2 MB on arm64 (2.6 MB stripped).
- **Hostile files and row shapes are refused,** as planned. SQLite's `sqlite_autoindex_*`
  indexes are expected. Sizes are checked from `length()` alone: a 26 MiB asset is refused in
  0.6 ms.
- **Checks are cheap.** Opening a 182 MiB document at every limit takes 0.63 ms of layout and
  size checks plus 42 ms for `quick_check`, so `quick_check` runs on every open.
- **Asset reads beside real saves pass:** p95 0.75 ms against a 16 ms bar. A read that meets a
  fullfsync commit waits up to about 40 ms. Appends cost about 1.8 ms more at the median while
  a reader is active.
- **Attachments of 10 MiB:** insert 114 ms (fullfsync), read 3 ms, 1 MiB ranges 1.7 ms.
- **Publishing never overwrites,** and leaves no temporary files.
- **A full disk** fails as a typed `Full`, intact.
- **The registry lock behaves:**
  - it excludes another descriptor and another process;
  - it works alongside SQLite's locks;
  - rename gives `Moved` and hard links give `Linked`;
  - the next holder removes stale discovery.
- **A writer's open recovers a hot journal.**

**What changes the design:**
1. **No read-only connection beside a writer in the app process.** With Apple's SQLite 3.51.0,
   a READ_ONLY connection in the same process as the writer intermittently fails the writer's
   locks with `SQLITE_IOERR_LOCK` and `EBADF`: 15 of 80 runs. During a backup, both sides
   failed.
   - Opening the reader READ_WRITE with `PRAGMA query_only=ON` fixed it: 0 of 80.
   - `FULL_MUTEX` didn't help: 16 of 80.
   - SQLite's own 3.53.2 build didn't have the problem: 0 of 80.
   - So `AssetReader` and any other in-process reader open READ_WRITE plus `query_only`. Other
     processes (helper, Quick Look) may stay read-only.
   - **Today's Duplicate did this:** `store::duplicate` opened its source READ_ONLY while the
     owner could save, and Snapshot reads did the same. Fixed on 2026-10-03: in-process
     readers open read-write with `query_only`, and
     `readers_beside_an_open_document_never_fail_its_saves` covers it.
2. **Duplicate and Share of an open document run on the owner's persistence queue,** with
   the owner's connection as the backup source. From another connection, a save waits for the
   whole backup: 1.7 s at the limit, against a 2 s busy timeout.
3. **A crashed document must be opened once before it's copied.** A copy without its hot
   journal passes `integrity_check` but holds the half-applied write; an attachment no longer
   matched its hash. Keep per-blob hash checks on read.
4. **Read-only opens refuse a file with a hot journal.** The catalog and Quick Look fall back
   until a writer opens it.
5. **NOFOLLOW refuses a symlink anywhere in the path.** Canonicalize the folder first, as the
   store does today.

**Not done:** a Linux engine build (no cross toolchain here). Part B is next.

### Phase 0 Part B results (2026-10-03)

`SingleFileSliceTests` runs in the spike worktree against a real `DocumentSession` in a window.
Evidence: `part_b` in `docs/evidence/single-file-spike-2026-10-03.json`.

- **Open, edit, attach, close, duplicate, reopen: passes.** Edits and an attachment save into
  the one file. After close, the folder holds only the `.slop`: no `state/`, journal or lock
  file. The duplicate and the original reopen with everything.
- **The page can't tell the difference.** The policy probe against assets served from the
  database matches the file-served handler, including 206 ranges over database bytes.
- **Open to ready at 1,000 rows:** median 230.2 ms for a single file vs 225.9 ms for a
  directory package.
- **Shortcuts taken, to undo in Phase 1:**
  - attachments use a short-lived connection, not the store's own;
  - attachment ids aren't SHA-256;
  - skins, artwork, Quick Look, the catalog and the CLI `schema` command weren't wired;
  - `HITSLOP_LIVE_DIR` is honored in every build.

### Phase 1: Rust container (`hitslop-core`, `storage` feature)

- **`store.rs` open path, in order:**
  1. connect with `NOFOLLOW`, `trusted_schema=OFF`, `SQLITE_DBCONFIG_DEFENSIVE` (through
     `set_db_config`, not a pragma), `cell_size_check=ON`, `mmap_size=0`, the busy timeout,
     DELETE journaling, `synchronous=EXTRA` and fullfsync, as today;
  2. check `application_id`;
  3. check the markers: `user_version`, then `app.package_format` and `app.runtime_abi`. A newer
     one gets `requires_update`, and nothing is written;
  4. require the exact layout. Compare `sqlite_master` (type, name, tbl_name, sql) with what
     `SCHEMA` produces in an in-memory database. This admits SQLite's automatic primary-key
     indexes (`sqlite_autoindex_*`) and nothing else;
  5. require the row shape:
     - exactly one `app` row;
     - a template has no `document`, `checkpoint`, `updates` or `attachments` rows;
     - a document has its `document` row;
     - updates without a checkpoint are refused, as today;
  6. check sizes with `length()` before loading any value, against separate budgets: the app
     (`PackageLimits`), artwork (image limits), attachments (`AttachmentLimits`), Loro
     (`StorageLimits`). `MAX_BYTES`/`SQLITE_LIMIT_LENGTH` must admit a 25 MiB asset;
  7. run the content validators (`package.rs`);
  8. only then migrate (writer) or initialize.
- **`package.rs` (new):** pure rules over `(app row, asset list)`, shared by native open and
  `pack`, and compiled into WASM for `core.ts`:
  - the manifest through the generated TypeBox validator (`validate_manifest`);
  - the descriptor and initial values (`validate`) and the theme (`theme.rs`);
  - asset path rules (no empty or dot segments, 240 characters at most) and `PackageLimits`;
  - `assets/app.js` must be UTF-8;
  - the skin path must exist in `assets`;
  - PNG header checks for the skin and artwork.
- **The publish rule** for `pack`, `create_document` and `duplicate`: write a private temporary
  file in the destination folder, validate, close and fsync it, then rename it into place.
  - Create and duplicate never overwrite (`renamex_np(RENAME_EXCL)` on macOS,
    `renameat2(RENAME_NOREPLACE)` on Linux).
  - A rebuild may replace only a stateless template.
- **API:**
  - `pack(stage, out)`. The stage walk keeps the containment, symlink, count and size checks;
  - `open_package(path)`, which validates and returns the manifest, markers, descriptor, theme
    defaults and skin bytes;
  - `create_document(template, dest)`: a backup to the temporary file, then the `document` row
    with a fresh `doc_id`, then publish. Initial values still seed on first open;
  - `duplicate` (`store.rs:750`), which now copies everything;
  - `share_copy(dest)`, the owner-side half of the Share operation: an online backup of the
    flushed document to a temporary file, then publish. It and Duplicate of an open document run
    on the persistence queue with the owner's connection as the source, so saves queue behind
    them;
  - `AssetReader`, a long-lived connection opened READ_WRITE with `PRAGMA query_only=ON` (never
    READ_ONLY in the app process; see Part A results) that returns the content type, total
    length and bytes for a byte range, using SQLite incremental blob reads (`sqlite3_blob_read`),
    never mmap (`immutable=1` for bundled masters). WebKit's media loader needs byte ranges:
    packaged `<audio>` fails without them (`docs/evidence/policy-probe-2026-10-03.json`);
  - attachment `put/get/list` on the store's connection, on the persistence queue. Keep "blob
    committed before its reference is saved", the SHA-256 check on read, and
    `AttachmentLimits`;
  - artwork `get` and `set_if_current(doc_id, saved_version)`;
  - `inspect`.
- **`registry.rs` (new):** the lock, discovery publish and withdraw, and finding the live owner,
  with the rules in [Outside the file](#outside-the-file-never-part-of-the-document).
  `WriterLock::acquire` (`store.rs:104`) moves here. The `moved` check uses the file's inode.
- **Content types:** a Rust-owned extension table, adding `wasm`, `mjs`, `wav`, `ogg`, `webm`
  and `otf`. Unknown extensions stay `application/octet-stream` and nothing new is refused.
  The Swift table in `SchemeHandler.swift` goes.
- **Tests** (`crates/hitslop-core/tests/store.rs`, plus a registry test):
  - pack → create → open → save → reopen;
  - duplicate and share-copy with attachments;
  - hostile layouts and row shapes refused;
  - limits;
  - publish never overwrites;
  - the registry cases from Phase 0;
  - no journal left after close;
  - templates have no state rows.

### Phase 2: FFI and the Swift host

- **FFI:** export the Phase 1 API (`crates/hitslop-core-ffi/src/lib.rs`). Asset, attachment and
  artwork bytes cross to Swift; Loro bytes still don't.
- **`SlopPackage.swift`:** a thin wrapper over `open_package`. Swift still decodes the skin
  image for the window.
- **`SchemeHandler.swift`:** normalizes the URL path, then serves `assets/…` from the session's
  `AssetReader` with its content type, plus the bundled shell. It answers `Range` with 206
  (that fix may land first, on today's files; see `archive/docs/SlopCSP.md`). The policy string is
  unchanged.
- **Page config:** add `descriptor` to the `config` reply (`packages/schema/src/page.ts:73` and
  the Swift handler). The native path in `boot.ts` stops fetching `/state.schema.json`. Shell
  and host ship in one bundle, so this lands in one change; the browser preview is unchanged.
- **Attachments:** rewrite `SlopAttachments.swift` and `HitSlopNativeCLI/Attachments.swift` over
  the store, through `DocumentOwner`'s persistence queue.
- **Discovery:** call the registry FFI from `DocumentSession.swift:658–668`,
  `DocumentOwner.swift:88` and `DocumentCommand.swift:132`. Remove `SlopPackage.discoveryURL`.
- **Create and duplicate:** `SlopDuplicator` calls `create_document` or `duplicate` (callers:
  `CatalogServices.swift:87`, `NativeCLI.swift:185`, `SlopDocumentActions.swift:79`).
- **Share a copy:** a window command that runs the page barrier (unsent text, queued writes,
  attachment imports), flushes the owner, calls `share_copy` to a temporary location, and hands
  the file to the share sheet. It fails visibly if the save fails.
- **Artwork:** `SlopAssetRefreshQueue` still renders from saved state after close. Then it
  takes the writer lease and calls `set_if_current`, skipping when the document is busy or has
  changed. A failure keeps the previous artwork and is never a save failure. `setIcon` still
  sets the Finder icon on the file for owned documents.
- **Quick Look extension (new target in `apps/apple/project.yml`):** a `QLThumbnailProvider`
  plus a preview provider. They read the `artwork` rows with a read-only open through the Rust
  core, so nothing else opens the database. A hot journal, a busy file or a validation failure
  falls back to the generic icon. Received files show their preview without the custom-icon
  attribute.
- **Catalog:** `CatalogScanner` and `SlopPackageSummary` read through `open_package` and
  `inspect`.
- **`Info.plist`:** drop `LSTypeIsPackage`, and conform to `public.data` and `public.content`.
- **Tests:**
  - add a fixture helper that packs test apps through the FFI, and move the 19 test files that
    build directories onto it;
  - delete the tests whose code moved to Rust;
  - add tests for Share (including a crash during the copy), artwork skipping when busy or
    stale, and Quick Look on a file with no icon attribute.

### Phase 3: engine, CLI, scripts

- **`crates/slop-engine` commands:** `pack`, `inspect` and `schema`, plus `--protocol`
  negotiation like the helper's. It links bundled SQLite only in its own release builds, behind
  a feature, so workspace tests and the XCFramework keep the platform library.
- **Finding the engine:** `HITSLOP_ENGINE`, then the app's copy in
  `hitSlop.app/Contents/Helpers/` (preferred on a Mac, so it never writes a format the installed
  app can't read), then the workspace's `target/release`. Resolution sits next to `findNative()`
  in `packages/cli/src/native.ts`.
- **npm distribution** (`@hitslop/engine-{darwin-arm64,darwin-x64,linux-x64,linux-arm64}` as
  optional dependencies of `@hitslop/cli`, built in CI) is a separate release task, required
  before the next CLI publish.
- **`packages/cli/src/build.ts`:**
  - `core.ts` keeps validating early, before Vite runs;
  - Vite compiles into a stage: `app.json` with evaluated JSON (manifest, `package_format`,
    runtime ABI, descriptor, initial values, theme), `assets/`, and optional artwork;
  - `slop-engine pack` validates again natively and writes `<slug>.slop`;
  - delete `package-check.ts`.
- **`slop dev`:** unchanged. The metadata worker still validates through `core.ts`, and the
  browser preview serves its JSON from source. Add checks that HMR and invalid metadata still
  behave.
- **CLI commands:** `slop schema` and `slop inspect` go through the engine, so they work on
  Linux. Document edits stay on the Swift helper; moving them is what remains of roadmap "Next".
- **Scripts:** adapt the folder walkers: `build-templates`, `embed-templates`, `templates`,
  `template-cache`, `native-fixtures`, `presentation-fixtures`, `packed-test`,
  `release-artifact`, `crash-matrix`, `bench-native-owner`, `shape-lab`, `compat*`.

### Phase 4: corpus, contract, docs

- **Corpus:** `bun run compat:capture dev`. `dev` isn't frozen, so replacing it is allowed.
  Entries become files.
- **Constants:**
  - `PackageFormat` stays in `packages/schema/src/constants.ts`;
  - `packageFormat` and `runtimeABI` leave the manifest schema (`manifest.ts:87`) because pack
    stamps them as `app` columns;
  - then `bun run schema:generate`.
- **`AGENTS.md`:** the `writer.lock` rule becomes the registry lock. Update the Compatibility
  marker wording: `packageFormat` is now an `app` column.
- **`docs/engineering-contract.md`:** marker table rows 17–19, package contents, lock,
  attachments.
- **`docs/roadmap.md` "Next":** the engine exists; moving document commands to it is what
  remains.
- **Rewrite the layout sections in place:**
  - `docs/reference/runtime.md`, `docs/architecture.md`, `crates/README.md`,
    `docs/guides/cli.md`;
  - the landing site's `concepts/how-slop-files-work.mdx`, `guides/icons-and-exports.mdx` and
    `guides/styling.mdx`;
  - the skills: `hitslop-document`, `hitslop-authoring` (+ `references/storage-and-packages.md`)
    and `hitslop-design/references/presentation-and-export.md`.
- **`docs/ideas.md`:**
  - Share is a built-in operation.
  - Sync: one path for devices and people; Turso and SQLite Sync rejected; theme overrides need
    merge semantics.
  - Agent notes would become an `app` column.

## Powerful slops and CSP

Moved to `archive/docs/SlopCSP.md`. The container doesn't change the policy. The only CSP-adjacent
part of this change is the Rust-owned content-type table (Phase 1).

## Sync (out of scope; shaped by this)

Your Mac and iPhone are just collaborating with yourself, so one path serves both your devices
and other people. Each device keeps its own file as a replica.

Replicas exchange Loro updates computed from version vectors (`export(updates(vv))`), as the
Durable Objects sketch in `docs/ideas.md` describes. The local `updates` table is compacted and
trimmed, so it isn't an outbox. Missing attachments travel by hash, and the app by the hash of
its canonical content.

Blockers that don't depend on the container:
- **Trimmed history.** Closing a document over 4 MiB trims its history, so offline replicas need
  a retention policy.
- **Undo.** Raw imports clear undo.
- **Theme overrides** sit outside Loro and need merge semantics, or a per-device rule.
- **Attachments arrive separately.** Track "edits synced" apart from "file complete".

## Open questions

- **Lock files.** Do registry lock files ever need pruning? Pruning is safe only with the
  same-file check after locking. Not needed for launch.
- **Time Machine.** Is it acceptable to copy whole files at the current attachment limit, or
  should the limits come down for launch?
- **Engine targets.** Windows isn't planned. Confirm the target list for the npm packages.

## Verification

- **Phase 0:** pass or fail against the bars above, recorded in `docs/evidence/`.
- **Each phase:**
  - `cargo test --locked --workspace`;
  - `bun run check && bun run test`;
  - `bun run build && bun run swift:test && bun run test:native`.
- **At the end:**
  - `bun run release:check`, `bun run hygiene` and `bun run landing:check`;
  - `bun run compat:capture dev`, then a replay.
- **By hand:**
  - create a document from a template; edit, close and reopen it; after close, `ls` next to it
    shows one file;
  - Duplicate keeps attachments;
  - Share a copy while editing, and the copy opens elsewhere;
  - `slop schema`, `get`, `apply` and `inspect` work on a closed document and a live one, and a
    second writer is refused;
  - renaming an open document gives `Moved`;
  - send a `.slop` by Mail or Slack, open it from Downloads, and Quick Look shows its preview;
  - `sqlite3 Recipe.slop 'select descriptor from app'` prints the schema.

## Review notes (2026-10-03)

Two reviews of the first draft. Taken from the first:
- Share is an operation;
- renames stay `Moved`, plus the registry rules;
- automatic indexes and row shapes in validation, and the validation order;
- keep `core.ts` for `slop dev`;
- one publish rule;
- artwork stays asynchronous behind the lease;
- the markers stay separate, and the schema binding stays;
- a native slice and the failure cases in the spike;
- corrections about attachment cleanup and the `updates` table;
- CSP split out.

Taken from the second: npm distribution as a separate release task, and the whole registry in
Rust. Rejected from the second, so it isn't proposed again:

- `PRAGMA defensive`: it's a `sqlite3_db_config` option, not a pragma.
- `synchronous = FULL`: today's EXTRA with fullfsync stays.
- A stage containing `schema.ts`, `initial.ts` and `theme.ts`: the engine can't evaluate
  TypeScript. The stage holds evaluated JSON.
- Deleting `core.ts`: it breaks `slop dev`.
- Copying Finder icon attributes "in one transaction": extended attributes aren't part of SQLite
  transactions.
- Rejecting all indexes: primary keys create automatic ones.
- Dropping the schema binding: see above.
- "Time Machine creates snapshot references": that's true of local snapshots, but backups to the
  backup disk copy a changed file whole.

Also rejected: `vite-plugin-singlefile` (see [Alternatives](#alternatives-considered)).
