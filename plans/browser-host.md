# Browser host

Status, 2026-10-06: the [storage spike](#spike-results) passed in Chrome and WebKit; this
is the plan for its first phase, `slop open --browser`, shipped as a beta. Share links
are a [separate, deferred proposal](share-links.md). Next: [step 0](#steps). Steps 1 and
2 are enabling refactors for this work, kept narrow and tied to the first working
browser document.

## Staging

1. **Now: `slop open --browser`, a beta.** A real `.slop` opens in a desktop browser,
   saves in browser storage and downloads back as a `.slop` the Mac app opens. No
   account, no service, no share links.
2. **Later:** the same host on hitslop.com, taking a dropped `.slop` with no account,
   then [share links](share-links.md). Both reuse this host and wait for launch.

`slop dev` stays as it is: a disposable in-page WASM owner. It can't prove the browser
path. Every open requires `assets/app.js` (`file/mod.rs`), and dev never builds one,
because Vite serves the live code.

## What ships

- `slop open --browser <file.slop>` opens the document in the default browser. It works
  wherever the CLI does, Linux included, with no Mac app. `slop open --browser` alone
  opens the copies page.
- A browser copy is a clean copy of the document as it is saved now: current state, no
  history, only the attachments it references. It autosaves in this browser and
  downloads as a `.slop`. Nothing is uploaded, synced or backed up.
- Opening a file again makes a new copy; the copies page continues one. The original
  file is never changed. To bring edits back to the Mac, download the copy.
- Desktop Chrome and Safari. Other browsers come later.

## Beta scope

The experience can be rough; the files it writes cannot. A downloaded copy is a `.slop`
every later hitSlop must open, so crash recovery, the open checks and correct exports are
as strict as natively.

Out of the beta:

- a second worker for saves of near-limit documents (one worker; a 30 MiB checkpoint
  pauses edits for about 300 ms);
- the **Use here** tab handoff, and polish of the copies page;
- mobile browsers, private windows and storage persistence prompts;
- reopening copies while no host runs (caching the runtime in a host Service Worker).

## Architecture

```text
slop open --browser (Bun)        one host per user, fixed loopback port
127.0.0.1:<port> (host origin)   copies page, save status, download; holds all storage
  ├─ dedicated worker per copy   Rust/WASM: owner + Loro + store + rusqlite + SQLite
  │     └─ sahpool VFS ────────► OPFS, one pool directory per copy
  └─ iframe: <copy>.localhost:<port>   page shell + that copy's app, one origin per copy
        └─ Service Worker ◄── MessagePort ──► the copy's worker: assets, byte ranges
```

### The host process

- **One reusable host.** The first `slop open --browser` starts the host in the
  foreground, as `slop dev` runs, and writes `~/.hitslop/browser-host.json`: port, pid,
  protocol and a random token. The port is fixed, because OPFS belongs to an origin and
  the port is part of it; another port would strand every copy.
- **Later invocations** read that file, check `/__host/health` with the token, and hand
  over their snapshot through an authenticated control request, then open the browser.
  They tell apart three cases and say which: our compatible host is running (hand over),
  an older hitSlop host is running (stop it first), another process owns the port.
- **Import sessions.** The host serves each snapshot under a bounded session: an
  unguessable ID that allows ranged reads and retries, and expires when the import
  finishes or after ten minutes. The snapshot file is deleted with its session.
- Reloads and the copies page need the host running. Without it, the page says to run
  `slop open --browser`.

### Import: a snapshot, never the live file

- The CLI asks the document's owner for a clean copy into a temporary file:
  `Request::Copy` (`crates/hitslop-core/src/owner.rs`), which waits for the save and runs
  `Store::copy_clean`. A live document's owner does it through a new `copy` socket
  request (`packages/hitslop/src/schema/socket.ts`; the command protocol is exact, so its
  version rises); a closed one, under the writer lock, as every CLI edit runs. A save by
  the Mac app can then never mix versions into one import.
- The copy's worker writes the snapshot under a new copy ID in chunks
  (`begin_import_unchecked`, `write`, `finish`), reading the session by ranges, so the
  file never sits whole in WASM memory, which never shrinks. Unchecked, because a
  checked import rewrites header bytes 18–19 and the checks must see the bytes as they
  arrived. The owner then opens the copy with every open check.

### Download: drain, flush, export

1. The host asks the copy's page to flush: the existing host request `flush`
   (`packages/hitslop/src/schema/page.ts`), whose `drainAndSave`
   (`packages/hitslop/src/shell/owner/document.ts`) sends unsent text, settles queued writes and
   attachments, then flushes the owner.
2. The worker exports: `sqlite_dbpage` pages in page order, inside one read transaction,
   through a sync access handle into a host-origin OPFS file (`exports/<copy>.slop`).
3. The host downloads that file as a `File`. Not an in-memory Blob: WebKit cannot load
   one of the largest document's size. Not the VFS's export: it refuses an open file and
   reads it whole.

### One owner, one protocol

- The worker runs the real Rust owner and store, one connection, one WASM instance per
  open copy.
- It answers the existing page protocol through `command::page`
  (`crates/hitslop-core/src/command.rs`), the path native pages use, and pushes
  publications as native does. The shell's `call` (`packages/hitslop/src/shell/bridge.ts`) gets
  a worker sink beside WebKit's; `nativeTransport` is used unchanged.
- The host's own messages (import, export, asset reads, flush) are TypeBox-defined and
  generated. Pages and the SDK see no new API; authored slops need no changes.

### Origins and isolation

- **Storage** lives only on the host origin (`127.0.0.1`): the copies' pools, the copies
  index and exports. No authored code runs there.
- **Each copy's app runs on its own origin**, `<copy-id>.localhost:<port>`, so apps cannot
  read each other's web storage, Service Worker or channels, as each native window has its
  own ephemeral web storage (`DocumentSession.swift`, `.nonPersistent()`). The frame's
  message channel is bound to its own copy and validated at the Rust boundary.
  JavaScript transports bytes and drives browser APIs; it never interprets document
  state.
- **The Service Worker routes by client**: each client's port comes from the host, keyed
  by the sending client's ID (`event.source.id`), never one global port. That prevents
  mix-ups; the per-copy origin is the boundary.
- **Assets come through the Service Worker**, as `slop://app` does natively: the shell
  page and boot script are static; boot waits until the Service Worker controls the
  frame (it calls `clients.claim()` on its first load) and holds the copy's port, then
  loads `/assets/*`. Responses carry the generated CSP with `slop:` mapped to `'self'`.
  Brotli assets are decoded in Rust; ranges of stored-as-is assets are read through
  SQLite's blob I/O. The embed relay and the outbound-network policy carry over.
- **No cross-origin isolation.** COOP/COEP would also govern authored frames, breaking
  what the page CSP allows (`frame-src https:`, `img-src https:`, the YouTube relay).

## Saving

- One save scheduler, the owner's: the step machine coalesces saves as today (150 ms
  after an edit, at most 1 s after the first unsaved one) and emits a deadline; the
  browser driver sets a timer and runs each storage job the step machine emits inline in
  the worker. Save decisions, `Store::job`'s append-or-checkpoint choice and sequence
  bookkeeping stay in Rust. The saved version advances only after commit.
- Writes resolve after their publication; the page's reply acknowledges acceptance, not
  the save. `flush()` waits for accepted changes to commit and never resolves as a no-op.
  Attachments persist through the store before their reference edits can save.
- A closing tab cannot await a flush, so `visibilitychange` and `pagehide` flushes are
  best effort. A killed tab may lose changes not yet confirmed saved; it never corrupts
  the copy.
- Quota and storage failures show truthful status with a retry. Unsaved work is kept
  while the worker lives and is never labelled saved.

## Tabs, copies and cleanup

- Before installing a copy's pool, take its Web Lock with `ifAvailable`. A second tab on
  the same copy says the copy is open elsewhere.
- A dead owner's lock can be granted before its sync access handles close. A refused
  install is retried in a fresh worker after 50 ms, never in the same one: a failed
  install may leave its own handles open.
- Each copy has its own pool directory and VFS name, and the URL carries the copy ID, so
  a reload continues it.
- The copies index is in IndexedDB: copy IDs, states, titles, dates and preview artwork,
  never document content. IndexedDB and OPFS cannot change together, so:
  - an import writes its entry as `importing` before creating the pool, and `ready`
    after the open checks pass;
  - on host start and before listing, `importing` entries and their pools are removed,
    pools without an entry are removed when their lock is free, and entries whose pool
    is gone are shown as missing;
  - each copy keeps at most one `exports/` file, replaced by its next export and swept
    on host start;
  - deleting a copy takes its lock, removes its pool, then its entry.

## Durability and support

- Say plainly that a copy lives in this browser, profile and device, and is not backed
  up. Keep Download prominent.
- Check `navigator.storage.estimate()` before an import, and warn before SQLite reports
  the disk full.
- Feature detection requires OPFS, `createSyncAccessHandle` in a dedicated worker, Web
  Locks and Service Workers. Without them, say that browser editing can't be saved;
  never present an in-memory editor as saved.

## Size

The browser build uses a size profile (`opt-level = "z"`, fat LTO, one codegen unit) and
`wasm-opt -Oz`. The real core with storage, the owner and the page protocol linked
measured 3.86 MB raw and 1.24 MB Brotli that way. Budget: 4 MB raw, 1.3 MB Brotli. A
typical `.slop` is 70–200 KB on top.

## Storage library

rusqlite on sqlite-wasm-rs with sqlite-wasm-vfs's `sahpool` VFS: the same rusqlite API on
both targets keeps one implementation of the file and store code, and real SQLite C keeps
the format identical byte for byte. The alternatives considered are in the
[evidence](../docs/evidence/browser-storage-2026-10-06.md#libraries-considered).

Versions, as decided 2026-10-06:

- rusqlite 0.40.2, the latest, accepts sqlite-wasm-rs `^0.5.1`; `Cargo.lock` resolves
  0.5.5.
- **Never sqlite-wasm-vfs 0.2.0.** Its `xCheckReservedLock` always reports a reserved
  lock, so SQLite never rolls back a hot journal, and a worker or tab that dies
  mid-commit leaves a corrupt file. The spike reproduced it in WebKit and Firefox.
- **Land on sqlite-wasm-vfs 0.3.0 now, on rusqlite 0.40.2.** 0.3.0 tracks lock levels,
  as sqlite.org's own `opfs-sahpool` does; its sqlite-wasm-rs 0.6 requirement is for its
  own tests, and it runs on sqlite-wasm-rs 0.5.5. It passed every suite unpatched,
  including 50 mid-commit kills in WebKit (29 rolled back through the journal) and 10
  tab closes per engine.
  - It needs wasm-bindgen ≥0.2.128: move the pin, `js-sys` and the
    `generated/core-tools` CLI together (the spike used 0.2.129, the latest).
  - The worker provides its OS callback type (`rsqlite_vfs::OsCallback`: no sleep,
    `crypto.getRandomValues`, `Date.now`), as sqlite-wasm-rs 0.5.5 does without atomics.
  - Its files are managed through `VfsFilesManager`, transfers through `DbTransfer`.
- Move to sqlite-wasm-rs 0.6 when a rusqlite release accepts it (master has since
  2026-09-21); the crash test gates that upgrade too. Don't pin rusqlite to git: the Mac
  app and the engine use it. Pin exact versions. Upgrade Loro only with the corpus
  passing.
- sqlite-wasm-rs compiles SQLite C with `cc` for wasm32-unknown-unknown. That needs an
  LLVM clang with the WebAssembly target; Apple's clang has none (step 0).

`sahpool` constraints the design follows:

- One connection per database: repeated opens share storage but do not coordinate
  locks.
- No WAL; SQLite is built with `SQLITE_THREADSAFE=0`. A WAL-mode file cannot be opened
  at all (no shared memory) and is refused as a storage failure.
- New databases default to 8 KiB pages (`SQLITE_DEFAULT_PAGE_SIZE=8192`); imported files
  keep theirs. The browser host creates no file from scratch.
- Its pool files carry their own headers, so a backing OPFS file is not a `.slop`.
  Bytes go in and out only through SQLite or the pool's import and export utilities.
  sqlite-wasm-rs builds SQLite with `SQLITE_ENABLE_DBPAGE_VTAB`, so `sqlite_dbpage` can
  stream pages with the file open.

## Steps

In order. Refactoring adapters raises no format or ABI marker. The shared rules stay
shared: SQL, format and open checks, Loro save-job construction, limits and attachment
integrity.

0. **Groundwork.**
   - **Toolchain.** `scripts/build/core.ts` builds the WASM core's SQLite C with an LLVM
     clang that targets wasm32 (`CC_wasm32_unknown_unknown`, `AR_wasm32_unknown_unknown`;
     Homebrew LLVM on macOS, distribution clang on Linux). Development setup and CI
     install it.
   - **wasm-bindgen 0.2.129.** Move the pin and `js-sys` in
     `crates/hitslop-core-wasm/Cargo.toml` and the CLI version `buildCoreWasm` requires,
     together.
   - **Size.** A `wasm` Cargo profile (dist with `opt-level = "z"`) for the WASM build,
     and `wasm-opt -Oz` after wasm-bindgen.
   - **The WAL decision** ([open questions](#open-questions)), applied in the shared
     open check.
   - Done when `bun run verify` passes, `slop dev` works, and the size is recorded.
1. **Owner as a step machine** (`crates/hitslop-core/src/owner.rs`). No behavior change.
   - `Actor` takes a message and the current time and returns publications, storage
     jobs and its next deadline. Time is a value in shared code: `std::time::Instant`
     panics on wasm32-unknown-unknown.
   - The native driver keeps its threads, `mpsc` channels and persistence thread around
     the step machine. `command::page` uses no threads or clocks, so it carries over once
     `Owner::submit` has a browser driver.
   - Done when the owner, store, command and corpus tests and the Swift native tier
     pass unchanged.
2. **Portable file and store.** The spike's probe found four files that don't compile
   for wasm32 with storage: the registry, the socket, `file/copy.rs`'s renames and
   `file/artwork.rs`'s oxipng. They become native-only; the rest compiles.
   - `file/`: the checks already take `&Connection`. Path resolution, NOFOLLOW,
     `Staged`/`publish_new`, `sync_folder`, `pack` and `stage_assets` are native-only.
   - `store.rs`: `job`, `write`, `load` and attachments are portable. A browser store
     opens a copy by VFS name (`Connection::open_with_flags_and_vfs`) and reads assets
     through its one connection; the path, inode, `Lease`, `Moved` reconnect and second
     asset connection are native. Connection flags and durability are per platform; the
     rollback journal stays.
   - Storage dependencies (rusqlite, sha2, brotli, data-encoding, jsonschema) build for
     wasm32; libc and oxipng stay native. The browser never writes artwork.
   - Done when `hitslop-core` with storage compiles for wasm32 in the Rust tier, and
     every native test passes unchanged.
3. **The browser test tier**, in `scripts/verify.ts` and `docs/testing.md`, before the
   owner it tests. Playwright (already a root dependency) drives Chrome and WebKit from
   the spike's runner. It starts with the corpus and grows with each behavior below:
   - every `tests/compat` entry opens, reads as recorded, takes its scenario edit and
     reopens; an unedited round trip exports byte-identical to its snapshot, and an
     edited export opens natively with the same state, `$id`s and attachments;
   - worker kills and tab closes mid-commit, the tab lock, interrupted import and export;
   - the frame's assets, CSP and isolation, with two different apps open at once;
   - the largest document.
   - Done when the tier runs in `bun run verify` and in CI's macOS job.
4. **The browser owner** in `crates/hitslop-core-wasm`. One WASM build serves `slop dev`,
   the Bun tests and the browser; dev keeps the disposable `WasmDocument`.
   - The driver is the worker's `onmessage` plus a timer for the step machine's deadline.
   - Pools on sqlite-wasm-vfs `=0.3.0`, with an `rsqlite_vfs::OsCallback` (no sleep,
     `crypto.getRandomValues`, `Date.now`).
   - Page requests through `command::page`, chunked import, open with checks, asset
     reads and ranges, export into an OPFS file. Host↔worker messages are TypeBox schemas
     in `packages/hitslop/src/schema`, generated.
   - Done when the tier's corpus, crash and export cases pass.
5. **The host and frame** in `packages/hitslop/src/shell`, built by `scripts/build/shell.ts`.
   - Owner bootstrap: lock, fresh-worker retry, install, the copies index with its
     cleanup rules.
   - The frame on its copy's origin: boot waits for its Service Worker and the copy's
     port; the worker sink for `call`; the Service Worker routing by client.
   - The host page: open, save status, Download (drain, flush, export), the copies page,
     feature detection.
   - Done when the tier's isolation, tab and download cases pass in Chrome and WebKit.
6. **The CLI**, in `packages/hitslop/src/cli/app.ts` and the engine.
   - `open --browser`: the host process, its discovery file, health and control
     requests, import sessions, and opening the browser.
   - The `copy` socket request to a live owner, and the closed-file copy under the
     writer lock, writing the snapshot.
   - Done when a packed CLI works on macOS and Linux, a second invocation hands over to
     the running host, and the three port cases give their messages.
7. **Contracts and docs, in the same change as step 4.** AGENTS.md says "The WASM core is
   for `slop dev` and tests only", and architecture says "The WASM build never edits a
   durable document". Update AGENTS.md, architecture, the engineering contract and the
   roadmap together, keeping Rust ownership, the one native edit path, immutable
   masters, TypeBox wire authority and released-file compatibility. Add `slop open
   --browser` to the CLI guide and the public CLI docs, marked beta.

## Done, for the beta

Steps 0–7 pass, and these hold in Chrome and Safari itself, not only Playwright's WebKit:

- A document from the Mac app, open there with unsaved edits, imports as its saved and
  flushed state; editing it in the browser, closing the tab mid-save, reopening and
  downloading gives a file the app opens with the same state.
- Typing, then clicking Download without leaving the field, includes the typed text.
- Continuous typing on a near-limit document: append saves stay within one frame at p95,
  or the second worker is planned.
- Quota exhaustion shows a storage failure, keeps the committed state and loses no
  confirmed save.
- The largest document imports in slices in WebKit without holding it whole.
- Two different apps open at once, with identical asset paths and reloads, cannot reach
  each other's web storage, Service Worker, port or owner.
- An import or export interrupted by closing the tab leaves nothing behind after the next
  host start, and no copy is listed that cannot open.

Then archive the spike harness.

## Open questions

- **WAL files.** Native accepts a WAL-mode file and rewrites it to rollback mode,
  leaving a `-shm` file, though `file::connect`'s comment says it is refused; the
  browser cannot open one. Recommended: the shared open check refuses a WAL-mode header
  (bytes 18–19 not 1) with a clear message in both hosts. No hitSlop build writes one.
  Decide in step 0.
- **Safari and `*.localhost`.** Chrome resolves `<copy-id>.localhost` to loopback. Check
  Safari in the by-hand run (`bun run.ts serve` in `spikes/browser-storage`, then
  `/?auto=1&suites=storage,tabs,frame,durability,large`). If it does not, each open copy's
  frame gets its own port instead; frames hold no data, so those ports need not be stable.

## Spike results

The [evidence](../docs/evidence/browser-storage-2026-10-06.md) has the measurements; the
harness is `spikes/browser-storage/`.

- **It holds** in Chrome and WebKit: every native connection setting applies, the
  corpus reads and exports byte-identical, exports open natively, mid-commit worker
  kills and tab closes recover, and the largest document (183 MB) imports, opens and
  exports.
- **sqlite-wasm-vfs 0.2.0 corrupts on a mid-commit crash** (it never rolls back a hot
  journal). 0.3.0, which runs on today's rusqlite, passes everything unpatched.
- **Saves:** 5 ms at p95 for a typical document; about 300 ms for a 30 MiB checkpoint,
  which is OPFS write time, not the journal mode (WAL saves at most 15%). Append saves
  under continuous typing were not measured.
- **Memory:** WASM memory never shrinks, hence chunked import and streamed export.
  Opening the largest document takes 213 MiB.
- **Origins:** a host on `127.0.0.1` and a frame on `localhost` work in both engines, with
  the Service Worker serving modules, CSS, fonts, images and media ranges under the CSP.
  The spike used one frame origin and one global port, which is why this plan moves to
  per-copy origins and per-client routing.
- **Core port:** four native-only files block compilation; the rest of storage, the
  owner and `command::page` compile for wasm32.

## Sources checked 2026-10-05

- [sqlite.org `opfs-sahpool` source](https://sqlite.org/src/file/ext/wasm/api/sqlite3-vfs-opfs-sahpool.c-pp.js)
  (`xCheckReservedLock` reports real lock state; checked 2026-10-06) and
  [sqlite-wasm-vfs 0.3.0](https://docs.rs/sqlite-wasm-vfs/0.3.0)
- [rusqlite 0.40.2](https://crates.io/crates/rusqlite/0.40.2) and its
  [master Cargo.toml](https://github.com/rusqlite/rusqlite/blob/master/Cargo.toml)
- [sqlite-wasm-rs](https://crates.io/crates/sqlite-wasm-rs) 0.5.5 and 0.6.1
- [sqlite-wasm-vfs `sahpool`](https://docs.rs/sqlite-wasm-vfs/0.3.0/sqlite_wasm_vfs/sahpool/index.html)
  and its [0.2.0 pool utilities](https://docs.rs/sqlite-wasm-vfs/0.2.0/sqlite_wasm_vfs/sahpool/struct.OpfsSAHPoolUtil.html)
- [Turso compatibility](https://github.com/tursodatabase/turso/blob/main/COMPAT.md) and
  [Turso in the Browser](https://turso.tech/blog/introducing-turso-in-the-browser)
- [sqlite-wasm (crates.io)](https://crates.io/crates/sqlite-wasm)
- rsqlite-wasm 0.1.4: `_docs/rsqlite-wasm-main` (`LIMITATIONS.md`,
  `rsqlite-core/src/executor/pragma.rs`, `rsqlite-storage/src/pager.rs`)
- [SQLite `sqlite_dbpage`](https://sqlite.org/dbpage.html) and sqlite-wasm-rs 0.5.5
  `build.rs` (`SQLITE_ENABLE_DBPAGE_VTAB`)
- [SQLite browser persistence](https://sqlite.org/wasm/doc/trunk/persistence.md)
- [OPFS synchronous access handles](https://developer.mozilla.org/en-US/docs/Web/API/FileSystemSyncAccessHandle)
- [Web Locks](https://developer.mozilla.org/en-US/docs/Web/API/Web_Locks_API)
- [WebKit storage policy](https://webkit.org/blog/14403/updates-to-storage-policy/)
- [Browser quotas and eviction](https://developer.mozilla.org/en-US/docs/Web/API/Storage_API/Storage_quotas_and_eviction_criteria)
- [App Clip data handoff](https://developer.apple.com/documentation/appclip/sharing-data-between-your-app-clip-and-your-full-app)
