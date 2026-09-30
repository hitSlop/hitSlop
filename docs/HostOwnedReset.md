# Host-owned reset: plan, deletions and remaining spikes

Status (2026-09-29): steps 1–6 are implemented. Step 1 made the Quick Checklist and
SDK meet the 1k targets (see §7). Step 2 deleted the legacy machinery, replaced runtime
negotiation with one bundled page shell, and shipped the fresh storage schema with
attempt-token recovery (bug 1). Step 3 split the owner queue from the persistence queue
(one write in flight, epoch-fenced completions, flush targets, close/discard fences;
S-B is `PersistenceSchedulingTests`). Step 4 added, next to the old API: validated
frontier tokens (bug 2), stateless `edit_text` with precomputed scripts (S-C chose fork;
`docs/evidence/stateless-text-2026-09-29.json`), text `set`, `sequence()`, `open` with
updates, UniFFI records, a typed `CoreError` and `BUILD_ID` (`hitslop-native
--core-build`). Step 5 moved Swift internals: typed `Storage` calls and a typed `SaveFailure`
(`full`, `busy`, `moved`, `invalidated`, `io`) that reaches the window; no reply cache;
page attachment/theme calls through the owner (`StorageBridge` deleted); the socket
lives with the owner and runs commands without a page barrier (a CLI edit no longer
blurs the focused field); a Swift-minted owner epoch that rotates on discard; one
Swift theme validator; one-way status with "pending" only on the dirty transition;
Retry Save and Discard on the owner; startup failure keeps the lock; CLI edits report
`{ids, sequence, value}`. Still open: view tokens for queued page work (they arrive
with the step-6 bridge), and a release check that the app and helper embed the same
core build (needs the app target to reference `DocumentOwner.coreBuildID`, verified
in an app build). Step 6 cut the wire over: the core has no drafts, sessions,
`command_current` or `splice` (text is `edit_text` or a whole-field `set`); publications
are `{previous, sequence, version, ops, issues}`; the page speaks `open`/`apply`/`text`/
`flush` with a view token and receives one ordered push stream (`__hitslop.publish`),
resyncing from a fresh snapshot on any gap (S-D); the SDK has a sequence-ordered store,
cached handles, a stateless text binding, one write queue, and a barrier that drains
pending work instead of joining a flush (bug 3); attachments import through a collector.
1k rows: open 0.6 s, checkbox p95 14 ms, drain 20 ms. Step 7 remains.

Once work starts, this document supersedes the migration machinery in
[LoroRustCutover.md](../archive/docs/LoroRustCutover.md) and [LoroHostPlan.md](../archive/docs/LoroHostPlan.md). The
architecture decision itself (Rust Loro owned by the host, UniFFI, async SDK) stands.

The plan comes from a full read of `crates/`, `packages/`, `apps/apple/`,
`examples/slops/quick-checklist/` and `scripts/`. Four parallel audits informed it:
- the Swift host;
- tests, scripts and docs;
- all 51 archived slops;
- a stress test of the design.

A second review then checked the risky parts: persistence, the attachment close barrier,
stream recovery, stateless text and page status. Three more audits verified its claims
in code. §1.1 records what was adopted, adapted or rejected. §1.2 records three existing
bugs those audits found.

Line references are to the working tree on `revision-2` at the time of writing.

**Decisions taken with the maintainer (2026-09-29):**

| Question | Decision |
|---|---|
| List size Quick Checklist must handle | About 1k rows without windowing. Targets, measured with the debug benchmark harness in one window: **1k rows** ready ≤ 1 s, checkbox acceptance p95 ≤ 50 ms, typing drain ≤ 50 ms; **5k rows** ready ≤ 3 s, checkbox acceptance p95 ≤ 100 ms |
| Page runtime | The app injects one page shell. All runtime contract, revision, sealing and ABI machinery is deleted |
| Text editing protocol | Replace the stateful draft protocol with a stateless one now |
| AGENTS.md process rules | Replace sealed-bytes, the five-column ledger and break-then-verify with short rules for the new architecture |
| Legacy and compatibility | **Nothing has shipped to production.** Start fresh: no legacy handling, no migration, no backwards compatibility, no version gates and no refusal messages for old packages or databases. There is no contract, revision or ABI number anywhere. A version marker is added only when there is a first public release to be compatible with |

---

## 1. Summary

**Does moving Loro into the host simplify the app? Yes, substantially.** Nearly every hard
problem in the old app came from one fact: *the engine lived in the view*. That forced:

- **Two edit paths.** An open window edited through its WebView. A closed document booted
  a hidden WebKit session (`headless.js`) just to run the engine.
- **A JS Loro and a page byte store.** The page held a CRDT replica (`memory.ts`,
  `storage.ts`) and shipped bytes to Swift through a dictionary RPC.
- **Versioned runtime bundles.** The app, the helper and the CLI each carried a sealed
  engine build, so every stored document could find a compatible reader. That meant
  contracts, revisions, a checksum ledger, historical replay and archive restore.
- **A frozen JS ABI with engine semantics.** The ABI embedded engine timing, and fixing a
  bug meant a new sealed revision.
- **Engine lifetime tied to the renderer.** A WebContent crash killed the document owner.

With `hitslop-core` in the host, all of that should disappear. What remains is:

1. A **Rust core** for document semantics.
2. A **thin Swift owner** for the lock, SQLite, the socket and window lifecycle.
3. A **page shell** that turns intents into requests and publications into an immutable
   snapshot. It holds no CRDT.
4. **Slops that contain only the app**: `app.js`, schema, initial data, theme and assets.

**Why a reset is needed at all.** The cutover was deliberately conservative. It wired the
new owner in *next to* nearly all of the old machinery, so the tree still compiles, tests
and ships:
- the old engine;
- the runtime ledger;
- 3.8 MB of WASM, twice, in the Mac app;
- request replay caches;
- owner sessions with gap resync;
- a stateful draft protocol;
- the JS-era storage RPC.

This plan deletes that machinery and reshapes each layer to the ideal form.

**Performance: what the evidence does and doesn't show.** The recorded window diagnostic
([evidence](evidence/loro-cutover-2026-09-29.json)) shows:
- **1k rows: checkbox acceptance p95 = 441 ms**, and a durable drain of 183 ms.
- **5k rows: never reaches `ready`.** WebContent spends its time inserting DOM.

The startup profile implicates DOM insertion, not Loro. **Edit latency has not been
attributed yet.** The candidates are Swift persistence scheduling (saves share the edit
queue), Quick Checklist markup and SDK fan-out (§7). Spike S-A attributes it before any
fix is ordered. Removing the unused WASM from the app saves package bytes; it doesn't
show a WebContent memory improvement. Closed CLI edits at 1k rows already pass (122 ms
p95 in a debug build, against a 150 ms budget).

### 1.1 Second review: what was adopted

| Review point | Verdict | Reason |
|---|---|---|
| A precise two-queue persistence protocol | **Adopted, plus a bug fix** | One SQLite handle and a compare-and-swap on `generation` require one write in flight. The existing +1 lost-reply guess is already a data-loss bug (§1.2) |
| Keep the attachment reference write inside the close barrier | **Adopted, with a smaller API** | `await import(); await set(ref)` loses the reference when close or capture starts in between. There is also an existing flush-sharing race (§1.2) |
| Don't destroy a healthy renderer on delivery trouble; check epochs | **Adopted** | The DOM holds unsent drafts and compositions. Nothing checks an epoch on queued page work or on callbacks today |
| Stateless text needs executable proof; never abort mid-batch | **Adopted and widened** | The `vv_to_frontiers` panic is reachable today; `LoroText::update` mutates while diffing; `fork_at` is O(history + state) and mints a peer; none of the suggested cases are tested |
| Track pending local work incrementally; `open` carries save state; define live `get` | **Adopted** | The initial state has no durable sequence or save error. Page status misses attachments, `bindValue` and composition at unmount |
| Keep the SQLite minimum-reader revision and an ABI integer; archive the sealed records | **Rejected** | Nothing has shipped, so there is no reader to protect. The reader revision can't trigger in production today. Git history is the archive |
| Verify app and helper core provenance | **Adopted as a build check only** | A release-time assertion that both binaries report the same core build ID (§4.2). Build integrity, not compatibility; there is no runtime gate |
| Update active contracts in the same change as the deletions | **Adopted** | `AGENTS.md`, `engineering-contract.md`, `versioning.md` and `testing.md` forbid the deletions, so they change in step 2 (§10) |
| Smaller corrections | **Adopted** | Insert ids must be supplied by the caller to make reruns safe; frontier tokens are usually smaller, not constant-size; persistence exports imported changes too; theme rules must be portable; "not Loro" was an overstatement |

### 1.2 Existing bugs found (fixed as part of the reset)

1. **Lost-reply recovery can lose data** (`DocumentOwner.swift:185-193`).
   - Save A commits generation G→G+1, but both its reply and the recovery `metadata`
     read fail, so the owner still believes G.
   - After another edit, save B sends G and gets `revision_conflict`. Recovery sees
     disk = G+1 = expected+1 and marks B committed.
   - B's updates are never written, and `dirty` is cleared.
   - Fix: each write stores an attempt token in the same transaction, and recovery
     compares tokens (§4.2).
2. **A page base token can crash the owner.**
   - Path: `text.rs:115-120` calls `vv_to_frontiers`, which ends in
     `shrink_frontiers(..).unwrap()` (loro-internal `oplog/loro_dag.rs:1308`,
     `version.rs:1014`).
   - Any version vector naming two or more peers, one of them unknown, panics.
   - Native poisons the owner, which can only recover by discarding unsaved edits. WASM
     has no unwind containment.
   - Fix: validated frontier tokens (§4.1).
3. **Attachment barrier race** (`document.ts:550-555`).
   - A barrier that joins an in-progress `flushTask` after it has passed
     `Promise.all(participants)` does not wait for a newly started import.
   - Close only survives because it runs `prepareClose` twice; capture and single
     barriers don't.
   - Fix: a looping barrier (§4.3).

Smaller existing issues fixed in passing:
- The Swift `PlatformContract` validator ignores `patternProperties` and
  `propertyNames`, so the theme schema is never enforced natively.
- Theme length counts grapheme clusters in Swift but UTF-16 units in JS. A value Swift
  accepts can then make every later window open fail.
- `theme.save` has no ownership check and can write after the lock is released.
- Capacity errors raised inside the SQLite transaction don't match the "Document is
  full" recovery UI.
- The live socket catch returns no error code, so the CLI says "outcome unknown" for
  work that never ran.
- `compact` and `close` failures publish no status.
- Unmounting a field mid-composition blocks every later flush with
  `composition_pending`.

---

## 2. What exists today (inventory)

### 2.1 Layers and their size

| Layer | Location | Size | Notes |
|---|---|---|---|
| Rust core | `crates/hitslop-core/src/` | 1,911 lines (`lib.rs` 934, `publication.rs` 640, `text.rs` 248, `identity.rs` 47, generated wire 42) | Descriptor kinds: text, boolean, counter, object, list(object). `set` accepts booleans only (`lib.rs:491-506`) |
| FFI / WASM adapters | `crates/hitslop-core-ffi`, `crates/hitslop-core-wasm` | 113 / 49 lines | FFI errors are a single `Failure{message}` string |
| Swift document module | `Sources/HitSlopDocument/` | `DocumentSession` 625, `Storage` 387, `SocketServer` 278, `DocumentCommand` 267, `DocumentOwner` 250, `OwnerCommands` 106, `SchemeHandler` 80, `RuntimeCatalog` 79, `StorageBridge` 75, `StorageProbe` 37 | Plus `Resources/runtimes/4/` (180 KB `index.js`, 3.8 MB WASM) |
| Swift host | `Sources/HitSlopHost/` | `SlopWindow` 1,007, `SlopRenderer` 309, `SlopCLIExport` 101 | Window lifecycle, capture, export |
| Page runtime + SDK | `packages/document/src/` | about 3,000 lines | Owner document 587, text binding 207, boot 211, capture 285, schema 402 |
| Old engine kept for tests | `packages/document/test-support/contract3/` | 3,014 lines (+194 support) | Synchronous engine on the `loro-crdt` npm package |
| Old-engine tests | `packages/document/tests/*` (12 files) | 3,335 lines | Only `owner.test.ts` (357 lines) tests the new SDK |
| Legacy scripts | `scripts/v1/` (13 files) | 1,641 lines | Compatibility replay, ledger, sealing, sensitivity, growth |
| Sealed fixtures | `tests/compatibility/` (59 dirs) | 33 MB | 56 are contract 3, which never shipped |
| Plan and ledger docs | `docs/` | about 2,650 lines across 9 docs | Plans, ledger, versioning, testing |

### 2.2 How a document opens today

1. `DocumentSession.Prepared` resolves the runtime directory through `RuntimeCatalog`,
   which requires `Resources/runtimes/4/{boot.js,index.js,identity.json,core/*.wasm}`
   (`RuntimeCatalog.swift:33`).
2. It then checks `storageRevision == 2` and creates the `DocumentOwner`.
3. `DocumentOwner.init` reads `assets/runtime.json`, requires contract 4 (again), loads
   SQLite through the dictionary RPC with base64 bytes, then opens the core. It opens
   with the checkpoint, then imports each stored update in its own FFI call, and each
   import builds a publication. Finally it parses `snapshot()` JSON just to read
   `session` and `sequence`.
4. The WebView loads `slop://app/` → `/__runtime__/boot.js` → `index.js`. The page asks
   for `config` over the `storage` handler and fetches `state.schema.json` and
   `initial.json`. It opens `OwnerDocument` through `nativeTransport` over the `owner`
   handler, imports `assets/app.js` and mounts it with `ctx`.
5. Only on the page's `ready` message does Swift start the socket server and write
   `state/host.lock` (`DocumentSession.swift:302-316`).

The bundled WASM is never loaded by a native page. `boot.ts:19` always selects
`nativeTransport` in the app. It ships only because `RuntimeCatalog` and `verifyCopies`
demand byte-identical copies of the CLI's dev runtime. It is embedded twice, once in
Resources and once in Helpers (`embed-hitslop-native.sh:85-99`).

### 2.3 How an edit travels today (a checkbox)

1. **Page.** `OwnerDocument.submit` mints a request id, structured-clones the batch,
   enqueues and `notify()`s.
2. **Swift receive.** The page posts to the `owner` handler. Swift serializes the whole
   request just to check its size (`DocumentSession.swift:249`), spawns a `Task`, then
   serializes the batch (`OwnerCommands.swift:22`).
3. **Owner.** `DocumentOwner.apply` parses and re-serializes the batch, sorted, and
   hashes it with SHA-256 for the idempotency cache (`DocumentOwner.swift:103-105`). Then
   Rust parses and applies it and returns a publication JSON string.
4. **Swift publish.** Swift parses the publication once in `didEdit`, only to read
   `patch.sequence` (`:154`). It pushes it through a new
   `Task { callAsyncJavaScript(...) }`, publishes a `"pending"` status, and schedules
   autosave. It parses the publication again in `bridge` for the reply
   (`OwnerCommands.swift:24`).
5. **Page receive.** WebKit converts the reply dictionary into a JS object, and the page
   runs a TypeBox `Check` over the whole reply (`transport.ts:17`).
6. **Duplicate delivery.** The page receives the same publication twice, once as the
   reply and once as the push. The second copy is dropped by the sequence check
   (`projection.ts:53`), but still calls `present()` and `notify()`.
7. **Page fan-out.** Each `notify()` runs every listener, including one per bound
   textarea (`document.ts:515`). There are about five notifies per edit. The `status`
   getter scans all drafts (`document.ts:87-96`).
8. **Status echo.** The page reports status back to Swift (`boot.ts:194-203`), which
   duplicates the owner's own status callback.
9. **Svelte render.** `visible` is a new array, so `animate:flip` measures every row
   (§7).
10. **Autosave.** 150 ms later, autosave runs **on the same serial queue** as every core
    call, with `synchronous=EXTRA; fullfsync=ON` (`Storage.swift:78`).

---

## 3. Target architecture

```
slop app.js (App + Svelte + svelte adapter) ──ctx──▶ page shell (host-injected; no CRDT)
shell: store{sequence,version,value,issues} · handles · bindText/bindValue · barrier · capture · theme
   │ postMessage: open · apply · text · flush · host services     ▲ ordered push stream:
   │                                                              │ publish[] · saved · failed · theme
   ▼                                                              │
Swift DocumentOwner
   owner queue ── core calls only
   persistence queue ── SQLite append/checkpoint, attachments, theme
   writer.lock · socket server for the owner's lifetime · one push stream per owner
   │ UniFFI records {sequence, ids, publication: String} · typed error enum
   ▼
hitslop-core (Rust): descriptor · validation · atomic batch · publications from Loro events
                     · counters · stateless edit_text · frontier version tokens

hitslop-native: acquires writer.lock → same owner in-process (no WebKit, no authored code)
                or forwards to the live owner's socket
slop dev / Bun tests: the same shell + hitslop-core-wasm transport (CLI package only)
```

### 3.1 Responsibilities

| Component | Owns | Does not own |
|---|---|---|
| `hitslop-core` | Descriptor interpretation, validation, `$id` rows, atomic batches, publications, issues, counters, text merges, byte export/import | Sessions, retries, persistence, transport |
| Swift `DocumentOwner` | Writer lock, core handle, SQLite, save scheduling, push stream, socket, typed errors, theme validation, attachments | Document semantics; parsing publications |
| Page shell | Snapshot store, handles, `change` collector, bindings, barrier, capture, theme application, host bridge | Any CRDT; any semantic validation |
| Slop package | `app.js` (App + Svelte + adapter), schema, initial data, theme defaults, assets | Engine, shell, any version metadata |
| CLI (npm) | Authoring, build, dev server (shell + WASM), forwarding to `hitslop-native` | Document edits itself |

### 3.2 Before and after

| Concern | Before | After |
|---|---|---|
| Closed-document edit | Earlier: hidden WebKit + `headless.js`; now: native owner | Native owner (unchanged), no WebKit |
| Compatibility surface | Runtime contract + revision + ABI + storage reader revision + sealed bytes + ledger + replay + restore | None; the app, helper, shell and CLI are built from one tree |
| Old packages and databases | Contract refusals, "prelaunch documents are not supported", "legacy database; migration is not implemented" | No special cases; anything that fails ordinary validation fails like any other invalid input |
| WASM in the Mac app | 3.8 MB × 2, never loaded | None |
| Engine copies | App resources, Helpers, CLI (must be byte-identical) | Native core in app/helper; WASM in the CLI only |
| Request idempotency | Swift reply cache + SHA-256 + a 100k-request limit, per session | None (nothing retries); the CLI sends explicit insert ids |
| Publication delivery | Reply and push both carry it; session + previous-sequence gap resync | One ordered push stream; replies carry `{sequence}` |
| Page session / owner change | `session` in every frame, `wrong_session`, resync | The WebView is replaced when the owner is replaced |
| Text editing | Draft id, sequence, parent, cached authored `LoroDoc`, retired set, release calls | One stateless request `{path, base, from, to, selection}` |
| Storage API | Dictionary RPC with base64 bytes, built for the JS bridge | Typed Swift methods on raw `Data` |
| Save scheduling | Same serial queue as edits; saves block edits | Separate persistence queue |
| CLI while a window is open | Every command runs the page close barrier and disables inputs | Commands go straight to the owner |
| Theme validation | Two implementations that disagree (Swift and JS) | Swift only; the page applies pushed variables |
| Save status | Owner → page → back to Swift | Owner → window and page |
| Errors | Strings sniffed with `contains("engine_panic")` | UniFFI error enum → `{code, message}` |

---

## 4. Detailed changes by layer

### 4.1 Rust core (`crates/hitslop-core`)

This is a targeted reshape, not a rewrite. The following stay: `publication.rs`
(event-driven, change-proportional publications), `identity.rs` (derived ids for
anomalous rows), counters, the atomic abort through `fork_at`, the issue scan and the
`Node` descriptor.

**Version tokens become frontiers.**
- Today's token is hex(JSON(version vector)) (`lib.rs:63-82`). Text requests decode it
  and call `vv_to_frontiers` (`text.rs:115-120`).
- **Latent crash.** Loro's `vv_to_frontiers` ends in `shrink_frontiers(..).unwrap()`
  (loro-internal `oplog/loro_dag.rs:1280-1306`). That fails for an unknown operation once
  two or more peers exist. A malformed or stale `base` from the page therefore becomes
  `engine_panic` and poisons the owner.
- **Fix.** Tokens become encoded Loro `Frontiers`. Before any use, on **every** path
  (fast, no-op and slow), check each ID with `oplog_vv().includes_id`. An unknown ID
  returns `stale_base`; it never reaches a panicking API. Then `frontiers_to_vv`
  (returns an `Option`) and `fork_at`/`diff` can be used. `export_since` converts with
  `frontiers_to_vv`.
- Tokens are usually smaller than version vectors (they grow with concurrent heads, not
  with every peer ever seen), but they are not constant-size.

**`edit_text`: the stateless text protocol.** It replaces `text.rs` and runs in this
order:
1. Resolve `path` on the owner. A removed row returns `path_not_found`, so a removed row
   can never be resurrected.
2. Validate the selection against `to` at UTF-16 boundaries before any mutation.
3. If `from == to`, return without publishing.
4. **Check identity.** The text container must have existed at `base`: `base_vv`
   includes the container's creating `(peer, counter)`, which normal `ContainerID`s
   embed. It must also be the container the path resolves to now. A row removed and
   reinserted with the same `$id` has a new container, so this fails with
   `path_not_found`.
5. **Compute the edit script before any mutation.**
   - Never call `LoroText::update` on the owner, or on a fork in the middle of a batch.
     It applies operations *during* the Myers diff (loro-internal
     `handler/text_update.rs:28-54`, `diff/diff_impl.rs:322-349`), so a timeout leaves a
     partial edit. Its `DiffHandler` is crate-private.
   - Candidate (proved in spike S-C): put `from` in a throwaway `LoroDoc`, run
     `update(to, timeout)` there, and capture the emitted `TextDelta` from a
     subscription. On timeout, discard the throwaway and fall back to a caret-hinted
     prefix/suffix splice. Then `apply_delta` to the target.
   - Delta indices are Unicode scalars, not UTF-16 (the `loro` crate is built without
     the `wasm` feature). Convert explicitly.
   - A multi-hunk script handles disjoint edits coalesced while a request was in flight,
     and repeated characters. A single prefix/suffix splice would turn them into one
     delete-and-reinsert of the middle, which merges badly with concurrent edits.
6. **Fast path:** if the owner's current text equals `from`, apply the script directly.
   This covers nearly every keystroke. The current code forks whenever the owner's
   version changed at all, even after an unrelated checkbox toggle (`text.rs:98,116`).
7. **Slow path**, only when this same field changed concurrently. Spike S-C chooses
   between:
   - **(a) fork:** `fork_at(base)`, require the fork's text to equal `from` (otherwise
     `stale_base`), apply the script, commit, import the delta and map the selection
     through cursors. Cost: O(history + state) per call (it re-encodes changes up to
     `base` and the whole state), and it mints a peer. Reuse one in-memory `text_peer`
     only while `owner_vv[p] == base_vv[p]`; otherwise mint and remember a new one.
   - **(b) transform:** `doc.diff(base, head)` for that container gives the concurrent
     delta. Transform the script's hunks through it and apply on the owner. No fork, no
     new peer, no import. Loro's `diff` validates frontiers and restores state, but its
     source is marked "needs testing".
8. Reply with `{sequence, authored, selection}`. `authored` is the version immediately
   after this edit on its own branch: the fork's version on the slow path, or the owner's
   version on the fast path. The merged text reaches the page through the ordinary push.

Why this is promising, and what still needs proof:
- Equality with `from` is a correct fast-path test. The only case where it differs from
  fork-and-merge is an ABA history, where the field returned to `from` through different
  characters. There the result is exactly `to`, which is what the user saw.
- The existing `tests/ancestry.rs` traces ("RabcXYZ", "遠abcXYZ", emoji selection, a
  removed focused row, a bad base) all walk through the design with today's results.
- That walkthrough isn't proof. Spike S-C (§6) must pass the full case list as executable
  tests before step 4 lands.

What it deletes:
- Rust: `Draft`, `drafts`, `retired`, `release_draft`, `detach_renderer`, and the error
  codes `wrong_session`, `draft_parent`, `request_conflict` and draft `unknown_outcome`.
- TypeBox: the `session`, `draft`, `sequence` and `parent` fields.
- SDK: draft ids and `releaseDraft` calls.

**Whole-field `set` on text.** The CLI and agents write a whole field with
`{type: "set", path, value}`:
- Implementation: the same precomputed edit script as `edit_text` (throwaway-doc diff
  with a timeout, prefix/suffix fallback), applied with `apply_delta`. Nothing is
  mutated before the script exists, so a timeout never aborts a batch halfway and never
  leaves stale container handles.
- Effect: `command_current`, `Intent::Splice`, the `CurrentIntent` schema and the
  current-state `stale_base` path can all go.
- Documented semantics: `set` replaces the owner's text **as it is at execution**. An
  outstanding concurrent draft may merge afterwards. Text that was accepted before the
  `set` is not guaranteed to survive it.

**Other core changes.**
- Remove `session` from snapshots and publications; Swift owns owner identity. Keep
  `sequence`.
- Remove `patchBuildMS` (instrumentation on the production wire).
- Add `sequence()`, and expose `open(schema, checkpoint, updates)` through FFI. The core
  already supports it (`lib.rs:768`); the FFI takes only a checkpoint, so Swift imports
  updates one call at a time.
- **FFI returns records and typed errors.** Return
  `ApplyResult {sequence, ids, publication: String}` and
  `TextResult {sequence, authored, selectionStart, selectionEnd, publication: String}`.
  Errors become `enum CoreError { Rejected{code, message, op_index}, Invalidated }`, so
  Swift never parses JSON or error strings.
- Small cleanups:
  - `resolve()` clones subtrees of `Node` on every intent (`lib.rs:462-466`); return a
    reference instead.
  - `examples/*.rs` stay only if a bench script uses them. Spike S-C adds slow-path
    samples at 5k rows with mature history to `cost_attribution.rs`.
- **Build ID.** Expose `hitslop_core::BUILD_ID` (a source and lockfile hash set at build
  time) through the FFI. `hitslop-native --core-build` prints it, and the release
  packaging check asserts that the app and the embedded helper report the same ID.
- The WASM adapter mirrors the FFI surface. Both adapters contain no semantics.

### 4.2 Swift owner (`HitSlopDocument`, `HitSlopHost`, `HitSlopNativeCLI`)

**Two queues, one write in flight.** Saves must never block edits, and the split must
not lose or misattribute a save.

- **Queue ownership.**
  - The **persistence queue** owns the single SQLite connection. *Every* `Storage` call
    runs there: `load`, writes, attachments and theme. A read on another queue during an
    open write transaction would see uncommitted data.
  - The **owner queue** runs core calls only and never touches SQLite.
  - The writer connection gets a `busy_timeout`. `SQLITE_BUSY` at commit (for example
    from a concurrent backup during Duplicate) is a definite failure, never a lost reply.
- **Owner-queue state:** `epoch`, `sequence`, `savedSequence`, `savedVersion`,
  `generation`, `rows`, `bytes`, `inFlight`, `saveFailure`, `closing` and `closed`.
  - The owner is the only writer, so it seeds `rows` and `bytes` at load and advances
    them on completion. It decides between append and checkpoint from these counters,
    not from a stale `metadata` read.
- **Job.** Captured on the owner queue: `{epoch, attempt: UUID, targetSequence, version,
  mode, bytes}`.
  - `bytes` is `export_since(savedVersion)`, or `checkpoint()` in checkpoint mode. The
    export includes imported changes; persistence covers every op, not only local ones.
  - **At most one job is in flight.** Edits that arrive during it coalesce into the next
    job. Pipelining would break the generation compare-and-swap.
- **Write** (persistence queue): `BEGIN IMMEDIATE`, compare-and-swap on `generation`,
  write rows, `generation += 1`, `last_attempt = attempt`, `COMMIT`.
  - **Lost reply:** recover on the same queue by reading `last_attempt`. If it equals
    the job's attempt, the write committed; otherwise it definitely did not. This
    replaces the +1 heuristic (bug 1, §1.2).
- **Completion** (posted back to the owner queue). Ignored if `epoch` changed.
  - **Success:** set `savedSequence` and `savedVersion` from the job; advance
    `generation`, `rows` and `bytes`; publish `saved(savedSequence)`. If
    `sequence > savedSequence`, schedule the next job. Example: a save captures sequence
    10, edit 11 arrives, save 10 completes. It acknowledges 10, the document stays dirty
    for 11, and save 11 follows.
  - **Failure:** set a typed `saveFailure` (`full`, `io`, `busy`, `moved`), publish
    `failed`, and keep all state. The next edit, flush or explicit retry tries again.
    Capacity errors from inside the transaction map to `full` too, so the "Document is
    full" UI always appears.
- **`flush()`:** capture `target = sequence`, make sure a job is scheduled, and resolve
  when `savedSequence ≥ target`. Reject on a failure that covers the target.
- **`close()`:**
  1. Set `closing`: new edits and ancillary writes are refused.
  2. Flush to the final target.
  3. `storage.close()` on the persistence queue, which releases the lock.
  4. Set `closed`.
  - If any step fails, clear `closing`, and keep the lock and all state.
- **`discard()`:**
  1. `epoch += 1` and cancel the autosave.
  2. Wait on a persistence-queue barrier so an in-flight write resolves; its completion
     is then ignored.
  3. `load` on the persistence queue.
  4. Rebuild the core on the owner queue, and publish.
- **Startup cleanup and cancelled opens** go through the same fences. Nothing calls
  `storage.close()` directly on a queue any more.
- **The closed-CLI and socket contracts are unchanged:** a reply means the edit is
  durable, and `hitslop-native` exits only after `close()` has released the lock.
- **Fault hooks for tests.** `testingPhase` becomes thread-safe and able to block. Add a
  hook that fails the recovery read. Spike S-B lists the required cases.
- Keep serial `DispatchQueue`s behind `async` wrappers rather than a Swift `actor`:
  - An actor does not guarantee FIFO order across unstructured tasks.
  - Blocking Rust and SQLite work would otherwise occupy the cooperative thread pool.

**Delete from `DocumentOwner`:**
- **The request reply cache:** `replies`, `replyOrder`, `retiredRequests`, `replyBytes`,
  CryptoKit hashing and canonical re-serialization (`DocumentOwner.swift:2,25-28,
  103-123,228`).
  - Nothing ever resends a request id. Each CLI retry mints a new UUID and only retries
    before admission (`DocumentCommand.swift:15-29,54`). `SocketClient` never resends.
    The page mints a new id per call.
  - The cache also causes a real failure: after about 100,256 applies in one session,
    every edit fails with "Request history limit reached" (`:111-113`).
- `replaceJSON`, `detachRenderer` and `releaseDraft`.
- The per-edit `"pending"` status push (`:156`).

**Typed storage.**
- Replace `Storage.call(["method": …])` (`Storage.swift:255-386`) with typed methods:
  `load() -> Loaded`, `append(Data)`, `writeCheckpoint(Data)`, `metadata()`,
  `putAttachment`, `readAttachment`, `listAttachments`, `loadTheme` and `saveTheme`.
- Bytes never become base64 inside the process.
- `StorageBridge.swift` (75 lines) is deleted. Page attachment and theme calls go through
  the owner, which also closes a gap: today they bypass the owner's closed/invalidated
  guard.
- `StorageProbe` (DEBUG, used by the CI crash matrix) moves to the typed API.
- **Fresh storage schema.** Two tables: `document(id=1, checkpoint, schema_key,
  generation, doc_id, last_attempt)` and `updates(seq, bytes)`.
  - Delete the `reader_revision` column everywhere it appears (`Storage.swift:27,32,
    39-42,71-77,91,114-140,149,159,249-254,285,293,299,317,370-373`), the
    `storageRevision` parameter, `checkReader()` and the `PRAGMA user_version` gates.
  - `user_version == 0` is also how `Storage` detects a new database today. Replace it
    with "`sqlite_master` is empty" before deleting the gate.
  - Delete the "prelaunch documents are not supported", "Legacy database; migration is
    not implemented" and "requires a newer storage revision" paths.
  - Delete the Bun re-implementation of the schema in `test-support/sqlite.ts`; the
    crash matrix verifies through `hitslop-native get` instead.
  - Keep the generation compare-and-swap, `doc_id`, the `schema_key` match, the bounds
    and the durability pragmas. None of those are versioning.

**The page bridge.** A single `hitslop` script message handler with a typed decode
replaces the `owner` and `storage` handlers.

| Page → host | Reply |
|---|---|
| `open` | state JSON string (`{sequence, version, value, issues}`) |
| `apply {intents}` | `{sequence, ids}` |
| `text {path, base, from, to, selectionStart, selectionEnd}` | `{sequence, authored, selectionStart, selectionEnd}` |
| `flush` | `{}` |
| `ready`, `failed`, `error`, `resize`, `attachments.put/read/list`, `theme.set/reset` | per method |

Errors are `{code, message}` from the typed enum. The code mapping at
`DocumentSession.swift:259-264` goes away.

**One ordered push stream per owner.**
- **Attaching.** `open` attaches the stream in the *same owner-queue job* that takes the
  snapshot, and finishes any previous continuation. The shell installs
  `__hitslop.publish` before sending `open`, and buffers pushes that arrive before the
  reply.
- **Order.** Yield into the stream on the owner queue. Never create a `Task {}` per push;
  that is what makes today's order uncertain (`DocumentSession.swift:104`). The consumer
  drains everything buffered into one `callAsyncJavaScript("__hitslop.publish(batch)")`
  and awaits it before sending the next. Awaited calls are ordered, and `publish` is
  synchronous in JS.
- **JSON handling.** Publication JSON strings are joined and passed through untouched.
  Swift never parses them.
- **Other push types.** The stream also carries `saved(sequence)`, `failed(error)` and
  the effective theme.
- **Contiguity.** Each publication keeps `previous`, and the page asserts it. `publish`
  is synchronous and isolates listener exceptions, so an authored observer can't break
  delivery.
- **Recovery without destroying a healthy page.** A gap, a page-side apply error or a
  buffer overflow triggers a resync; Swift drops the buffer and sends a `resync` marker.
  - The page calls `open` again and replaces its store.
  - Bindings keep their DOM drafts and `confirmed {text, version}`. Versions stay valid
    because they name owner history.
  - `reached(seq)` waiters resolve once the resynced sequence is ≥ `seq`, and reject on
    `owner_replaced` or close.
  - Only WebContent death replaces the WebView. Unsent drafts are then lost, and the
    recovery UI says so.
- **Epochs for queued work.** Replacing a view stops pushes but doesn't cancel work that
  is already queued.
  - `ownerEpoch` rotates on discard or owner reload. Each attached view gets a
    `viewToken`.
  - A page message captures both when it arrives. The owner-queue job rejects with
    `owner_replaced` if either changed.
  - Pushes carry the `viewToken`. Deliveries to a detached view are dropped.
- **Scope.** A background render (icon, preview, package export) already opens its own
  read-only snapshot owner (`SlopRenderer.swift:55-81`). Each owner has one stream.

**Lifecycle fixes (old-engine leftovers).**
- **The socket follows the owner, not the page.**
  - Today the socket and `host.lock` start on the page's `ready` and stop when the
    renderer dies (`DocumentSession.swift:302-316,552`).
  - Every socket request goes through a page `prepareClose`/`cancelClose` round trip
    (`:382-384`). That sets `blocked` and `disabled` on every bound input, so **running
    any CLI command blurs the field the user is typing in and cancels IME composition**.
  - New behavior: the socket starts when the owner opens. `get`, `apply`, `batch`,
    `theme.*`, `attachments.*` and `compact` run directly on the owner.
  - **A live `get` returns owner-accepted state** and flushes the owner only. Characters,
    or a whole composition, that exist only in the DOM are not included. One request can
    be outstanding while more typing stays local. Close, export and capture remain the
    operations that drain the page. The CLI help text (`app.ts:28-31`) and the skills
    say so.
- **Status flows one way.** Today it loops owner → page `__ownerEvents.status` →
  `hostCall status` → `onStatus`. New: owner → window, and owner → page for display.
- **Close flushes once.** Today close flushes three times: page `prepareClose`, then page
  `close` (which calls `prepareClose` again, `document.ts:402-404`), then
  `owner.close()`. New: the page barrier drains bindings, then `owner.close()` saves.
- **Retry and discard use the owner directly.** Retry Save and Discard call the owner.
  Today Retry Save goes through the page's flush and fails whenever the renderer is dead
  (`SlopWindow.swift:504-551`).
- **Startup failure keeps the lock.** Today a startup failure releases the writer lock
  and re-prepares the owner (`DocumentSession.swift:528-540,473-484`). The owner is
  fully loaded before the page starts, so keep the lock.
- **Epoch.** The socket epoch becomes a UUID that Swift mints for each owner and rotates
  on discard. Today it mirrors the core's `session`, and the
  `epoch = owner.session` after `detachRenderer()` does nothing (`:488`).
- **Label.** Rename "Reopen saved document" (`SlopWindow.swift:983`). It actually
  reattaches the live owner, including unsaved edits.
- **String-coupled errors.** "Document is full" (`SlopWindow.swift:950` ↔
  `DocumentOwner.swift:181`) and "Owner invalidated:" (`:501` ↔ `:86`) become cases of
  the typed enum.

**Theme validation lives only in Swift.**
- Today the same `theme set` is accepted or rejected depending on whether a window is
  open:
  - With no window, `OwnerCommands.swift:66-90` checks tokens, lengths (in grapheme
    clusters), `{};` and `var(--slop-*)` references.
  - With a window, `theme-runtime.ts:10-57` validates CSS with `style.setProperty`,
    counts UTF-16 lengths and applies a 64 KiB pre-check.
- New rules, enforced on writes in Swift only:
  - lengths in UTF-16 units;
  - no blank values and no `{};`;
  - known keys and known `var(--slop-*)` references;
  - 64 KiB on the merged result.
- Extend `scripts/v1/platform-validator.ts` with `propertyNames`,
  `patternProperties`, `pattern` and `maxLength`, so the TypeBox schema is actually
  enforced in Swift. The two relational rules (known key, known reference) stay in a
  small Swift function.
- **Page load never validates**, so an open can never fail on theme. The page applies
  the pushed variables and the browser ignores invalid CSS. The `setProperty` check is
  deleted.
- The browser dev server has no theme write path, so it needs no counterpart.
- `theme.save` requires ownership and a document that isn't closing.

**No package gate.** Runtime negotiation is deleted outright, with no replacement.
- Delete:
  - `RuntimeCatalog.resolve` and the rest of `RuntimeCatalog.swift`.
  - The `assets/runtime.json` checks in `DocumentOwner.init` (`:32-40`), which read the
    same file with different limits and disagree on `==` versus `>=`.
  - The `storageRevision == 2` check in `Prepared`.
  - The `runtime-info` subcommand.
  - `SlopTelemetryRuntime` and the `runtime_contract` / `runtime_revision` telemetry
    fields (12 call sites).
- A package is valid when its manifest, descriptor, initial data and assets validate.
  A package the current app can't run fails that ordinary validation, or fails at mount
  with the normal error UI.
- There is no ABI number. `ctx` is an internal interface between the shell and the
  Svelte adapter, both built from this tree, until a first public release needs a
  compatibility promise.

**Shell serving.**
- `Package.swift` copies `Resources/shell/` in place of `Resources/runtimes/`.
- `SchemeHandler` serves `/__shell__/*` from the app, and `/assets/*`,
  `state.schema.json` and `initial.json` from the package.
- Drop the `wasm` MIME type and `'wasm-unsafe-eval'` from the app's CSP
  (`SchemeHandler.swift:62,72`).
- `embed-hitslop-native.sh` copies only the shell.

**CLI helper.**
- No automatic replay, ever. An uncertain outcome prints "Outcome unknown — run slop
  get".
- `hitslop-native` prints the reply's `{ids, sequence}` along with the state, so a
  minted row id is visible.
- The insert field is `id` (not `$id`). A rerun is refused as a duplicate
  (`lib.rs:551-555`) only when the **caller** supplied that `id`, because a helper-minted
  id is new on every run. The skill tells agents to supply ids for inserts they may
  retry.
- The live socket catch returns a proper error code (`DocumentSession.swift:386`)
  instead of none.

### 4.3 Page shell and SDK (`packages/document`)

The target is about 1,200 lines of runtime source (including the capture code that
stays), against about 3,000 today.

**`store.ts`** holds `{sequence, version, value, issues, savedSequence, saveFailure}`.
`open` returns all six, so a page reattaching to an unsaved or failed owner shows the
right status immediately.
- `publish(batch)` processes publications in order and ignores any with
  `sequence ≤ current`.
- It applies operations grouped per list, building one `$id → index` map per list. Today
  every operation runs its own `findIndex` and array copy, which is O(n) per operation
  and O(n²) for a large batch (`projection.ts:3-36`).
- It freezes only newly created objects, so unchanged rows keep their identity.
- It resolves `reached(sequence)` waiters.
- It notifies **path subscribers**. An operation notifies its own path and its
  ancestors; a container `set` notifies its whole subtree. The Svelte adapter subscribes
  once, at the root. Today every textarea binding subscribes to the whole document, so
  a checkbox makes 1,000 bindings read `textarea.value`.

**`handles.ts`**
- Handles are built lazily and cached by path key. Today `doc.at(task)` builds and
  freezes a full handle tree on every call, twice per row per render.
- `doc.at(row)` resolves through a WeakMap from snapshot object to path.
- Writes are `set / insert / remove / move / increment / decrement`.
  - Each resolves after `reached(reply.sequence)`, so the snapshot has updated before
    the promise resolves.
  - `insert` mints the id and resolves `{id}`.
  - Handle writes share one FIFO queue, so `insert` followed by `move` cannot reorder.
- `change(tx => …)` collects synchronously. It rejects async, nested and throwing
  callbacks, and handles that escape the callback. Inside a collector, `insert` returns
  `{id}` synchronously.
- The text handle is just `set(value)`. `splice` and `replace` are removed.

**`text.ts` (bindText)**
- Each binding has at most one request in flight and holds a
  `confirmed = {text, version}` pair.
- **During composition,** it neither sends nor writes the DOM; it reconciles on
  `compositionend`. It writes `.value` only when the value differs, which preserves the
  browser's native undo stack.
- **When a request is in flight,** it waits for `reached(reply.sequence)`. Then:
  - **The DOM still equals what was sent:** show the store's text for the path and set
    `confirmed = {storeText, storeVersion}`. Use `reply.selection` only when
    `store.sequence == reply.sequence`. Otherwise map the caret with the prefix/suffix
    splice.
  - **The user typed meanwhile:** set `confirmed = {sent, reply.authored}`, and the next
    request goes from `sent` to the DOM value at `authored`.
- **When the bound handle changes** (the selected row changes), send dirty text for the
  old path, then show the new field. Today this throws "Flush the text draft before
  changing its binding" (`document.ts:528`), and four archived slops hit it.
- **On `destroy()`** it sends the final value. Tab switches unmount rows
  (`App.svelte:177`). Unmounting or blurring during a composition commits the current
  value instead of leaving a pending composition that blocks every later flush.
- Remote reconciles that rewrite `.value` reset the browser's native undo stack; local
  typing never does. That is accepted.
- A removed target blurs and disables. Programmatic writes update the auto-grow mirror
  (§7) or dispatch `input`.
- Text requests from different bindings may run concurrently.

**`bindValue`** stays, for booleans now and for all scalars when they land. Previews are
dropped until scalars land.

**Barrier (close and capture)**
- Block new sends; it does *not* set `disabled`.
- Then loop: `while (pending.size) await Promise.allSettled(pending)`. `pending` holds
  bindings with pending work and in-flight attachment operations.
- Then start a fresh flush. The barrier never joins a flush that has already passed its
  drain point (bug 3, §1.2).

**Attachments**
- API: `attachments.import(file, (tx, ref) => { tx.fields.photo.set(ref) })`.
  - The callback is a synchronous collector with the same rules as `change()`.
  - It covers the multi-field cases (codex-pet and soma-amp write two fields).
- Mechanism:
  1. `import` joins the barrier's `pending` set before its first `await`. It refuses to
     start while a barrier is active.
  2. The blob is written through the owner on the persistence queue: atomic and
     fsynced, as today.
  3. The collected intents are submitted with a per-operation admission token, so they
     pass an active barrier. This replaces the global `admittedCommit` flag.
  4. `import` awaits `reached(sequence)`, then resolves `ref` or rejects with the
     write's error. The five archived slops that never returned their write now see
     failures.
- Why not `const ref = await import(file); await set(ref)`: close or capture can start
  between the two awaits, reject the `set`, and leave a saved blob with no saved
  reference.
- Delete `stageSave`, `admittedCommit`, `participants` and the async `commit`
  callback. Rewrite `DocumentOwnerTests.swift:209-231`, which uses `replace`.

**Status**
- `save-failed` if `saveFailure` is set.
- Otherwise `saving` if `sequence > savedSequence` or `pendingLocal > 0`.
- Otherwise `saved`.
- `pendingLocal` is an incremental counter covering queued writes, dirty or composing
  bindings, in-flight `bindValue` commits and attachment operations. Nothing scans all
  bindings.

**Keep**
- `schema.ts`, with `defineDocument` and `s` trimmed to implemented kinds (§5).
- `Slop.svelte` with the export and icon capture snippets.
- `capture.ts`.
- `theme-runtime.ts`, now apply-only.
- `identity.ts`, `newID` only.
- A trimmed `ctx`.
- Export `window.resize` from the Svelte entry.

**Delete**

| File | Lines | Reason |
|---|---|---|
| `owner/session.ts` | 41 | Socket commands no longer enter the page; `discardPending` only throws |
| `owner/projection.ts` | 69 | Merged into `store.ts`; session/gap logic gone |
| `owner/text-binding.ts` | 207 | Replaced by stateless `text.ts` |
| `owner/transport.ts` | 62 | Replaced by the single-handler bridge and WASM dev transport |
| `session-types.ts`, `runtime-handle.ts` | 29 | Mirror dead members |
| `view-lifecycle.ts` dead members | — | `discardPending`, `retrySave`, `captureBegin`, `captureRestore` are never called; Swift calls `__hitslopCapture` directly |
| `memory.ts`, `storage.ts` | 86 | Page byte store; no live importer |
| `bridge.ts` base64 | — | Bytes never cross the page |
| `attachments.ts` `AttachmentController` | — | Legacy-only |
| `identity.ts` legacy helpers | — | `derivedID`, `effectiveIDs`, `isID`, `ID_KEY` are legacy-only |
| `boot.js` / `runtime-entry.ts` duplication | — | One shell entry |
| `runtime-identity.json`, `sdk-identity.json` | — | Deleted with no replacement. Remove their consumers in build, template-cache, native-helper, release-bundle and packed-test, including the CLI's check that a project's SDK identity matches the CLI's (`build.ts:73-83`) |
| `abi.ts` compatibility rules, `ctx.abi`, `ctx.capabilities` | — | `ctx` is internal; no "this interface only grows" rule, no capability list |

**CLI (`packages/cli`)**
- `build.ts` stops writing `assets/runtime.json` and writes no version metadata. Its
  plugin blocks imports from `/__shell__/`.
- `template.ts prepareRenderer` drops its runtime-capabilities comparison, and
  `runtime-capabilities.ts` is deleted.
- The dev server (`authoring.ts`) serves `/__shell__/` and `core/` from the CLI package,
  using a WASM transport that emulates the push stream.
- For tests, the emulated stream randomizes the order of replies and pushes.

### 4.4 Contracts (`packages/schema`)

TypeBox stays authoritative; `bun run schema:generate` produces the Swift and Rust
models.

| File | Contents |
|---|---|
| `core.ts` | `Intent` (set/insert/remove/move/increment), `Batch`, `TextEdit`, `PatchOp`, `Publication {sequence, version, ops, issues}`, `State`, `CoreError` |
| `page.ts` | Page → host messages and replies; host → page pushes |
| `socket.ts` | CLI ↔ owner |
| `manifest.ts` | Manifest. Drop the `runtime: "hitslop-v1"` field and the `$schema` const requirement (`manifest.ts:69`, generated `manifestReadSchema`); keep `$schema` as an optional editor hint |

Delete:
- `runtime.ts` (runtime identity, capabilities and requirements).
- The `bridge.ts` methods `load`, `metadata`, `append`, `checkpoint`, `config` and
  `runtimeRecovered`.
- `OwnerContractsSchema`, folded into `core.ts` and `page.ts`.
- `CurrentIntent` and `CurrentCommand`.
- In the generated Swift: `Runtime.generated.swift`, the dead `BridgeMethod` cases,
  `SocketRequest.import`, and `SocketDiscovery.epoch` and `pid`, which nothing reads.

---

## 5. The document DSL

### 5.1 Keep the author API shape

`defineDocument` / `s`, `useDocument`, `doc.current`, `doc.fields`, `doc.at`,
`change(tx => …)`, `bindText`, `bindValue`, `flush` and `<Slop>` with its capture
snippets are the right API. The archived slops are written against it. Do not design a
second API.

### 5.2 Advertise only what works

Today the `s` builder and the `Handle` types offer string, number, integer, enum,
optional, record, tree, rich text and scalar lists. The core rejects all of them at
runtime, and the owner throws "Unsupported descriptor" (`document.ts:370`). Slops are
authored by agents from these types, so the types must not promise capabilities that
don't exist. Export only the kinds that are implemented, and add each new kind with its
Rust implementation, SDK handle and a fixture in the same change.

### 5.3 Vocabulary demand from the 51 archived slops

| Kind | Slops needing it | Plan |
|---|---|---|
| text, boolean, list(object), object | Checklist, 39+ | Now |
| **string** | 39 | **Next milestone** |
| **enum** | 30 | **Next milestone** |
| **integer** | 19 | **Next milestone** |
| **number** (10 bounded) | 17 | **Next milestone** |
| **optional** (string, object, number, enum, text, integer) | 15 | **Next milestone** |
| scalar list | 8 (alien-radio, harada, meeting-notes, metronome, pixel-art, slide-deck, wordle, workout) | With the first of these slops |
| nested list(object) | 6 (flashcards, grade-calc, side-quest, slide-deck, trip-itinerary, weekly-planner) | The core already supports nesting; polish with the first of these slops |
| record | 5 (harada, wordle, habit-heatmap, pocket-sheet, morning-pages) | With the first of these slops |
| counter | 2 (koi-pond, side-quest) | Already implemented |
| tree, rich text | 0 | Do not implement |

Only one archived slop (Choice Point) fits the current core; 48 of the others `set`
non-boolean scalars. The first milestone after Quick Checklist is therefore **string,
number, integer, enum and optional**. It comes with scalar `bindValue` (committing on
`input`, coalesced) and scalar `preview` (for sliders and drawing), restored alongside
Small Expenses.

### 5.4 What this design already fixes for the archive

| Archive problem | Slops | Fixed by |
|---|---|---|
| Programmatic `replace` fails with `stale_base` if any write lands first | 13 (codex-pet, daily-planner, markdown-editor, …) | Text `set(value)` diffs against the text at execution time; no base |
| bindText throws when the bound row changes with a pending draft | 4 (grade-calc, morning-pages, slide-deck, trip-itinerary) | The binding follows handle changes |
| Attachment `commit` callbacks that don't return the write | 5 | `await import()`, then an ordinary write |
| `globalThis.slop.window.resize` used because `ctx.window` is unreachable | 2 (doodle-board, soma-amp) | Export `window.resize` |
| `data-slop-selection` / `data-slop-render` set but ignored | 32 | Implement in the shell or delete from slops (decide at migration) |

### 5.5 No optimistic overlay (deliberate)

Almost every slop drives controlled inputs from `doc.current`, so an input shows its old
value until the host accepts the write. Once acceptance takes a few milliseconds (the
performance gate), that delay is invisible. Previews cover drags and sliders.

The remaining async issues become **migration guidance** in the authoring skill:

| Pattern | Slops | Guidance |
|---|---|---|
| Read-modify-write from a stale snapshot (e.g. `set(qty + 1)`) | 17 | Use a `change(tx)` collector, or a semantic operation (`increment` on a counter) |
| Writes inside `$effect` or on mount ("ensure an entry exists") | 5 | Put defaults in `initial.ts`; make ensure-writes idempotent (record `put`) |
| Synchronous insert id / `change` result outside a collector | 8 | `await`; inside `change` the id stays synchronous |
| `try/catch` around unawaited writes | 4 | `await` the write |
| Reading `doc.current` right after a write | 5 | `await` the write; the snapshot has updated when it resolves |
| Cross-list move as remove + insert | 1 (weekly-planner) | `change(tx)` with remove and insert using an explicit id |

Revisit a pending-intent overlay only if a restored slop still feels laggy on the fast
owner. It would duplicate projection logic in TypeScript.

---

## 6. Spikes and measurements still worth doing

Do not run another engine or placement spike. Each spike below produces tests or numbers
that are kept: spike tests become the first tests of the suite that owns the behavior.
A spike must pass before the step that depends on it lands (§10).

| Spike | Why | Required evidence and pass criteria | Gates |
|---|---|---|---|
| **S-A Perf attribution** (first) | Edit latency is unattributed; fix order should follow data | `BenchmarkTests` with `HITSLOP_BENCH=1 HITSLOP_BENCH_ROWS=1000,5000 HITSLOP_BENCH_WINDOWS=1` (the default window grid is `[1,10,20]`), run three ways: autosave pushed to 5 s (DEBUG env var), `animate:flip` removed, and a slop that renders no DOM. Record open-to-ready, checkbox acceptance p95 and drain for each | Step 1 fix order |
| **S-B Persistence scheduling** | Two queues introduce new races (§4.2) | Thread-safe blocking `testingPhase`, plus a hook that fails the recovery read. Swift tests: (1) edit during a slow save acknowledges the captured sequence, stays dirty and saves again; (2) failed save followed by more edits, where one later write covers all of them and `generation` is unchanged by the failure; (3) lost commit reply, with and without a failed recovery read, loses nothing (**first reproduce bug 1 on today's code**); (4) flush during an in-flight write waits for its own target; (5) discard during an in-flight write ignores the late completion; (6) close during an in-flight write releases the lock only after the final write; (7) checkpoint threshold reached while an append is in flight; (8) document-full mid-flight surfaces the typed `full` error and UI; (9) `SQLITE_BUSY` from a concurrent backup is a definite failure. Existing closed-CLI, lost-reply and crash-matrix tests still pass | Step 3 |
| **S-C Stateless text** | Walking traces isn't proof (§4.1) | Rust prototype of `edit_text` with both slow-path candidates. Literal expected text and caret for: the ported `ancestry.rs` traces; two bindings on one field with delayed replies and CLI edits; repeated characters; disjoint edits coalesced while a request is in flight; caret-only moves; row switch or unmount mid-request; remove then reinsert with the same `$id`; malformed, unknown-peer and two-peer base tokens on the fast, no-op and slow paths, with no panic (**first reproduce bug 2 on today's code**); a whole-field `set` timeout inside a multi-intent batch leaves no partial state. Measure each slow-path candidate at 5k rows with mature history (about 10k ops and 20 peers) | Step 4; picks fork or transform |
| **S-D Stream recovery** | Delivery failure must not lose DOM drafts (§4.2) | SDK over WASM plus Swift: overflow or a gap triggers a resync, and dirty and composing text survive it; a request queued by a replaced view or an old owner epoch is rejected; `reached` waiters settle on resync, replacement and close; a throwing listener doesn't break the stream | Step 6 |
| Complete publication cost | The 2 ms gate was never measured in isolation | Timestamp owner commit → push issued → JS `publish` done, at 1k/5k rows, for a single-row edit and a 1k-op batch | Recorded, not gating |
| Memory | 258 MiB WebContent at 1k in a debug build | Re-run after the Checklist changes | Recorded, not gating |
| System IME, native undo/redo (manual) | Synthetic composition only | Kotoeri / Pinyin in title and task fields: compose, cancel, commit while a CLI `set` lands; Cmd-Z/Shift-Cmd-Z after local typing | Broad release |

Deferred until measurements justify them: SQLite journal tuning (WAL), a View Transition
for reordering, and list windowing.

Deferred design questions (not spikes now):
- Undo through Loro's `UndoManager` scoped per origin.
- Collaboration outbox and relay. Frontier tokens and stateless text are both compatible
  with them.
- History pruning, and schema evolution.

---

## 7. Quick Checklist performance plan

> **Outcome (step 1, 2026-09-29; [evidence](evidence/checklist-perf-2026-09-29.json)).**
> - **Root cause of the 1k edit latency:** SDK fan-out, not Loro or saves. Every notify
>   refreshed all ~1,000 text bindings, six times per edit, about 315 ms. Path-scoped
>   binding notifications fixed it.
> - **Root cause of the 5k startup failure:** WebKit mounting 5,000 textareas. Rows now
>   render as text and become a textarea only while being edited.
> - **1k:** all targets pass. Open 0.6–0.9 s, checkbox 12–17 ms, drain 12–21 ms.
> - **5k:** opens in about 1.2 s and accepts a checkbox in 12 ms. The macOS system
>   WebKit then spends about 170 ms on layout, which grows superlinearly (5/13/28/167 ms
>   at 0.5k/1k/2k/5k rows). Newer WebKit lays out the same page in 6 ms. The maintainer
>   accepted this: 1k is the product target.
> - The plan items below are kept as the record of what was tried.

**Targets.** 1k rows: ready ≤ 1 s, checkbox acceptance p95 ≤ 50 ms, typing drain
≤ 50 ms. 5k rows: ready ≤ 3 s, checkbox acceptance p95 ≤ 100 ms. The production ready
timeout stays 15 s (`DocumentSession.swift:209`). All are measured with
`HITSLOP_BENCH=1 HITSLOP_BENCH_ROWS=1000,5000 HITSLOP_BENCH_WINDOWS=1 bun run bench:windows`.

**Likely causes of today's numbers:**
1. **Saves may block edits.** Autosave fires 150 ms after an edit and runs on the
   owner's queue with full fsyncs, so any benchmark iteration slower than 150 ms queues
   the next `apply` behind a save. Typing after a pause may stall the same way. S-A
   measures how much this contributes.
2. **`animate:flip`.** Svelte measures every item of an animated keyed `{#each}` on
   every reconcile, whatever the duration. `visible` is a new array on every publication
   (`App.svelte:21`), so even a title keystroke forces two layouts of up to 1,000
   auto-sized textareas.
3. **Per-row component weight.** Each row has a bits-ui `Checkbox`, a
   `DropdownMenu.Root`, a textarea with a `ResizeObserver` and JS sizing
   (`sizeToText`/`scheduleResize`), and two `doc.at(task)` handle-tree builds.
4. **Main-thread churn.**
   - About five `notify()` fan-outs over about 1,000 bindings per edit.
   - An O(n) `status` scan.
   - The duplicate publication.
   - Two status pushes and a status echo per edit.
   - Swift parsing every publication twice.

   Each item is small; together they are noticeable.

**Changes, in order.** Items 2–6 work on today's SDK and ship before the SDK rewrite.
1. **Attribute first** (spike S-A). The persistence split follows in step 3, after
   spike S-B.
2. **Delete `animate:flip` entirely.** A row-count threshold would need two `{#each}`
   blocks, which remount (and lose focus) when the list crosses it. If the move
   animation matters, use a View Transition (deferred).
3. **Native checkbox.**
   - Replace bits-ui `Checkbox` with `<input type="checkbox">` and style `:checked` with
     a `data:` SVG background (the CSP allows `data:` images).
   - Update the CSS that keys off `[data-checkbox-root]` and `[data-state]`
     (`styles.css:283-298`).
   - Reset `checked` if the write is rejected. The browser toggles it before the owner
     accepts, and nothing reverts it otherwise.
4. **One shared actions menu.** Move a single bits-ui `DropdownMenu` out of the
   `{#each}` and anchor it with `DropdownMenu.Content customAnchor={el}` (supported in
   bits-ui 2.19.2).
   - `onInteractOutside` must ignore the anchor button, or clicking it to close reopens
     the menu.
   - `onCloseAutoFocus` prevents the default and focuses the anchor.
   - Set `aria-haspopup` and `aria-expanded` on the row buttons by hand.
   - Close the menu when its row disappears.
5. **CSS auto-grow textarea.** Replace `sizeToText`, `scheduleResize` and the per-row
   `ResizeObserver` with the grid "replicated value" pattern:
   - The wrapper is `display: grid`, with the textarea and `::after` sharing
     `grid-area: 1 / 1`. The `::after` sets `content: attr(data-value) " "`; the trailing
     space gives a trailing newline its own line.
   - Update `data-value` from the element on `input`, and from bindText's programmatic
     writes. Do not derive it from `task.text`, which lags while typing.
   - Match `font`, `letter-spacing` and `overflow-wrap: anywhere`. Leave 1–2 px of
     inline-end slack, because `overflow: hidden` clips any wrapping mismatch.
   - Keep `.grow > *` in the shared grid cell. Capture swaps inputs for spans
     (`capture.ts:236-254`), and a spare child would otherwise stack as a second row.
   - macOS 15.2 WebKit has no `field-sizing: content`, which rules out the pure-CSS
     alternative.
6. **`content-visibility` for off-screen rows.** Set
   `.checklist-row { content-visibility: auto; contain-intrinsic-size: auto 59px }`
   (17 + 17 + 24 + 1 px), so rows outside the viewport skip layout and paint.
   - Paint containment clips focus rings (2 px outline, 3 px offset,
     `styles.css:32-36`). Add inline padding with a matching negative margin.
   - Add `html[data-slop-capture] .checklist-row { content-visibility: visible }`,
     because the export view reuses `.checklist-row`.
7. **SDK changes** (§4.3), which remove the O(N) work per edit:
   - path-scoped binding notifications;
   - operations applied in batches;
   - lazy cached handles;
   - a single delivery per publication;
   - no status echo.
8. **Only if 5k still misses `ready`:** render off-screen rows as plain text and promote
   a row to a textarea on focus. This adds focus and keyboard-navigation work, so it
   stays last.

Record before and after numbers in a new file under `docs/evidence/`.

---

## 8. Everything that can be removed

The groups below are grouped for review; the atomicity rule decides what must land
together. A history guard makes the fixtures and the guard itself inseparable:
`compatibility-history.ts` is reached from `check`, `test`, `build` and `pack` through
`runtime-artifacts.releases()`, and in CI it rejects the removal of any fixture or ledger
row with contract ≥ 3.

CI is also unpassable today, for two reasons:
- `prepare-checks` runs `compatibility:restore`, which downloads `runtime-3-1` and
  `runtime-3-2` archives that exist on no GitHub release.
- `check:sealed-templates` can never pass: all 51 specimens are contract 3, and Quick
  Checklist builds contract 4.

**No runtime has shipped to production.** Every contract, revision, sealed runtime
and fixture in the tree is pre-release scaffolding. None of it gets a migration,
refusal message or compatibility test.

### 8.1 Old engine and its tests

| Path | Size | Notes |
|---|---|---|
| `packages/document/test-support/contract3/` | 10 files, 3,014 lines | Old synchronous engine on the `loro-crdt` npm package |
| `packages/document/test-support/{sqlite,writer-lock}.ts` | 197 lines | Bun reimplementation of SQLite format 2. Remove once crash-matrix verifies through `hitslop-native get` |
| `packages/document/tests/{convergence,handles,import,vocabulary,storage,session,bindings}.test.ts`, `helpers.ts` | about 2,400 lines | Old engine only |
| `packages/document/tests/{attachments,open,view-lifecycle}.test.ts`, `theme.test.ts` test 3, `types.ts` | about 900 lines | Delete **after** their REWRITE items land (§9.3) |
| `packages/document/src/{memory,storage}.ts` | 86 lines | Page byte store; no live importer |
| `loro-crdt` in root `package.json` and `bun.lock` | — | Also remove `verifyProvenance`'s unused lookups (`runtime-artifacts.ts:81-85`), otherwise the build breaks |
| `packages/document/node_modules/loro-crdt` (local) | — | Stale symlink |

### 8.2 Runtime contracts, sealing and replay

| Path | Size | Notes |
|---|---|---|
| `runtimes/` (`README.md`, `releases.json`, `storage-decisions.json`) | 104 lines | Ledger and a stale README describing `headless.js` |
| `tests/compatibility/{3-1,3-1-issues,3-1-saved-state,3-1-svelte,3-2-container-values}` | about 330 KB | Contract-3 fixtures |
| `tests/compatibility/template-*` (51) and `README.md` | about 33 MB | Contract-3 template specimens |
| `tests/abi/{fixture-schema.ts,plain/,svelte/}` | about 320 lines | Sources of sealed contract-3 consumers |
| `scripts/v1/compatibility.ts`, `compatibility-worker.ts`, `compatibility-check.ts`, `compatibility-history.ts` | 701 lines | Replay, digests and history guard |
| `scripts/v1/ci-baseline.ts`, `restore-runtimes.ts` | 112 lines | Baseline for the history guard; downloads of release archives |
| `scripts/v1/seal-templates.ts`, `runtime-release.ts` | 161 lines | Sealing |
| `scripts/v1/author-fixtures.ts`, `test-sensitivity.ts` | 550 lines | Contract-3 fixture authoring; fault injection into contract-3 sources |
| `scripts/v1/growth.ts`, `storage.ts`, `storage-child.ts` | 117 lines | Old-engine growth and Bun-storage diagnostics |
| In `scripts/v1/runtime-artifacts.ts` | — | `releases()`, `verifyReleasedIdentities`, `verifyStorageDecision`, `verifyProvenance`, the `contract < 4` catalog branch, and `verifyCopies` of `core/` into the app |
| In `scripts/v1/{test,check,check-built,release-check}.ts` | — | `checkRuntime` / `checkHistory` tails; the `check:sealed-templates` stage |
| Root scripts | — | `check:sealed-templates`, `compatibility:restore`, `test:compatibility`, `fixtures:seal`, `test:storage` |
| CI | — | The restore step in `.github/actions/prepare-checks`; sealed-template steps in `ci.yml` and `macos-release.yml` |
| `packages/cli/tests/compatibility.test.ts` | 295 lines | Specimen seals, contract-3 readers, history guard |
| `packages/cli/tests/runtime-artifacts.test.ts` ledger cases | — | Keep the byte-identity check only for the shell copies |
| `packages/document/src/{runtime-identity,sdk-identity}.json`, `packages/schema/src/runtime.ts`, `runtime.schema.json`, `packages/cli/src/runtime-capabilities.ts` | — | Deleted; no replacement |
| `apps/apple/.../HitSlopDocument/RuntimeCatalog.swift` | 79 lines | Negotiation; its contract-3 branch is unreachable |
| `apps/apple/.../HitSlopDocument/Resources/runtimes/4/` | 3.8 MB WASM + 180 KB | Becomes `Resources/shell/` without `core/` |
| `apps/apple/.../HitSlopCore/Generated/Runtime.generated.swift` | — | Runtime identity schemas |
| `apps/apple/.../Tests/HitSlopDocumentTests/RuntimeCatalogTests.swift` | 131 lines | Tests negotiation only |
| `RuntimeCompatibilityTests` refusal loop | — | Keep its contract-4 host path as `OwnerHostPathTests` |
| `hygiene.ts` `headless.js` exemption | — | Stale |
| `generated/v1/runtime-releases/` and baselines (local, gitignored) | 18 MB | Unused after deletion |

### 8.3 Version, compatibility and legacy handling (all of it)

Nothing has shipped, so none of this has a user to protect. It goes with no
replacement: no gate, no refusal message, no migration, no compatibility test.

| Item | Location | Notes |
|---|---|---|
| Manifest `runtime: "hitslop-v1"` and the `$schema` const requirement | `packages/schema/src/manifest.ts:69`; generated `SlopManifest.generated.swift` (`hitslopV1`), `manifestReadSchema`; `examples/slops/quick-checklist/manifest.json`, `packages/cli/templates/checklist/manifest.json` | Keep `$schema` only as an optional editor hint |
| `assets/runtime.json` | Written by `packages/cli/src/build.ts:180-187`; read by `DocumentOwner.swift:32-40` and `RuntimeCatalog.swift:64` | Packages carry no runtime metadata |
| Runtime catalog and contract directories | `RuntimeCatalog.swift`, `Resources/runtimes/<contract>/`, `packages/cli/runtimes/<contract>/`, `runtimeDirectory` (`build.ts:11`) | One `shell/` directory in the app; `shell/` + `core/` in the CLI |
| Runtime and SDK identity | `runtime-identity.json`, `sdk-identity.json`, the `./identity` export in `packages/document/package.json`, the project-SDK identity match (`build.ts:70-80`), `packages/schema/src/runtime.ts` | The CLI builds with whatever SDK the project resolves |
| Runtime capability handshake | `runtime-info` (`NativeCLI.swift:155-160`), `packages/cli/src/runtime-capabilities.ts`, `template.ts prepareRenderer` comparison, `DocumentSession.runtimeCapabilitiesData` | — |
| `ctx` versioning | `ABI` const, `ctx.abi`, `ctx.capabilities`, the "only grows" rule in `abi.ts:3-14` | `ctx` is internal |
| Storage revisions | `reader_revision` column, `storageRevision`, `checkReader()`, `PRAGMA user_version` gates and their messages (`Storage.swift`); `Prepared` revision check (`DocumentSession.swift:78-79`); `DocumentOwner.swift:44` | Fresh schema (§4.2) |
| Telemetry version fields | `SlopTelemetryRuntime`, `runtime_contract` / `runtime_revision` (`SlopTelemetry.swift:49-53,92-95,100-105,137-140`), 12 call sites | — |
| Failure reasons for old runtimes | `SlopFailureContext` `unsupportedRuntime` | — |
| JSON import and replacement | `ImportJSON.swift`, `DocumentCommand` import parameters, `SocketRequest.import`, the npm CLI `import` command (`packages/cli/src/app.ts`), `DocumentOwner.replaceJSON` | Refused-only code; returns when it's actually built |
| Refusal and compatibility tests | `DocumentOwnerTests.contractThreeIsRefusedBeforeStorage`, the old-runtime cases in `cli.native.test.ts` and `LoroCLITests`, `RuntimeCatalogTests`, the `RuntimeCompatibilityTests` loop, `runtime-capabilities.test.ts`, the storage-revision case in `DocumentSessionTests` | — |
| Version language in docs and skills | "Contract 4 / ABI 2" and "only hitslop-v1" in `skills/hitslop-authoring`, `skills/hitslop-document`, the CLI help text (`app.ts:86`); `AGENTS.md` lines 3-7 and 12; `docs/versioning.md` (delete, no rewrite); `docs/history/runtime-reset.md` (archive) | — |
| Naming | "contract", "revision", "runtime" (for the page shell) in code, telemetry and file names | Use "page shell"; rename `HitSlopDocument/Resources/runtimes` → `shell` |

The only numbering constraint left is the npm registry: `@hitslop/*` versions already
published there can't be reused.

### 8.4 Swift leftovers of the old engine

| Item | Location | Replacement |
|---|---|---|
| Reply cache, hashing, history limit | `DocumentOwner.swift:2,25-28,103-123,228` | Nothing (no retries); explicit CLI insert ids |
| `replaceJSON`, `detachRenderer`, `releaseDraft` | `DocumentOwner.swift:136-141,210` | — |
| Publication parsing in Swift | `DocumentOwner.swift:154`, `OwnerCommands.swift:24,28` | UniFFI records; strings passed through |
| Dictionary storage RPC + base64 | `Storage.swift:255-386`, `DocumentOwner.swift:49-62,174-179,214-218` | Typed methods |
| `StorageBridge.swift` | 75 lines | Owner handlers |
| `owner` + `storage` message handlers | `DocumentSession.swift:241-350` | One `hitslop` handler |
| Per-push `Task` | `DocumentSession.swift:102-118` | Ordered stream |
| Socket started on page `ready`, stopped on renderer death | `DocumentSession.swift:302-316,552` | Socket for the owner's lifetime |
| Page barrier around every CLI request | `DocumentSession.swift:382-384` | Direct owner calls |
| `theme.*` routed into JS | `DocumentSession.swift:378-381` | Swift theme owner |
| Status echo | `DocumentSession.swift:330-334`, `boot.ts:188-203` | Owner → window/page |
| `epoch` mirror | `DocumentSession.swift:20,93,488` | Swift-minted owner UUID |
| Lock released on startup failure | `DocumentSession.swift:473-484,528-540` | Keep the lock |
| Duplicate storage-revision checks | `DocumentSession.swift:78-79`, `DocumentOwner.swift:44` | One constant |
| Triple flush on close | `SlopWindow.swift:884-907`, `document.ts:402-404` | One barrier + owner close |
| JSON import (always refused) | `HitSlopNativeCLI/ImportJSON.swift` (27 lines), `DocumentCommand` import parameters, `SocketRequest.import`, 16 MiB import allowances | — |
| Unreachable `.schema` owner branch | `OwnerCommands.swift:64-65` | — |
| Dead `BridgeMethod` cases | Generated `Contracts.generated.swift` | — |
| `wasm` MIME and `wasm-unsafe-eval` | `SchemeHandler.swift:62,72` | — |
| Duplicate WASM embed | `embed-hitslop-native.sh:85-99` | Shell only |

Optional deduplication, recommended but not required:
- The "take the lock or find the live socket" logic appears in both
  `SlopCLIExport.swift:43-56` and `DocumentCommand.swift:75-88`.
- The hello/epoch handshake is repeated.
- The capture scaffolding in `SlopRenderer.capture` and `targetPNGData` is duplicated.
- Export telemetry is duplicated between the window and the CLI export.
- The NSApp setup code is duplicated.

### 8.5 SDK leftovers

See the delete table in §4.3. In addition:
- **Types:** `async-types.ts` indirection where possible, and the types for rich text,
  tree, record and scalar lists until they are implemented.
- **Schema:** `TextRequest` draft fields and `CurrentIntent`.

### 8.6 Rust leftovers

| Item | Location |
|---|---|
| Stateful draft protocol | `text.rs` (248 lines), rewritten as `edit_text` |
| `command_current`, `Intent::Splice`, `stale_base` path | `lib.rs:807-824,507-539` |
| `session`, `patchBuildMS` | `lib.rs:731,750,803,892-910` |
| FFI `Failure{message}` | `hitslop-core-ffi/src/lib.rs:7-16` |
| `detach_renderer`, `release_draft`, `command_current` FFI/WASM exports | Both adapters |

### 8.7 Docs, spikes and local disk

| Path | Action |
|---|---|
| `docs/NextPhasePlan.md` (528) | Archive (superseded JavaScriptCore plan) |
| `docs/LoroHostPlan.md` (590) | Archive; rationale summarized in the new architecture doc |
| `docs/LoroRustCutover.md` (421) | Archive when this plan lands |
| `docs/test-ledger.md` (508) | Archive; no new ledger |
| `docs/benchmarks/v1/`, `docs/evidence/render-profile-2026-09-25.json` | Archive |
| `docs/versioning.md` | Delete (no versioning policy until a first public release) |
| `docs/testing.md`, `engineering-contract.md`, `guides/development.md`, `guides/releasing.md`, `docs/README.md`, `crates/README.md` | Rewrite for the new model |
| `AGENTS.md` | Rewrite non-negotiables (§11) |
| `spikes/` (116 tracked files) | Move to `archive/spikes/`; nothing live imports it |
| Spike build dirs (`.build`, `target`, `dist`, `node_modules`), about 5.5 GB local | Delete locally (untracked) |
| `examples/slops/.svelte-check/` (ignored) | Delete locally; stale output for archived slops |

---

## 9. Tests: start fresh at three boundaries

### 9.1 Rules

- Every test names an observable failure at the boundary that owns the behavior.
- Delete a test in the same change as the code it protects.
- Never bend production code to keep an old test compiling.
- No tests of private call sequences, CSS strings or contract numbers.
- The frozen, testable contracts are the wire envelopes, save-before-close ordering and
  lock ownership.

### 9.2 Suites

| Boundary | Suite | Proves |
|---|---|---|
| **Rust** (`cargo test --locked --workspace`) | `crates/hitslop-core/tests/` | Table fixtures `{schema, initial, intents → value \| error}` for the Checklist kinds (extend `fixtures/checklist.json`); atomic rejection, then continued use, export and reopen; row identity and derived ids; publication equals a fresh snapshot (keep `chaos.rs`, `publications.rs`); counters (keep `counters.rs`); `edit_text`: the S-C case list; text `set` timeout fallback inside a batch; `export_since(saved)` includes imported changes; out-of-order import rejected; FFI panic invalidates the owner |
| **Bun SDK over WASM** | `packages/document/tests/sdk.test.ts`, grown from `owner.test.ts` | Write resolves after the snapshot updates; unchanged rows keep identity and stay frozen; collector rules; FIFO handle writes; bindText at 0/20/100/500 ms reply delay with typing in flight, composition, a concurrent CLI `set` on the same field, row switch, removed row; reply and push order randomized; path-scoped notifications (a checkbox does not touch other bindings); barrier drains; attachments (blob before reference, limits, rejection vs fault); `schema.test.ts` canonical key; fast-tier replay of `tests/fixtures/4-1*` on Ubuntu |
| **Swift** (`bun run swift:test`, `bun run test:native`) | `HitSlopDocumentTests`, trimmed `HitSlopHostTests` | Create, edit, save, reopen; failed save keeps lock and edits and retry succeeds; the S-B persistence cases; the S-D epoch and stream cases; writer-lock contention between app and helper (never stolen); closed `hitslop-native` apply with no WebKit; a live CLI edit reaches an open window without blurring the focused input; publication order under interleaved page and CLI edits; storage contracts (symlinked `state/`, relocation, capacity failure keeps edits, checkpoint reply loss), added only where missing; `OwnerHostPathTests` (get, apply, theme, compact, PNG/PDF) |
| **Bench** (manual, not a CI gate) | `BenchmarkTests`, `cost_attribution.rs` | §7 gate; `fork_at` at 5k |

### 9.3 Keep and rewrite

**Keep:**
- All `crates/*/tests` and their fixtures.
- `owner.test.ts` as the seed of `sdk.test.ts`, `platform-contracts.types.ts` and
  `cli.native.test.ts`.
- The schema and CLI feature tests.
- All Swift suites, with `Loro*` renamed to `Owner*`.
- `tests/compatibility/{4-1,4-1-svelte}`, moved to `tests/fixtures/`.
- `tests/abi/{owner-svelte,probe}`.

**Rewrite at the new boundary** (only the behavior worth protecting):
1. **Attachments.** Blob before reference, dedupe, limits, lost acknowledgement,
   rejection vs fault → `sdk.test.ts`. Swift `AttachmentTests` already covers the native
   side.
2. **View lifecycle.** Reload keeps flushed edits; capture waits for durability; a failed
   save keeps the view → `sdk.test.ts`.
3. **Theme.** Keep `ThemeController` tests 1–2 as they are, without the legacy imports.
   Serialization moves to a Swift test, now that Swift is the only theme owner.
4. **Live units from `open.test.ts`.** The canonical `schemaKey` test moves to
   `schema.test.ts`.
5. **Storage contracts.** Check against `DocumentSessionTests` and `LoroEngineTests`;
   add only what is missing.
6. **Convergence.** Confirm Rust covers "exported once" and "out-of-order rejected".
7. **`types.ts`.** Rebuild the compile-time checks against the trimmed `s` and `Handle`
   types.
8. **`DocumentOwnerTests.fixture()`.** Build the skeleton from `4-1` instead of `3-1`.
9. **`crash-matrix`.** Verify through `hitslop-native get`, and move `MemoryStore` out of
   `src/`.

---

## 10. Sequencing

Quick Checklist creates, edits, saves, reopens and exports at the end of every step.
The checks are `cargo test`, `bun run check && bun run test`, and (on macOS)
`bun run build && bun run swift:test && bun run test:native`.

Active contracts and docs change **in the same step** as the behavior they describe,
not at the end.

| Step | Contents | Exit criteria |
|---|---|---|
| **1. Measure + Checklist markup** | Spike S-A. §7 items 2–6 on today's SDK | Numbers recorded against the targets |
| **2. Legacy deletion + shell** | §8.1–§8.3 in one atomic change (history guard together with the fixtures); `Resources/shell`; no runtime metadata in packages; fresh storage schema with `last_attempt` recovery (bug 1); CLI build, dev and template changes; rebuilt native fixtures; the REWRITE items whose old tests go; rewritten `AGENTS.md`, `engineering-contract.md` and `testing.md`; `versioning.md` deleted | CI passes again; the app contains no WASM |
| **3. Persistence** | Spike S-B, then the two-queue protocol (§4.2), typed save errors, and `theme.save` ownership | S-B tests pass |
| **4. Additive Rust** | Spike S-C, then `edit_text`, text `set`, validated frontier tokens (bug 2), precomputed scripts, `sequence()`, `open` with updates, UniFFI records, typed errors and `BUILD_ID`, all next to the old API. Swift updates its error catch sites | Rust suites pass; the old API is still wired |
| **5. Swift internals** | Typed storage; delete the reply cache; owner-lifetime socket without the page barrier; owner epochs and view tokens; single theme owner and validator fixes; one-way status; CLI ids and error codes | Swift suites pass; a CLI edit doesn't blur the focused field |
| **6. Atomic wire cutover** | Rust removals (§8.6); Swift bridge, push stream and resync (S-D); SDK store, handles, text, status and barrier; the attachments collector (bug 3); WASM dev transport with push emulation; regenerated schemas | Full end-to-end Checklist verification (§12) |
| **7. Docs, skills, templates, DSL types** | New `docs/architecture.md`; `packages/cli/templates/checklist` and `skills/*` (async migration guidance); trimmed `s`/`Handle` types; archive old docs and spikes | Docs describe only the live system |
| **8. Next milestone** (separate plan) | Scalars + `bindValue` + `preview`; restore Small Expenses | — |

Why this order:
- **Wire changes land together.** Removing drafts, `command_current` or sessions in
  Rust breaks Swift and the SDK at once, so all wire-level removals wait for step 6.
- **Behavior-neutral work goes first.** App markup, the queue split and deletion don't
  change the wire, so they land early and produce real numbers before the rewrite.

---

## 11. AGENTS.md: new non-negotiables (draft)

These replace the rules about preserving sealed runtime bytes and release records, the
five-column ledger, and break-then-verify.

- `hitslop-core` (Rust) owns document semantics. The Swift `DocumentOwner` owns the
  writer lock, SQLite, saving, the socket and the push stream. The page shell
  holds no CRDT, and slops contain only the app.
- **One edit path.** The CLI forwards to the live owner or takes the lock and runs the
  owner in-process. Never steal or unlink `writer.lock`. Closed edits never start WebKit
  or run authored code.
- **Nothing has shipped: start fresh.** No legacy handling, migrations, backwards
  compatibility, version gates, refusal messages or compatibility tests. The app, helper,
  page shell and CLI are built from one tree. Add a version marker only when a first
  public release needs one.
- **TypeBox owns the wire.** Run `bun run schema:generate`; never edit generated files.
  Swift passes publication JSON through without parsing it.
- **Writes are async.** They resolve after the snapshot updates. Collectors are
  synchronous. Reads come from immutable snapshots.
- Merged anomalies are preserved and flagged, never repaired on read.
- Flush before close or export. A failed save keeps ownership and shows a native retry.
- Descriptor kinds exist in the types only once Rust, the SDK and a fixture implement
  them.
- **Tests live at the owning boundary:** Rust semantics, the SDK over WASM, and Swift
  integration. Delete tests together with the code they protect. No tests of private
  call sequences, CSS strings or contract numbers.

---

## 12. Verification of the finished reset

- `cargo test --locked --workspace`.
- `bun run check && bun run test`.
- `bun run build && bun run swift:test && bun run test:native`.
- `bun run slops:dev`: Checklist edits in a browser through the WASM transport.
- Manual app run with Quick Checklist:
  1. Type in the title while running `slop apply` with a `set` on the same field. The
     input keeps focus and both edits survive.
  2. Toggle, reorder through the shared menu, file finished tasks, and restore them.
  3. Type, then close immediately. Reopen and check that the text is there. Export PNG
     and PDF.
  4. Kill `hitslop-native` during a closed edit, then reopen. Expect the old or the new
     state, never a torn one.
  5. Start an attachment import and close immediately; reopen and find both the blob
     and its reference.
  6. Kill WebContent mid-typing and recover. Force a stream overflow and check that
     dirty text survives the resync.
  7. Type during a deliberately slow save; discard during a write.
- The §7 benchmark gate, with before and after numbers recorded under `docs/evidence/`.

---

## 13. Risks

| Risk | Mitigation |
|---|---|
| Stateless text regresses an IME or ancestry edge case | Spike S-C's case list lands before the cutover, next to the old API; SDK tests at 0–500 ms delay with randomized reply/push order; manual system IME before broad release |
| The slow text path is expensive at 5k rows | Spike S-C measures fork versus transform with mature history; it runs only for concurrent edits to the same field |
| The persistence split loses or misattributes a save | One write in flight, epoch-checked completions, attempt-token recovery, fences for close and discard (§4.2); spike S-B's race tests |
| Push-stream backpressure stalls the page | Batched drains; bounded buffer; overflow triggers a resync that keeps DOM drafts (S-D) |
| Deleting the history guard hides accidental fixture edits | The remaining fixtures (`4-1*`) are exercised by Swift and Bun tests; nothing sealed remains to guard |
| Removing a kind from the types breaks an archived slop's type check | Archived slops are not checked; each kind returns with its first slop |
| A package or database created by an earlier development build | Nothing has shipped. Such files fail ordinary validation or are recreated from templates; no migration and no special message (approved) |
| npm registry versions | `@hitslop/*` versions already published to npm can't be reused, so the next publish must use a higher version number. That is the only numbering constraint; there is no compatibility promise |

---

## 14. Summary of the simplification

This removes:
- a second engine;
- an in-page byte store;
- a WebKit edit path, which is already gone but still has code and docs;
- runtime contract negotiation and a sealed-bytes ledger with its replay, restore and
  history-guard tooling;
- every version number, gate and refusal path: runtime contract and revision, ABI,
  storage reader revision, SDK identity matching, the manifest `runtime` field and the
  JSON import that only ever refused;
- 3.8 MB × 2 of unused WASM in the app;
- a request replay cache that eventually refuses edits;
- session and gap-resync logic in two languages;
- a stateful text-draft protocol;
- a storage RPC designed for a JS client;
- duplicate theme validation;
- a status echo loop;
- a page barrier on every CLI command;
- about 8,600 lines of legacy tests, test support and scripts, and 33 MB of sealed
  fixtures. A small part of that behavior returns as much smaller tests at the new
  boundaries.

What remains:
- a Rust core that already works;
- a Swift owner with two queues and one push stream;
- a page shell of about 1,200 lines;
- slops that contain only their app;
- tests written for the architecture that actually runs.
