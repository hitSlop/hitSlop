# Browser storage spike (2026-10-06)

Can a hitSlop document live in a browser as the same SQLite file, saved by the same Rust
code? Measured on the development M1 with release builds (rustc 1.96.1), against desktop
Google Chrome 154 and Playwright's WebKit 26.6 (Safari's engine, not Safari itself), each
in a persistent profile on loopback. Data: [browser-storage-2026-10-06.json](browser-storage-2026-10-06.json).

The stack: rusqlite 0.40.2 on wasm32 (its default `ffi-sqlite-wasm-rs` feature compiles
SQLite 3.53.0 through sqlite-wasm-rs 0.5.5) with sqlite-wasm-vfs's `sahpool` VFS (0.2.0
patched, then 0.3.0 as published), one pool directory and VFS per copy, in a dedicated
worker. No COOP/COEP. SQLite C needs
an LLVM clang with the WebAssembly target (Homebrew LLVM 22; Apple's clang has none).

## Result

The route holds in both engines. sqlite-wasm-vfs 0.2.0 has a defect that corrupts a
document on a mid-commit crash; the spike found and patched it, and 0.3.0, which runs on
today's rusqlite, has it fixed and passes every suite unpatched
([below](#sqlite-wasm-vfs-030)). None of the stop conditions remain. The table is the
patched 0.2.0 run.

| | Chrome | WebKit |
| --- | --- | --- |
| Every native connection setting applied (DEFENSIVE, no checkpoint on close, 32 MiB length limit, `trusted_schema`, `cell_size_check`, `mmap_size`, `journal_mode=DELETE`, `synchronous=EXTRA`, `secure_delete=FAST`, NOFOLLOW) | yes | yes |
| F1 to first state (install, import, open, checks, load) | 143 ms | 87 ms |
| Manifest validated by jsonschema in WASM | yes | yes |
| Saved state equals native `slop get` after a reopen in a new worker | yes | yes |
| Unedited import exports byte-identical; edited export opens natively | yes | yes |
| Corpus: 14 documents read, theme, export, recorded edit and reopen | 14/14 | 14/14 |
| Damaged inputs refused as native refuses them | 5/6, WAL differs | 5/6, WAL differs |
| Small checkpoint save, p50 / p95 | 2.1 / 5.0 ms | 1 / 2 ms |
| 30 MiB checkpoint save, p50 / p95 | 307 / 370 ms | 225 / 268 ms |
| A replied save survives an immediate `terminate()` (5 cases × 5 rounds) | yes | yes |
| Worker killed mid-commit, 50 kills (patched VFS) | 50/50 | 50/50 (34 rolled back) |
| Tab closed mid-commit, 10 closes (patched VFS) | 10/10 | 10/10 |
| Service Worker assets in a cross-site frame | yes | yes |

## Crash recovery: the VFS never rolled back

As published, sqlite-wasm-vfs 0.2.0 corrupts a document when its worker dies mid-commit.
WebKit failed after 1 kill, Firefox (checked before the scope narrowed) after 2 to 6, both
with a hot journal beside the database and `quick_check` reporting "invalid page number".

Cause: `sahpool`'s `xCheckReservedLock` always reports a reserved lock held, so SQLite's
hot-journal check concludes another connection is writing and reads the half-written file
as it is. Given the same database and journal, native SQLite rolled back to a file passing
`quick_check` and `integrity_check`. sqlite.org's JavaScript `opfs-sahpool` reports real
lock state (forum post b2fbb61642), and sqlite-wasm-vfs 0.3.0 tracks lock levels.

With one line changed (report no reserved lock; one connection owns each pool), WebKit
passed 50 of 50 kills twice (43 and 34 rolled back through the journal) and every tab
close recovered. Chrome's `terminate()` lets the running synchronous WASM call finish,
so worker kills there never land mid-commit; closing the tab does, and all 10 recovered.

A dead worker's Web Lock can be granted before its sync access handles close (WebKit,
twice in 100 kills; Firefox too): the next install fails with `NoModificationAllowedError`
or `InvalidStateError`, and succeeds when retried in a fresh worker after 50 ms. A retry in
the same worker could collide with handles its failed install opened.

## sqlite-wasm-vfs 0.3.0

0.3.0 lists sqlite-wasm-rs 0.6 only for its own tests, so it was built on rusqlite 0.40.2
and sqlite-wasm-rs 0.5.5 as published, with wasm-bindgen 0.2.129 (it needs 0.2.128 or
later) and an OS callback type of a few lines: no sleep, `crypto.getRandomValues`,
`Date.now`, as sqlite-wasm-rs 0.5.5 provides them without atomics. It linked with no
unresolved imports. The same suites, unpatched, in Chrome and WebKit:

| | Chrome | WebKit |
| --- | --- | --- |
| Storage, corpus 14/14, refusals (WAL still refused), exports byte-identical | pass | pass |
| Worker killed mid-commit, 50 kills | 50 committed | 29 rolled back, 21 committed |
| Tab closed mid-commit, 10 closes | 10/10 (8 hot journals) | 10/10 (10 hot journals) |
| Durability, tabs, Service Worker frame, F3 | pass | pass |
| Small / 30 MiB checkpoint save, p95 | 6.2 / 410 ms | 2 / 273 ms |

Its API changes what the host does:

- Import is chunked (`begin_import_unchecked`, `write`, `finish`). Importing F3 as it
  downloaded peaked at 5.8 MiB of WASM memory in Chrome, against 177 MiB whole. WebKit's
  fetch stream delivered the file in one piece, so it peaked at 177 MiB there; reading
  the input in slices bounds it (not run).
- Export through the VFS refuses an open file and reads it whole; the `sqlite_dbpage`
  stream into an OPFS file keeps working with the copy open.
- Files are listed and removed through `VfsFilesManager`.

The patched 0.2.0 build, rebuilt with wasm-bindgen 0.2.129, still passes the storage suite
in both engines (WebKit: 10 of 10 kills rolled back).

## Import, export and download

At F3 (183,664,640 bytes: 48 MiB of random assets, 100 MiB of attachments, ~28 MiB of
state), both engines import in 206 ms, reach first state in 362–368 ms, quick-check in
83–85 ms and export byte-identical both ways.

| WASM memory | MiB |
| --- | ---: |
| An empty worker | 1.8 |
| The importing worker after `import_db_unchecked` | 177 |
| The owner after opening and loading | 213 |
| The owner after a streamed (`sqlite_dbpage`) export | 213 |
| The owner after `export_db` | 388 |

WASM memory never shrinks, so import runs in its own short-lived worker and export streams.

Handing the export to a download: Chrome reads back an in-memory Blob of the chunks, but
WebKit refuses one ("Load failed"), whether of 1 MiB chunks or 16 MiB parts. Streaming the
pages through a sync access handle into a host-origin OPFS file, then reading it as a
`File`, works in both: 468 ms (Chrome) and 699 ms (WebKit) for F3, with no memory growth.

## Where a large save's time goes

A 30 MiB checkpoint save, p50 of 8, in ms: Loro's snapshot export, the statements, and
the commit (syncs and the journal's deletion). WAL in the browser needs exclusive locking,
since `sahpool` has no shared memory; it opens and saves that way.

| Mode | Chrome export / write / commit / total | WebKit export / write / commit / total |
| --- | --- | --- |
| `DELETE`, `synchronous=EXTRA` (as shipped) | 15 / 34 / 275 / 328 | 15 / 19 / 200 / 233 |
| `DELETE`, `synchronous=NORMAL` | 15 / 32 / 241 / 288 | 15 / 21 / 182 / 215 |
| `WAL`, exclusive locking, `synchronous=FULL` | 16 / 53 / 280 / 341 | 16 / 30 / 151 / 199 |

The time is writing 30 MiB through OPFS, not the journal design: WAL saves nothing in
Chrome and 15% in WebKit, and relaxing syncs about 12%.

## Tabs and origins

A second owner's `ifAvailable` lock request is refused; when the holder dies it is granted
(within 1 ms of a worker's termination, 230–450 ms of a tab's close). Installing a held
pool without the lock fails in both engines.

The host on `127.0.0.1` and the copy frame on `localhost` are two secure origins. In both
engines the frame's Service Worker registers (partitioned under the host's site), needs
`clients.claim()` to control the frame's first load, and controls reloads at load. Through
it the app module, a relative module, Brotli CSS decoded in Rust, a stylesheet, a font, an
image, a byte range (206) and a seekable WebM all load from the owner worker under the
generated CSP, which refuses inline, `blob:` and `eval` scripts. The frame sees its own
empty OPFS, cannot reach `parent.document`, and cannot fetch from or start a worker on the
host origin. The YouTube relay was not run.

## Size

Each binary after wasm-bindgen; bytes raw / gzip / Brotli.

| Binary | Release | Size profile + `wasm-opt -Oz` |
| --- | --- | --- |
| (a) today's `hitslop-core-wasm` | 4,244,902 / 1,382,359 / 938,911 | 1,851,069 / 758,806 / 588,419 |
| (b) (a)'s core plus SQLite and `sahpool` | 5,736,248 / 2,035,852 / 1,391,476 | 2,602,941 / 1,135,081 / 896,576 |
| (c) (b) plus jsonschema, brotli and sha2 | 7,556,543 / 2,659,205 / 1,789,928 | 3,721,179 / 1,571,138 / 1,190,284 |
| (d) the real core with storage, owner and page protocol linked | 8,016,700 / 2,784,394 / 1,868,783 | 3,855,547 / 1,629,498 / 1,235,571 |

The size profile is `opt-level = "z"`, fat LTO and one codegen unit. (b) and (c) are the
spike's binding, which reaches less of the core than (a); (d) measures what Phase 1 ships.

## Core port probe

With storage enabled for wasm32, only four files fail to compile, all native by design:
the registry (writer lock and discovery), the socket, `file/copy.rs`'s renames and
`file/artwork.rs`'s oxipng. With those stubbed, `store`, `owner`, `command`, `envelope`,
the file checks and the manifest validator compile. What remains fails only at run time:
the owner's threads, channels and `Instant`, and `Store::open`'s path. `command::page`
uses none of them, so the page protocol carries over once the owner has a browser driver.

## Native findings

- A WAL-mode file is accepted natively: `inspect`, `get` and `apply` open it, and the
  writer rewrites it to rollback mode, leaving a `-shm` file. `file::connect`'s comment
  says such a file is refused. The browser cannot open one (`sahpool` has no shared
  memory) and refuses it as a storage failure.
- `slop inspect` on a WAL-mode file leaves `-wal` and `-shm` files beside it.
- A file 1,000 bytes short of its last page opens and edits, natively and in the browser.

## Libraries considered

The spike kept rusqlite on sqlite-wasm-rs with the `sahpool` VFS: the same rusqlite API on
both targets keeps one implementation of the file and store code, and it is real SQLite
C, so the format is identical byte for byte. Rejected before the spike, from their
sources and documentation (checked 2026-10-05):

- **Turso** (Rust rewrite, 0.8.x; browser package `@tursodatabase/database-wasm`).
  - Its COMPAT.md says it supports WAL only and plans no rollback journal. Our files are
    `journal_mode=DELETE`.
  - It has no backup API, `db_config`, `sqlite3_limit`, blob I/O or serialize.
  - Writing `auto_vacuum` is experimental, `trusted_schema` and `cell_size_check` are
    unsupported, and invalid UTF-8 is rewritten to U+FFFD.
  - Its browser build targets wasm32-wasip1-threads and needs COOP/COEP.
  - MVCC (`BEGIN CONCURRENT`) solves concurrent writers. hitSlop has one writer per
    document by contract, and Loro merges concurrent edits inside that owner.
- **`sqlite-wasm`** (crates.io, 0.1.x). It wraps the sqlite.org JS build with the `opfs`
  VFS, which needs COOP/COEP and `SharedArrayBuffer`. Its message-based exec/query API
  has no backup, blob, limits or `db_config`, so storage would be written twice.
- **rsqlite-wasm** (pure Rust, 0.1.4; reviewed in `_docs/rsqlite-wasm-main`).
  - No auto-vacuum: `PRAGMA auto_vacuum` returns a hard-coded 0, there is no
    pointer-map support, and `Pager::allocate_page` appends pages linearly. Its writes
    would leave our `auto_vacuum=FULL` files corrupt for real SQLite.
  - `integrity_check` and `quick_check` always return `ok`.
  - It has no backup, `trusted_schema`, `cell_size_check`, `query_only`, limits or
    `db_config`.
  - It has its own API, and it shards a database across `name.db.000…` files.
- **sqlite.org's `@sqlite.org/sqlite-wasm` and wa-sqlite from JavaScript.** Storage
  policy would move out of Rust.
- **diesel, sqlx.** Diesel reaches WASM through sqlite-wasm-rs anyway; sqlx has no WASM
  target.

## Not covered

Safari itself, mobile browsers, private windows, quota exhaustion, `persist()` grants,
append saves (the probe saved checkpoints only), sliced import in WebKit, and the YouTube
relay. Firefox was dropped from scope after the crash investigation.
