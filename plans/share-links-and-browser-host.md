# Share links and a persistent browser host

Status: proposed, 2026-10-05; revised the same day after two reviews. Product choices
are agreed and the storage library is chosen. Implementation has not started. The next
step is the [M0 spike](#m0-spike-plan), which gates the core port.

## Agreed product choices

- A share publishes a fixed copy of the document's current contents. Recipients edit
  independent copies; neither their changes nor the sender's later changes propagate.
- The sender signs in to publish and manage links. Recipients need no account.
- Browser copies autosave locally and can be downloaded as `.slop` files. No cloud
  saving of recipients' edits, cross-device recovery, or live collaboration in v1.
- Make Share visible in the floating document toolbar, immediately before `…`, and
  available through File → Share…. Keep the existing offline file-sharing action.
- An iOS app and App Clip are later clients. They are not prerequisites for this work.

## Sender and recipient flow

1. **Share** opens a popover; opening it uploads nothing. Explain: “Anyone with this
   link can make their own copy. Includes your document's current contents.” Offer
   **Create Link** and **Send File…**.
2. **Create Link** signs the sender in if needed, then returns to the document. Drain
   pending page edits and attachments, flush through the owner, and build the
   publication artifact ([Publication](#publication-artifact)). Never upload or copy a
   busy live SQLite file directly.
3. The sharing service accepts the authenticated upload, validates it without executing
   authored code, and stores an immutable artifact. An unguessable share ID resolves to
   it through an access-controlled share record:
   `https://hitslop.com/s/<share-id>`, with no local path or document contents.
4. Show **Copy Link**, **Send…** and **Stop Sharing**, plus the publication date. Send
   opens the native share picker with the URL. Failure offers retry, never a
   working-link state. Retrying one publication must not create duplicate shares.
5. Reopening Share retrieves the existing link. **Create New Link…** publishes a newer
   snapshot; it never changes what an old link means. The account keeps managing older
   links.
6. The share page shows the title, preview and **Use in Browser**, **Open in hitSlop**
   and **Download .slop**. Do not depend on installed-app detection; a user action
   invokes the app, with a browser/download fallback.
7. **Use in Browser** creates a new local copy from the artifact. Returning to the share
   offers **Continue Your Copy** when one exists locally, plus starting another copy.
8. **Saved in this browser** appears only after a durable save. Opening a browser copy
   in the native app needs a downloaded file; the share URL alone cannot carry local
   edits.
9. Stop Sharing prevents new hosted access. Existing local and downloaded copies stay
   usable, and a browser “Your copies” page reaches local copies whether or not the
   share is still active.

## Architecture

```text
hitslop.com/s/<share-id>       share page: title and preview from the share record; no WASM
        │ Use in Browser
host origin (trusted)          copies index, save status, download, open in hitSlop
  ├─ dedicated worker          Rust/WASM: owner + Loro + store + rusqlite + SQLite
  │     └─ sahpool VFS ──────► OPFS, one pool directory per local copy
  └─ iframe: copy origin       page shell + authored app
        └─ Service Worker ◄── MessagePort ──► worker: app assets, byte ranges
```

- **One worker per open copy.** The owner and the store share one WASM instance. SQLite
  is single-threaded there and `sahpool` is confined to its dedicated worker anyway;
  two instances would double Loro and SQLite memory and need a save-job protocol.
  Natively the two queues keep slow writes from blocking edits. In the browser, measure
  checkpoint latency first and split only if it blocks input noticeably.
- **The host origin holds storage; authored code never runs there.** Each copy's app
  runs on its own origin, with no access to OPFS, the copies index, other copies or
  owner controls. Its message channel is bound to its own copy and validated at the
  Rust boundary. JavaScript transports bytes and drives browser APIs; it never
  interprets or reconciles document state.
- **Assets come through a Service Worker, as `slop://app` does natively.**
  `SchemeHandler.swift` serves the constant shell page, `/__shell__/boot.js`, and the
  document's app assets, whole or as a byte range, under the generated page CSP. In the
  browser the shell page and boot script are static files. Boot waits until the copy
  origin's Service Worker controls the page and holds the host's MessagePort, then
  loads `/assets/*`, which the Service Worker answers from the owner worker. Responses
  carry the same generated CSP with `slop:` mapped to `'self'`. Brotli assets are
  decoded in Rust, because the browser does not content-decode Service Worker
  responses. The embed relay and the outbound-network policy carry over unchanged.
  The copy frame keeps `allow-same-origin`, which is safe because its origin is never
  the host's.
- **Asset fallbacks, if a Service Worker cannot control the copy frame in a target
  browser.**
  - (a) Put copy origins on subdomains of the host's own site. The frame is then
    same-site and not partitioned. The host keeps no cookie session; recipients have
    none.
  - (b) Serve published apps' immutable assets over HTTP from the service, by app
    digest, with byte ranges from R2. The Service Worker is then needed only for
    copies opened from local files.
  - Two other routes don't work:
    - `blob:` URLs: the CSP refuses `blob:` scripts.
    - Delivery over a MessagePort alone: the shell imports `assets/app.js` by URL
      (`packages/shell/src/app-module.ts`), and apps load relative modules, CSS
      `url()`s, fonts and media ranges.
- **Choose the host origin once.** OPFS data belongs to an origin, so moving it strands
  every browser copy.
- **No cross-origin isolation.** COOP/COEP would also govern authored frames, breaking
  what the page CSP allows (`frame-src https:`, `img-src https:`, the YouTube relay),
  and the roadmap already gives pages no `SharedArrayBuffer`. This rules out every
  SQLite route that needs it.

## Storage library

Keep **rusqlite**, on **sqlite-wasm-rs** with the **sqlite-wasm-vfs `sahpool`** VFS.
The same rusqlite API on both targets keeps `file.rs` and `store.rs` one implementation.
It is real SQLite C, so the format is identical byte for byte: rollback journal,
`auto_vacuum=FULL`, backup, limits and `DEFENSIVE` behave as natively. `sahpool` is a
Rust port of SQLite's own `opfs-sahpool`. It needs a secure context and a dedicated
worker, not COOP/COEP.

Rejected:

- **Turso** (Rust rewrite, 0.8.x; browser package `@tursodatabase/database-wasm`).
  - Its COMPAT.md says it supports WAL only and plans no rollback journal. Our files are
    `journal_mode=DELETE`, and native refuses WAL files.
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

Versions as of 2026-10-05:

- rusqlite 0.40.2, the latest, accepts sqlite-wasm-rs `^0.5.1`; `Cargo.lock` resolves
  0.5.5. That pairs with sqlite-wasm-vfs 0.2.0, which always pulls in tokio and
  `indexed_db_futures`.
- sqlite-wasm-rs 0.6.1 with sqlite-wasm-vfs 0.3.0 is leaner: `sahpool` is its only
  feature and wasm-bindgen is optional. It needs wasm-bindgen ≥0.2.128 (we pin
  `=0.2.127`) and a rusqlite release that accepts 0.6. rusqlite master already allows
  `>=0.5.1, <0.7.0`.
- Run the spike on 0.5.5 + 0.2.0. Land the port on 0.6 + 0.3 once rusqlite releases
  it, upgrading wasm-bindgen and its CLI in the same change. Pin exact versions.
  Upgrade Loro only with the corpus passing, as everywhere else.
- sqlite-wasm-rs compiles SQLite C with `cc` for wasm32-unknown-unknown. That needs an
  LLVM clang with the WebAssembly target; Apple's clang has none. `scripts/core-build.ts`,
  development setup and CI need one.

`sahpool` constraints the design follows:

- One connection per database: repeated opens share storage but do not coordinate
  locks.
- No WAL; SQLite is built with `SQLITE_THREADSAFE=0`.
- Its pool files carry their own headers, so a backing OPFS file is not a `.slop`.
  Bytes go in and out only through SQLite or the pool's import and export utilities.
  `export_db` returns the whole file in memory. sqlite-wasm-rs builds SQLite with
  `SQLITE_ENABLE_DBPAGE_VTAB` (0.5.5 `build.rs`), so `sqlite_dbpage` can stream pages.

## Rust core changes

The shared rules stay shared: SQL, format and open checks, Loro save-job construction,
limits and attachment integrity. Native operating-system services move behind native
code. Refactoring adapters raises no format or ABI marker.

- **`file.rs`.** The checks already take `&Connection`: `check`, `markers`, `layout`,
  `state`, `read_app`, asset `encode`/`decode` and `AssetReader` reads. `opened` also
  takes a path. These are native-only:
  - `resolve`, and the NOFOLLOW/canonicalize side of `connect`
  - `Staged` and `publish_new` (libc renames), and `sync_folder`
  - `pack` and `stage_assets`
- **`store.rs`.** `job`, `transaction`, `load` and attachments are portable. These are
  native-only: `path`, `inode`, `Lease`, the `connected`/`Moved` reconnect, and
  `asset_reader`'s second connection.
  - The browser store opens a copy with `Connection::open_with_flags_and_vfs` on that
    copy's pool.
  - Assets are read through the store's one connection.
  - Connection flags and durability settings are chosen per platform (`fullfsync`, the
    file lock and POSIX flags are native).
  - Keep the rollback journal; the spike proves it on `sahpool`.
- **`owner.rs`.** Make `Actor` a step machine: messages in; publications, storage
  actions and timer deadlines out.
  - The native driver keeps its threads, `mpsc` channels and the persistence thread.
  - The browser driver is the worker's `onmessage` plus `setTimeout`, and runs each
    storage action inline after the batch that produced it.
  - Save decisions and sequence bookkeeping stay in Rust. The saved version advances
    only after commit.
  - Shared code takes time as a value: `std::time::Instant` panics on
    wasm32-unknown-unknown.
- **Import and export hold at most one copy of the file in memory.** The largest
  document is about 182 MiB: 32 MiB of state, 100 MiB of attachments and 50 MiB of
  assets.
  - Import: write the downloaded bytes under a new copy ID with `import_db_unchecked`,
    then open the copy with every open check. A refused import is deleted before it
    is listed.
  - Not `import_db`: it forces header bytes 18–19 to rollback mode
    (`sqlite-wasm-vfs` 0.2.0 `sahpool.rs`). That would let through a WAL file native
    refuses. Our checks must see the bytes as they arrived.
  - Export: flush, then read `sqlite_dbpage` in page order inside one read
    transaction, streaming into a `ReadableStream`. `DEFENSIVE` allows reads, and
    memory stays at one chunk. `export_db` is the fallback if M0 rejects streaming.
- **Dependencies for the browser build:** SQLite, brotli decoding, sha2 and the
  jsonschema manifest validator. Check each one's target support and its size; don't
  just remove `cfg` guards. Authoring validation stays native; browser opening runs
  the shared checks. Artwork optimization (`file::optimize_png`, oxipng over the C
  libdeflate) is native-only today: the browser either stores artwork as captured or
  enables libdeflater's `freestanding` feature, measured like the others.
- **The worker protocol** is TypeBox-owned and generated. Pages and the SDK use the
  existing operations; authored slops need no OPFS API or storage rewrite.

## Browser host

- **Durable transport.** It replaces the disposable `browserTransport` for production;
  development and tests keep theirs.
  - Writes still resolve after their publication. `flush()` waits for accepted changes
    to commit and never resolves as a no-op.
  - Attachments persist through the store before their reference edits can save.
- **Saving.** Autosave while the page is active. Browser timers can be throttled and a
  closing tab cannot await a flush, so lifecycle flushes are best-effort. Download
  always awaits a real flush.
  - Quota and storage failures show truthful status with retry. Unsaved work is kept
    while the worker lives and is never labelled saved.
  - A killed tab may lose unacknowledged changes.
- **Tabs.** Before installing a copy's pool, take its Web Lock with `ifAvailable`.
  - A second tab reports that the copy is open elsewhere and offers **Use here**. That
    asks the holder over a BroadcastChannel to flush, `pause_vfs` and release. Never
    use `steal`.
  - Each copy has its own pool directory and VFS name, so different copies open in
    different tabs independently.
- **Copies index.** IndexedDB holds copy IDs, share IDs, titles, dates and preview
  artwork, never document content. Opening a pool is not needed to list it, and the
  index tolerates pools that have disappeared. Deleting a copy takes its lock and
  removes its pool.
- **Durability.**
  - Request persistent storage where supported. WebKit grants it by heuristics,
    chiefly for Home Screen web apps. Safari clears script-writable storage after
    seven days of use without interaction with the site.
  - Show whether `navigator.storage.persisted()` holds. Check
    `navigator.storage.estimate()` before an import, and warn before SQLite reports
    the disk full.
  - Say plainly that a copy lives in this browser, profile and device, and is not
    backed up. Keep Download and Open in hitSlop prominent.
  - Do not promise offline launching until runtime caching is built and tested.
- **Feature detection.** If OPFS sync access handles, module workers or Service
  Workers are missing, explain that browser editing cannot be saved and offer Download
  and Open in hitSlop. Never present an in-memory editor as saved.
- **Size budget.** Today's core binary is 3.9 MB raw (Loro and the core); storage adds
  SQLite, jsonschema, brotli and sha2. The share page loads no WASM; the runtime loads
  on Use in Browser.
  - The workspace release profile sets only `panic = "unwind"`, and
    `scripts/core-build.ts` runs no wasm-opt.
  - The browser build gets a size profile (`opt-level = "z"`, LTO, one codegen unit)
    and `wasm-opt -Oz`.
  - M0 records raw and compressed sizes and sets the budget.

## Publication artifact

Publishing is a new native owner operation. It writes a new file and never mutates the
original. Send File and Duplicate use the same operation
([clean copies](sqlite-format-review.md#1-clean-copies)), so no copy that leaves the
owner carries history or removed attachments; only captures keep the plain backup.

- **State without history.** Export a checkpoint at the latest frontiers
  (`state_only` or a shallow snapshot there). Only a cut before the latest version
  keeps deleted content in its base state (see `Store::close_job`).
- **Attachments that are still referenced.** Nothing deletes from `attachments` today,
  so a whole-file copy carries every attachment ever added, including removed ones.
  - Rule: include an attachment only when its ID (64 hex characters,
    `AttachmentIdPattern`) appears inside any string in the current state, including
    within longer text such as markdown. The scan reads materialized values, not
    bytes. Close-time reclamation uses the same
    [attachment scan](sqlite-format-review.md#attachment-scan).
  - A false inclusion would need the exact SHA-256 of a stored attachment. An app that
    stores a transformed ID loses that image in the recipient's copy, which breaks
    the image but leaks nothing.
  - A typed attachment descriptor kind would miss IDs inside text, and existing apps
    would still need the scan; it is an authoring feature, not a prerequisite.
- **Artwork** is rendered for the published state, not taken from the close-time
  artwork.
- **Checks.** The artifact must pass every open check, or publishing is refused with a
  clear reason. Invalid stored state is never repaired, and `$id`s are preserved.
- **Share ownership** lives outside authored state and in the account, so links stay
  manageable after the local file moves or is deleted.

## Sharing service and native integration

- **Cloudflare HTTP service with R2 artifacts**, matching the roadmap.
  - The Worker handles authenticated publication create and finalize, the sender's
    share list, anonymous capability reads and downloads, and owner-only revocation.
  - Uploads go to R2 through presigned URLs.
  - TypeBox owns every request and reply. Retries are idempotent.
- **Validation runs `slop-engine inspect`** in a Linux container; the engines workflow
  already builds `linux-x64` and `linux-arm64`. These are the same native checks the
  CLI runs, on real SQLite, executing no authored code. A WASM validator inside a
  Worker would hit the Worker memory limit at the largest documents. A share becomes
  readable only after validation passes.
- **Access control.** Artifacts stay private behind the share record. Public object
  URLs and caches must not bypass revocation. An unlisted link is bearer access, not
  an identity-restricted invitation. No public catalog.
- **Native Share flow.**
  - The toolbar button and File → Share… reach the same popover.
  - Sign-in and upload recover from failure.
  - Copy and Send the link; manage older links.
  - Keep offline Send File.
  - **Open in hitSlop** from the web passes the share ID to the app, which downloads
    and opens a new document.
- **Capabilities.** If [slop capabilities](slop-capabilities.md) lands first, its
  per-code consent applies before third-party slops are distributed, and native-only
  capabilities need explicit browser behavior.
- **Decisions to settle before M2:** account provider, metadata store, container
  host, upload limits and retention.

## Milestones and acceptance

1. **M0, spike** ([plan](#m0-spike-plan)). It decides the storage line, the worker
   model, import and export, the copy-origin layout, browser support and the size
   budget.
2. **M1, browser host on local files.** Open a `.slop` from disk, or in staging from a
   URL. Prove isolation, assets, saving, multiple tabs, the copies index, download,
   storage-unavailable behavior and the core refactor. Native and browser tests run
   against the same fixtures: `$id`, Unicode text, undo, themes, attachments. Every
   `tests/compat` entry opens through the browser storage path.
3. **M2, share links.** The publication artifact, the service, validation, revocation,
   the native Share flow, and the share page feeding M1's import.
4. **M3, Open in hitSlop** from the web, and older-link management polish.

Before shipping, exercise these cases:

- Termination during a commit, quota exhaustion, corrupt and truncated input, and
  newer markers (refused with nothing written).
- Repeated opens, two tabs, interrupted uploads and downloads, and revoked links.
- Artifacts containing no deleted text and no unreferenced attachments.
- One recipient's edits never reach another recipient or the publisher.
- Authored code cannot reach another copy, the host origin or the owner.

Run the applicable Rust, shell/WASM, Swift/native and compatibility checks from
[testing](../docs/testing.md).

## M0 spike plan

**Goal:** decide, with measurements, whether the browser path holds as designed:
- storage on rusqlite and `sahpool` in one worker
- the Service Worker asset frame
- memory at the largest document
- binary size

The spike changes no product code. Its results settle the
[open questions](#open-questions) and the browser support matrix for M1.

### Setup

- **Where.** `spikes/browser-storage/` is its own Cargo workspace, not a member of
  the root one, with its own `Cargo.lock`. It depends on:
  - `crates/hitslop-core` by path, without `storage`
  - `rusqlite =0.40.2`, sqlite-wasm-rs 0.5.5, `sqlite-wasm-vfs =0.2.0` (`sahpool`)
  - `wasm-bindgen =0.2.127`

  Beside the crate: a static harness (host page, owner worker, copy page and Service
  Worker) and a Bun script that builds everything and collects results.
- **Results.** Write `docs/evidence/browser-storage-<date>.json` and `.md`, and record
  the decisions in this plan. Then move the spike to `archive/spikes/browser-storage/`,
  as with earlier spikes.
- **Where it runs.**
  - Desktop browsers run against `localhost` origins wherever the browser treats them
    as secure.
  - Mobile runs, private windows and both domain layouts need the harness deployed on
    two domains. Deploying is the user's step.
- **Browser matrix:**
  - Desktop: Safari, Chrome and Firefox, each in a normal and a private window.
  - Mobile: iOS Safari and Android Chrome.
- **Fixtures** (copies only; never edit `tests/compat`):
  - **F1.** A copy of `tests/compat/dev/documents/fixture-collections.slop` with a
    theme override and one attachment, added with `slop apply --attach` and the
    app.
  - **F2.** Every `tests/compat/dev/documents/*.slop`.
  - **F3.** About 150 MiB, built from a template whose assets are random, so Brotli
    can't shrink them, with ten 10 MiB random attachments. That approaches
    `ASSET_BYTES` and `ATTACHMENT_BYTES`.
  - **F4.** Damaged inputs: a truncated F1, F1 with one corrupted page, F1 with
    `user_version` raised (a newer marker), a WAL-mode file, and a non-SQLite file.

### S0. Toolchain and size

1. Build sqlite-wasm-rs for wasm32-unknown-unknown with Homebrew LLVM (Apple's clang
   has no WebAssembly target). Record the exact setup `scripts/core-build.ts`,
   development setup and CI would need.
2. Build three binaries:
   - (a) today's `hitslop-core-wasm`
   - (b) (a) plus SQLite and `sahpool`
   - (c) (b) plus jsonschema, brotli and sha2, each reached through a real call so
     nothing is stripped
3. Build each with the current release profile, then with the size profile
   (`opt-level = "z"`, LTO, one codegen unit), then with `wasm-opt -Oz` on top.
4. **Record** raw, gzip and Brotli sizes for each.
5. **Proposes:** the size budget.

### S1. Storage in one worker

1. **Open.** In a dedicated worker:
   - Install `sahpool` with pool directory `copies/<copy-id>` and VFS name
     `<copy-id>`, then open with `Connection::open_with_flags_and_vfs`.
   - Apply the native connection settings: `file::connect`'s pragmas and `db_config`,
     and `configure_writer`'s `journal_mode=DELETE` and `synchronous=EXTRA`.
   - Record any setting the VFS rejects or ignores.
2. **Import F1** with `import_db_unchecked`. Run the open checks as SQL: `application_id`,
   `user_version`, the table layout against `file.rs`'s `SCHEMA`, and `quick_check`.
   Load it with `hitslop_core::Document::open`, using the app row's `AppSpec`, the
   checkpoint and the updates.
3. **Save.** Edit with `apply_batch`. Write checkpoint saves shaped like
   `Store::transaction`: an upsert of `Document::checkpoint()` plus
   `DELETE FROM updates`, under `BEGIN IMMEDIATE`. Store a 10 MiB attachment the way
   `store_attachment` does.
   - Appended updates need an export since a version, which `Document` doesn't
     expose. Checkpoint saves exercise the same transaction, journal and sync path.
4. **Reopen.** `terminate()` the worker, start a new one, reinstall the pool and
   reopen. Pass when the state equals `Document::state()` from before.
5. **Crash recovery.**
   - Kill the worker during a 30 MiB checkpoint commit, at 50 random points. Then
     close the whole tab mid-commit and reload.
   - Pass when every reopen rolls back to the last committed state through the hot
     journal and `quick_check` passes.
6. **Export** two ways:
   - (a) `sqlite_dbpage` in page order inside one read transaction, streamed into a
     `ReadableStream` and a Blob
   - (b) `export_db`

   Pass when:
   - Both are byte-identical.
   - An unedited import exports byte-identical to its input.
   - The edited export passes `slop-engine inspect`, opens in the app, and shows the
     same state.
7. **Corpus.** Every F2 document imports, passes the checks, loads, and exports
   byte-identical.
8. **Refusals.** Every F4 input is refused with its expected code, its pool entry is
   deleted, and no other pool file changes. The WAL-mode file must be refused as
   native refuses it. Repeat it with `import_db` to confirm that the header rewrite
   would hide it.
9. **Record:**
   - time to first state
   - import time
   - save latency (p50/p95) for small and 30 MiB checkpoints
   - time from edit to durable acknowledgment

### S2. Tabs and lifecycle

1. **Same copy, second tab.** Its `ifAvailable` Web Lock request is refused, so it
   shows "open elsewhere". **Use here** sends a BroadcastChannel request. The holder
   flushes, closes its connection, calls `pause_vfs` and releases the lock; the new
   tab takes the lock and installs. Pass when the old tab makes no write after the
   handoff and its edits are refused.
2. **Holder tab killed.** The browser releases the lock, and the second tab acquires
   it and installs the pool.
3. **Different copies** open in two tabs at once.
4. **Without the lock**, deliberately: record how `sahpool` fails when a second tab
   installs the same pool. This documents why the lock is required.
5. **Background tab.** Measure how timer throttling delays autosave, and how often a
   `pagehide` or `visibilitychange` flush completes.

### S3. Asset frame

1. **Copy page.** Serve the static shell page and boot script on the copy origin.
   - The copy page registers the Service Worker. Boot waits for the worker to control
     the page and for the host's MessagePort, then loads F1's `assets/app.js`.
   - The app's relative modules, a CSS `url()`, a font, an image and a seekable video
     (byte ranges) are served from the owner worker.
2. **CSP.** Serve the generated page CSP with `slop:` mapped to `'self'`. Confirm that
   `blob:` and inline scripts are refused and that the YouTube relay embed plays.
3. **Layouts.**
   - L1: copy origins as subdomains of the host's site.
   - L2: copy origins on a separate registrable domain.
4. **Per browser and layout, record** whether the Service Worker:
   - registers
   - controls the frame
   - survives a reload
   - serves ranges

   Also record how storage is partitioned.
5. **Isolation.** The copy frame cannot read host-origin OPFS, IndexedDB or the
   worker.
6. **Fallback (b).** If neither layout works in a target browser, serve the same
   assets over plain HTTP by app digest and record that path.

### S4. Memory and quota

1. **F3 at size.** Import, open and export F3 by both export methods, on every desktop
   and mobile browser.
2. **Record:**
   - success or failure for each method
   - peak WASM memory (`memory.buffer.byteLength`)
   - JS heap, through `performance.measureUserAgentSpecificMemory` where available
3. **Quota.** Fill storage close to the quota (Chrome's DevTools quota override, or
   filler copies).
   - Pass when the error maps to a storage failure, the committed state survives, and
     `estimate()` predicted the shortfall.
4. **Persistence.** Record the `persist()` and `persisted()` results per browser.

### S5. Core port probe

Time-boxed to two days, in a throwaway worktree that is never merged.
1. Enable `file`, `store`, `manifest` and `error` for wasm32 with `storage`.
2. List every compile blocker:
   - `libc`, `std::os::unix` and `std::fs`
   - `Instant`
   - jsonschema's dependencies and `getrandom` backends
3. Build the real binary and measure it.
4. **Gives:** the true size and the scope of M1's refactor.

### Exit

**Record these decisions in this plan:**
- **Storage line:** confirm rusqlite with `sahpool`, and say whether to stay on
  0.5/0.2.
- **Worker model.** Proposed thresholds, to adjust once data arrives:
  - A typical save never delays a publication past one frame (16 ms).
  - The largest checkpoint blocks the worker for less than 250 ms.
- **Import and export method.**
- **Copy-origin layout:** L1, L2 or fallback (b).
- **Size budget.**
- **Browser support** and the feature-detection rules M1 follows.

**Stop and reassess the browser-storage route** if any of these hold:
- Crash recovery leaves a file that `quick_check` rejects.
- Exports don't reopen natively, or aren't byte-identical when unedited.
- `sahpool` can't hold the largest document on desktop browsers.

## Contract updates and deferred work

This work extends WASM from dev and tests to production browser documents and brings
accounts and hosted sharing forward. Update AGENTS.md, architecture, the engineering
contract and the roadmap together when it lands. Keep Rust ownership, the one native
edit path, immutable masters, TypeBox wire authority and released-file compatibility.

Live rooms, sync, cloud recipient copies, presence, shared undo, public discovery and
App Clips stay separate. An App Clip can later hand a native `.slop` to the full iOS
app through an App Group container; its local data stays purgeable.

## Open questions

- **Domain layout for copy origins.** A separate registrable domain (L2) is the usual
  isolation. It makes the frame third-party, where storage partitioning and Service
  Worker support differ by browser. Subdomains of the host's site (L1) are same-site,
  and the host keeps no cookie session. Assets served by the service, fallback (b),
  cover published apps if neither works. S3 decides.
- **One worker.** Does a single worker meet input latency at the largest checkpoint?
  S1 measures it.
- **Streaming export.** Does `sqlite_dbpage` streaming meet the memory target on
  mobile, or does export fall back to `export_db` with a size limit for browser
  copies? S4 decides.

## Sources checked 2026-10-05

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
