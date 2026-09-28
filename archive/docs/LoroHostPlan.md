# Loro in the host: architecture and proving spikes

Status: architecture decision and spike record, updated 2026-09-29.
[LoroRustCutover.md](LoroRustCutover.md) owns the agreed SDK, compatibility policy,
production milestones and current gate table. It supersedes the JavaScriptCore
direction in [NextPhasePlan.md](NextPhasePlan.md). Implementation contracts change
alongside the milestones that land, not merely because these plans were written.

**Approved:** host-owned Rust core + UniFFI; breaking runtime contract 4 / ABI 2
without contract-3 migration; slop archival with Quick Checklist as the first port;
async ordinary writes, synchronous transaction collectors and explicit durable
flush. Keep sealed historical bytes and release records. System Japanese/Chinese
IME gates broad release, not the limited signed trial.

The second native-owner iteration passed the spike's ordinary edit-latency,
render and drain gates (5k checkbox/move 3/3 ms versus 8–10/9–10 ms for the
Swift-native control). The third iteration added publication hardening, CLI
race/kill evidence and exact integer counters. This is implementation evidence,
not a Rust-versus-Swift language comparison or proof that the full production SDK
is complete. The production SDK, native owner and CLI are now wired and tested;
[the cutover checkpoint](LoroRustCutover.md#implementation-checkpoint) records that
evidence separately. Production performance, matched memory and complete publication
budgets remain open, along with the installed trial and packaging.

See the [binding report](../spikes/hitslop-core/REPORT.md) and
[native-owner report](../spikes/hitslop-core/NATIVE-OWNER.md) for current and
historical measurements. The first integration's 39/45 ms results and full-value
publication bottleneck are historical; event-driven publication replaced that
implementation. Native hosts have no renderer CRDT. WASM builds target browser
development/tests and later server work, not a second replica inside the native view.

## 1. Decision in one paragraph

Keep Loro. Move document ownership out of the WebView and into the host,
behind a **hitSlop-owned Rust crate** (`hitslop-core` = the `loro` crate plus
hitSlop's document semantics). The crate has a coarse API of JSON and bytes:

- Swift and the Cloudflare Durable Object handle only opaque bytes and JSON
  strings.
- The WebView becomes a view that sends **typed intents** and receives
  **patches**.
- The CLI and agents send the same intents to the same owner.
- There is one edit path, no optimistic replica in the view, and no JSON
  mirror.
- The same crate builds as a native library for macOS/iOS (UniFFI) and Tauri,
  and as WASM for `slop dev`, Bun tests and the Durable Object.

## 2. Why

### 2.1 The problem we actually have

Before the cutover, the CLI had two edit boot paths:

- With the document open, edits go over the socket into the WebView's Loro.
- With it closed, `HitSlopWasm/DocumentCommand.swift` booted an invisible
  WebKit session (`WasmSession(headless: true)`, `headless.js`) just to run
  the engine.

Both paths already shared the same semantic engine. Their different boot/lifecycle
paths exist because **the engine lives inside the view**. Native ownership removes
the headless WebKit dependency and ties engine lifetime to the document owner rather
than the renderer. The native cutover retains one semantic engine.

### 2.2 The options we measured

| Direction | Evidence | Verdict |
|---|---|---|
| Loro-WASM in each WebView (today) | Current runtime; `spikes/engine-placement` baseline | Two CLI paths; ~50 MiB WASM per window; the engine dies with the renderer |
| JSON ops + TS reducer in host JavaScriptCore | `spikes/document-authority/REPORT.md` | Fine at ≤1k rows; **fails its gates at 5k** (27.6 ms apply vs. a 5 ms target; 51–54 ms render); unsuitable at 40k (233 ms, 818 MiB peak). No JavaScriptCore on Windows/Linux. Text OT, rebase and offline editing are left for us to build. |
| Native Loro owned by the host | `spikes/engine-placement/REPORT.md`; `_docs/hitslop-sync` spikes 2–4 | Partial Swift interpreter: 2–6 ms checkbox/move up to 5k rows; 29/33 ms at 40k in the single-window cell. Stress tails failed some every-run gates. Earlier native experiments proved hosted relay feasibility with different semantics/storage. |

The native result includes indexing and smaller publications, not placement alone.
The older placement candidates retain fork staging; the newer Rust core proves
atomic rejection with in-place edits and recovery at pre-batch frontiers. The
renderer displays 40 rows, and timing is acceptance/render opportunity, not
physical paint or durability. Re-measure after
semantic coverage grows; never substitute the archived relay timings for UI latency.

### 2.3 Against the long-term goals

| Goal | Host-owned Loro | JSON ops + JavaScriptCore |
|---|---|---|
| One UI/CLI edit path | Yes | Yes |
| Real-time collaboration | Merge is built in; the relay stores and forwards bytes | Server ordering, client rebase and text OT must all be built |
| iOS (flaky network) | CRDT merge supports offline branches; delivery still needs a protocol | Offline shared editing requires an additional ordering/rebase design |
| Windows/Linux (Tauri) | Rust core runs natively in Tauri | No JavaScriptCore; would need QuickJS/V8 or a hidden WebView |
| Undo/history (deferred) | Largely available from Loro's op log | Must be built |
| Agent op contract | Same `$id` intents | Same `$id` intents |
| Server validation/permissions | Room-level access is easy; field-level checks decode with the same core in WASM | Easy |

"Host and Cloudflare just move bytes" holds, with three honest exceptions:

1. **Late-joiner snapshots and compaction.** The Worker runs the core (or
   `loro-crdt`) as WASM.
2. **The durable outbox and ack protocol.** This must be designed either way;
   spike 4 already did most of it.
3. **Field-level permissions.** These require decoding. Room-level access
   control does not.

## 3. Learning from the first version (`_docs/hitslop-sync`)

The first version already had a native Swift actor owning Loro via loro-swift
1.13.3, the webview holding confirmed JSON, and a Durable Object relay. See
`docs/native-loro-spike.md`, `native-loro-bridge-spike.md` and
`native-loro-incremental-spike.md` in that folder.

| First version | What went wrong | v2 |
|---|---|---|
| `change(draft => …)` mutated a JSON clone; Swift **diffed JSON** into Loro ops | The reconciler preserved keyed moves via `list.mov`, but inferred text edits and needed schema-aware before/after reconciliation | The SDK sends **typed intents**; the core does not reconstruct writes by diffing whole JSON documents. |
| Editable `stores/data.json` mirror, a byte journal across files, external-edit review sheets and recovery | The largest source of complexity | **No mirror.** Agents use the CLI. Storage stays SQLite with opaque Loro bytes. |
| Semantics written in Swift on loro-swift | loro-swift already wraps Rust through UniFFI. The Swift semantic interpreter is not shared with Tauri or the Worker; fine-grained access also crosses FFI repeatedly. | Semantics live in **our Rust crate**, behind coarse calls. We own the binding/toolchain; lower call overhead is measured, not assumed. |
| One relay entry per edit, one replay entry per ack | ~6 minutes to clear 1,000 hosted edits | Bounded batching of the outbox and replay |

The archived native checklist was a generated variant: its preparation script
added `await` to structural actions and replaced ordinary text bindings with the
ancestry-aware adapter. The native spike did not prove unchanged synchronous app
code safe across an asynchronous bridge.

Carried forward because they were proven:

- The text draft and edit ancestry, not host-frame replacement. Spike 3
  showed that replacing a focused field with every host frame turns `abcXYZ`
  into `abcXYZXYX`.
- The accepted-vs-durable confirmation policies.
- Immutable outbox batch IDs and hashes.
- Separate server-ack and client-applied receipts.
- An incoming cursor that advances only after a durable import.
- A relay that is an opaque ordered byte log.

## 4. Target architecture

### 4.1 Components

| Component | Owns |
|---|---|
| `hitslop-core` (Rust) | One `LoroDoc`, descriptor interpretation, `$id` resolution, validation, intents→Loro ops, atomic batches, projection and patches, preserve-and-flag issues, text splices at a base version, export/import |
| Swift `DocumentOwner` (actor) | `writer.lock`, one core handle, SQLite persistence (format unchanged: checkpoint + appended updates), save coalescing and `flush`, broadcasting patches to attached views, the socket server, and later the sync outbox and connection |
| WebView SDK (`@hitslop/document`) | Typed intent builders, the frozen snapshot store with structural sharing, text drafts and ancestry, local previews, and promises for acceptance and durability |
| CLI (`hitslop-native`, `@hitslop/cli`) | Forwards to the lock holder, or becomes the owner in-process |
| Durable Object (later) | Room auth, an ordered opaque byte log, idempotency keys, and compaction via the WASM core |

### 4.2 The core's API (JSON in, JSON/bytes out)

```rust
Document::create(schema_json, initial_json) -> Document
Document::open(schema_json, checkpoint: Bytes, updates: Vec<Bytes>) -> Document
doc.snapshot() -> Json            // { session, sequence, version, value, issues }
doc.apply(batch_json) -> Json     // { ok: { version, ids, patch } } | { error: { code, opIndex, message } }
doc.export_since(version) -> Bytes    // SQLite append + sync outbox
doc.checkpoint() -> Bytes
doc.import(bytes) -> Json         // { version, patch, issues }
doc.version() -> String           // opaque history token
doc.text(request_json) -> Json    // draft ancestry, publication and reconciled selection
doc.release_draft(draft_id)       // session-scoped cleanup
```

- Each call crosses FFI once. The placement spike showed that per-row crossings
  were the slow design; a container-ID index and confirmed move patches fixed
  it.
- The same crate compiles to a UniFFI xcframework (macOS/iOS), a native crate
  (Tauri), and wasm-bindgen (browser dev preview, Bun conformance tests,
  Workers).

### 4.3 Flow

```
 WebView (Svelte SDK)            Swift DocumentOwner (actor)            CLI / agent
 ─────────────────────           ─────────────────────────────          ───────────
 doc.fields.tasks.insert(v) ─┐   holds writer.lock + hitslop-core   ┌── slop apply --op …
 bindText → splice{base}  ───┼─▶ core.apply(batch) → patch ──┬───────┘   (socket if the app owns;
 doc.change(tx => …)      ───┘        │                      │           in-process if closed)
                                      │                      ▼
 snapshot store ◀── patch ────────────┼──── broadcast to every attached view
                                      ├──▶ SQLite append (export_since), coalesced; flush() = barrier
                                      └──▶ outbox → Durable Object relay (later; opaque bytes)
```

### 4.4 One edit path

- **The document is open:** the app holds `writer.lock` and the
  `DocumentOwner`. `hitslop-native` forwards `get`/`apply`/`batch`/`import`
  over the existing Unix socket.
- **The document is closed:** `hitslop-native` takes `writer.lock`, links the
  same Rust library, constructs a `DocumentOwner`, applies, saves and exits.
  **No WebKit.**
- Production discovery reuses `state/host.lock`; `state/writer.lock` alone owns
  the package. Never use the spike’s `host.json`, bypass a busy lock or unlink
  `writer.lock`.
- Export and the Finder icon still render the authored view in a WebView, fed
  the owner's snapshot. Rendering is the only thing WebViews do.

### 4.5 Intents (a single wire shape for the SDK, socket and CLI)

```ts
type Seg = string | { id: string };              // key, or list row by $id
type Path = Seg[];
type Anchor = { before: string } | { after: string }; // omitted anchor = end

type Intent =
  | { type: "set";       path: Path; value: unknown }
  | { type: "increment"; path: Path; by: number }
  | { type: "insert";    path: Path; value: object; at?: Anchor; id?: string }
  | { type: "remove";    path: Path; id: string }
  | { type: "move";      path: Path; id: string; at?: Anchor }
  | { type: "splice";    path: Path; base: string; index: number; delete: number; insert: string };

type Batch = { id: string; intents: Intent[]; message?: string; origin: "view" | "cli" };
```

- `decrement(n = 1)` is SDK sugar for `increment(-n)`; no separate wire intent.
- **A batch is atomic.** Validation observes earlier operations in the batch,
  including inserted and removed rows. Rejection leaves state, version and emitted
  publications unchanged. Loro transactions do not roll back applied operations;
  the current spike uses in-place edits with recovery at pre-batch frontiers and
  verifies continued use after rejection. Preserve that proof when porting.
- **Paths never use indices.** Intents based on an old snapshot still target
  the right row. Intents on removed rows reject with `path_not_found`; they
  are never retargeted.
- **The SDK allocates application `$id`s** and Rust validates them. In a
  transaction, allocation lets later intents reference the new row synchronously;
  it does not imply acceptance. CLI callers may supply an ID or ask the core to
  mint one. Ordinary SDK insertion returns its ID after acceptance.
- **Splice indices are UTF-16**, applied through Loro's `*_utf16` APIs.
- **Stable error codes:** `path_not_found`, `type_mismatch`, `out_of_range`,
  `duplicate_id`, `stale_base`, `too_large`, `unknown_intent`.

### 4.6 Host → view

- On mount: `{ type: "state", session, sequence, version, value, issues, status }`.
- After every accepted local batch, CLI batch or remote import:
  `{ type: "patch", session, previous, sequence, version, ops, issues, origin, batchId? }`.
  Operations use `$id` paths and come from Loro diff events. Swift sends them over
  the private bridge; request replies and pushed events share one SDK handler.
- The SDK applies patches to a deeply frozen, **structurally shared**
  snapshot. The Svelte adapter assigns it to `$state.raw`; stable keys preserve
  components and unchanged row identities avoid unnecessary child updates. Authors
  never apply patches or transport versions themselves.
- `status` events: `pending` / `saved` / `save-failed`, and later `synced`.
- Add an owner-session identity and monotonically increasing publication sequence,
  plus the preceding sequence on patches. A gap or new owner session requests a
  fresh state; old replies cannot overwrite a newer publication. CRDT versions
  describe history and are not transport sequence numbers.
- Requests need an identity and content hash before automatic retries are enabled.
  Cache identical accepted retries within the owner session; reject changed content
  under the same ID. Durable retry receipts and owner-restart outcomes belong to S3.
  Without a durable receipt, report an unknown outcome and require `get`, never
  silently resubmit an increment/insert.
- Patches must carry changes to `issues`, not just document values. Full-state
  recovery is explicit; it is not a persistent JSON mirror.

### 4.7 No optimistic updates

| Kind of edit | Behavior |
|---|---|
| Structural (checkbox, insert, move, remove, set) | Send the intent; the snapshot updates when the publication arrives. The event-driven Rust spike passes ordinary latency gates. Await acceptance before clearing input or displaying success; authors write no document rollback code. |
| Text (`bindText`) | The DOM owns the draft while edits are pending. Serialize submissions per binding and retain the draft's authored frontier/parent edit separately from the current merged publication. Recompute queued splices against acknowledged authored ancestry; never apply coordinates calculated after an unacknowledged insertion to an older confirmed string. Historical edits may use `fork_at` plus merge, subject to S2. Composition sends only committed text. Reconcile selection after pending edits settle. |
| Previews (slider drag, color scrub) | Local SDK overlay; explicit set or flush commits it. Incoming publications update confirmed state beneath the overlay; failed commits retain the preview for recovery. |
| Read-after-write | Ordinary edits resolve after acceptance and incorporation of the publication into this view. `const { id } = await tasks.insert(v)` returns the ID; other ordinary writes resolve void. Await Svelte `tick()` separately for DOM rendering. |

If measurement at large sizes ever demands it, the SDK can add a
pending-intent overlay rebased on each patch. That is safe because the host
stays authoritative. Do not build it now.

### 4.8 Storage

- Keep the SQLite format-2 envelope for the experiment. Engine placement alone
  does not require changing tables or the envelope format. Loro layout changes
  require a separate storage-revision decision; async SDK behavior requires an ABI
  decision. Neither automatically implies a SQLite format bump.
- The owner appends `export_since(lastSaved)` on each coalesced save. It
  writes a checkpoint on close and at a size threshold.
- Close/export establish a barrier, drain DOM drafts/previews, finish queued edits
  and save before capture/teardown. Swift coordinates but cannot skip the renderer
  drain. Read-only capture views cannot mutate the owner.
- A failed save retains the in-memory edits and lock and shows native retry. A
  cancelled close restores normal service; successful close destroys views.
- Attachments stay host-owned immutable blobs. Their SDK import waits for durable
  bytes, accepted reference insertion and saving; adapt the commit callback to the
  async write contract.

### 4.9 Sync (design only; built after launch)

- Commit local updates and their immutable outbox IDs/bytes/hashes in the same
  SQLite transaction. Only committed entries may be sent. Batch transport without
  changing an already assigned retry ID or its bytes.
- The Durable Object appends to an ordered log keyed by idempotency ID and
  broadcasts.
- A client imports remote updates → patch broadcast. Persist imported bytes and
  the incoming cursor atomically before acknowledging application. A server upload
  ACK does not advance that cursor. Missing dependencies and poisoned-log recovery
  need explicit S6 policies; the archived reject-and-pause behavior is not complete
  production recovery or preserve-and-flag semantic parity.
- For late joiners, the Durable Object serves the last compacted snapshot
  (produced by the WASM core) plus the log tail.
- Local → shared: upload a checkpoint as the room seed. Peer IDs are never
  persisted (unchanged).
- CLI and agent edits are ordinary local batches, so they sync like any
  other edit.

## 5. SDK and DSL decisions

The authoritative contract and examples are in
[LoroRustCutover §3](LoroRustCutover.md#3-author-facing-dsl-and-async-sdk).

- Keep pure-data `defineDocument` / `s`, `useDocument`, immutable `doc.current`,
  typed fields, `doc.at`, bindings, `<Slop>` and `ctx` services. No raw CRDT/bridge
  concepts in authored code.
- Ordinary writes return promises: insert resolves `{ id }`; other edits resolve
  void. `change<R>` collects synchronously and resolves `R` after batch acceptance.
  Its write-only transaction handles synchronously allocate row references. Reject
  async/nested collectors and escaped transaction handles; callback failure sends
  nothing. Reads remain the last published snapshot, not a speculative document.
- `await` means accepted and published into this view, not durable or DOM-rendered.
  `flush()` drains drafts/previews/queued writes and establishes local durability.
  Operation rejection and storage failure remain distinct.
- Integer counters use the tested contribution-map design with exact safe-integer
  increments/decrements and overflow rules. No reset API: subtracting an observed
  total is not a concurrent reset. Anomalous counters project as `null` plus an
  issue, preserving their original stored contributions internally.
- Supported vocabulary grows with core/SDK/binding fixtures. The approved archival
  allows unsupported descriptors to remain unavailable until their slops return.
  It does not permit silently accepting unimplemented semantics.
- JSON replacement of existing documents is explicitly unsupported in the contract-4
  trial. Immutable-template creation remains supported. A later replacement port
  must preserve destination-version checks and identities, with its own fixtures.
- **Compatibility decision is made:** runtime contract 4 / ABI 2, no contract-3
  migration, archival and incremental restoration. Sealed bytes stay immutable;
  active contracts change with implementation, not this plan alone.

## 6. Spikes

Each spike lives in `spikes/<name>/`, is disposable, and ends with a
`REPORT.md` covering the question, method, results, verdict, and what changes
in this plan. Evidence goes under `.hitslop/v1-evidence/<name>/`, following
the existing conventions: five fresh processes per cell, rotated candidate
order, frozen executable and asset hashes, and medians of per-process p95s.
Reuse the engine-placement harness (`spikes/engine-placement/run.ts`, `web/`,
`Sources/Harness`) and the document-authority harness
(`spikes/document-authority/Sources/Harness`) wherever possible.

S1 includes patch correctness and a minimal text-ancestry screen. S2–S5 guide
coverage expansion; spike success does not replace real app/SDK proof. The agreed
cutover distinguishes the limited signed trial from broad release: actual system
IME gates broad release, while memory/publication budgets stay open until measured
or explicitly adjusted with evidence. Follow the cutover's current gate table.
S6/S7 remain later work.

### S1: `hitslop-core` in Rust, native and WASM (go/no-go)

- **Question:** Can one Rust crate on `loro` 1.16.2 own hitSlop semantics
  behind a coarse API, run natively in the Mac app and as WASM in Bun, and
  beat or match the native-Loro candidate?
- **Build:**
  - Crate `spikes/hitslop-core/core` implementing the checklist subset
    (`text`, `boolean`, `object`, `list(object)`), with `set`, `insert`,
    `remove`, `move`, `splice`; `$id` minting; atomic batch; patches from
    Loro diff events; `export_since`, `checkpoint`, `import`.
  - A UniFFI xcframework (arm64 macOS) consumed from a SwiftPM target.
  - A wasm-bindgen package consumed from Bun.
  - A conformance fixture table `fixtures/*.json`
    (`{schema, before, batch, after | error, patch}`), run by **both** Bun
    (WASM) and Swift (native). Expected values/errors come from independent literal
    scenarios and applicable current semantic fixtures. Agreement between two
    bindings to the same buggy core is insufficient proof.
  - Patch application must reproduce both a fresh snapshot and its issues after
    local edits and imports. Include stable row identity, sequential batch paths,
    Unicode, rejection, malformed imports and checkpoint/update reopening.
  - A minimal inherited `abc` → `abcXYZ` pending-input/remote-edit screen with
    explicit ancestry; full system-IME/selection coverage remains S2.
  - A headless native edit/reopen without WebKit and a renderer destruction/remount
    check. Process startup, writer routing and crash durability remain S3.
  - A new `rust-core` candidate in the engine-placement matrix, using the
    same Svelte view and workloads.
- **Measure:**
  - checkbox, move, splice and paste acceptance
  - render opportunity
  - sustained typing drain
  - open time
  - memory at 100/1k/5k/40k rows × 1/5/10 windows
  - FFI call cost
  - xcframework size and app-size delta
  - WASM size
- **Pass:**
  - In ordinary 1k/5k × 1/5/10-window cells, every-run checkbox/move acceptance
    p95 ≤ 50 ms; render-opportunity median p95 ≤ matched WASM + 16.7 ms;
    every-run sustained typing drain < 50 ms.
  - Relative to the freshly measured Swift-native control, median acceptance p95
    regression ≤ max(2 ms, 20% of control), memory regression ≤ 10%. Exceeding a
    comparative budget triggers profiling and an explicit decision, not weakened
    correctness. Report 40k stress tails separately; they do not veto the ordinary
    product envelope or get hidden in pooled medians.
  - Every fixture passes identically in Bun and Swift.
  - The xcframework builds from a script under the pinned Xcode/XcodeGen and
    signs with the hardened runtime and the current entitlements.
  - **Atomicity:** reject a later operation after earlier operations have executed,
    proving unchanged live state/version and no publication plus continued editing,
    drafting, byte export and reopening. In a disposable source copy, bypass the
    isolation/recovery mechanism and verify the owner test detects partial mutation.
    Never run a sensitivity script against production source.
- **Kill or adjust:**
  - If the UniFFI/xcframework toolchain can't be made reproducible in CI,
    evaluate `swift-bridge` or a C ABI with cbindgen before abandoning the
    Rust core.
  - If performance loses to native Loro, profile the patch derivation first.

### S2: Text ancestry and IME on `splice{base}`

- **Question:** Does the host-owned text binding stay correct under
  latency, remote edits and IME, using base-version splices?
- **Method:** Port the spike-3 matrix from
  `_docs/hitslop-sync/docs/native-loro-bridge-spike.md`:
  - three inputs before acknowledgement at 0/20/100/500 ms injected delay
  - remote edits around acceptance and during composition
  - Unicode (emoji, combining marks, CJK)
  - paste, selection replacement, backspace
  - native AppKit keyboard events through `WKWebView`, including the focused
    responder's undo/redo
  - CLI `splice`/`set` on the focused field
  - CLI `remove` of the row whose text field is focused
  - renderer reload with pending drafts
- **Pass:**
  - The final text is exact in every trace (e.g. `abc` → `abcXYZ`, never
    `abcXYZXYX`).
  - The caret never jumps.
  - Nothing is sent mid-composition.
  - A focused-row removal blurs cleanly without resurrecting the row.
  - A failed save keeps the DOM draft.
  - A system IME (Japanese/Chinese) manual gate is recorded with the enabled
    input sources before broad release; the approved limited trial may precede it.
- **Output:** the `bindText` contract, plus SDK-boundary Bun tests with a
  fake host for the non-IME cases.

### S3: Single edit path, locks and CLI cold start

- **Question:** Is "forward vs. own" race-free, and is headless CLI editing
  fast without WebKit?
- **Method:** A scripted harness that interleaves:
  - CLI `batch` while the app is opening the document
  - CLI `batch` during the close/save barrier
  - two CLIs at once on a closed document
  - the app opening while the helper owns it
  - the helper killed mid-batch and mid-save (`SIGKILL`)

  Time helper launch → lock → open → apply → save → exit at 100/1k/5k rows,
  and compare with today's headless WebKit path.
- **Pass:**
  - Within an owner session, identical request retries return the recorded result
    without applying again. Across owner restart, a durable receipt proves the
    outcome or the helper reports an unknown outcome requiring `get`; no blind
    replay. Do not claim exactly-once retry across restart without durable receipts.
    No lock is bypassed and no loss is silently reported as success.
  - With the window open, a CLI edit reaches the view in one patch.
  - Reopening after a kill yields the old or the new state, never a torn one.
  - Helper cold-start apply ≤ 150 ms at 1k rows.
- **Output:** the socket method set, retry codes, and the app's behavior when
  the helper holds the lock (proposed: short wait, then "being edited" with
  retry; never steal the lock).

### S4: Counters, merges and preserve-and-flag in the core

- **Question:** Does the proposed per-writer counter converge where Loro
  Counter doesn't, and do merged anomalies surface as issues rather than
  corruption?
- **Method:**
  - First specify numeric domain (exact bounded integers, decimal or float),
    overflow, deterministic summation, set/reset versus increment, contribution
    ownership and growth after many reopen sessions. Do not claim exact arbitrary
    floating-point sums without a representation that provides that property.
  - Two and three replicas with concurrent increments, including large
    values near the safe-integer boundary and cancellations.
  - Export/import regrouping (reproduce `spikes/engine-placement/counter-probe.ts`
    against both designs).
  - Restoring an old checkpoint on one replica, then merging.
  - Finder-copy duplication (two packages with a shared history) editing and
    merging.
  - Concurrent `$id` duplication and move+remove; valid concurrent enum sets stay
    in the enum, while an explicitly malformed imported enum value is preserved
    and flagged. Two valid last-writer-wins enum writes alone cannot create a new
    invalid enum member.
- **Pass:**
  - Counter replicas converge to the independently specified arithmetic result;
    exactness and overflow behavior match the chosen numeric domain.
  - The counter-probe failure reproduces against Loro Counter but not against
    the new design.
  - Anomalies are preserved and reported in `issues`, never repaired on read.
  - Local writes stay strict.

### S5: Patch publication and snapshot structural sharing

The correctness subset is part of S1; this phase expands randomized coverage and
proves rendering cost. Correctness blocks integration; the complete 2 ms budget
remains an explicit measurement gate under the cutover's trial/release policy.

- **Question:** Are `$id`-keyed patches derived from Loro diff events correct
  and cheap, and does the SDK's structural sharing keep Svelte re-renders
  proportional to the change?
- **Method:**
  - Property test: random intent batches (and random remote imports) →
    apply the core's patches to the previous snapshot → compare with a fresh
    `snapshot()`.
  - Count re-rendered rows per edit in the Svelte view at 1k/5k rows.
- **Pass:**
  - Zero divergence across ≥100k random steps.
  - A single-row edit re-renders O(1) rows.
  - Patch build + transfer + SDK apply p95 ≤ 2 ms at 5k rows.

### S6: Sync protocol on the core (design check before building collaboration)

- **Question:** Does spike 4's outbox/relay protocol carry over to the Rust
  core with batching, and does the Worker compaction path work?
- **Method:**
  - Port `_docs/hitslop-sync/Prototypes/native-loro-relay` and the
    `SpikeDocument`/`SpikeBatch`/`SpikeConnection` logic onto
    `DocumentOwner` + `hitslop-core`.
  - Add bounded upload and replay batching.
  - Run the core's WASM build in the Durable Object to produce compacted
    snapshots.
  - Two Mac clients plus one CLI agent in one room.
  - Rerun spike 4's failure table: force quit offline, crash before send,
    dropped ack, crash after ack, relay restart, invalid peer bytes,
    quota/revocation.
- **Measure:** backlog clear time for 1,000 edits (spike 4: ~6 min hosted),
  late-join bootstrap time and bytes, Worker CPU per batch, WASM bundle size
  against Worker limits.
- **Pass:**
  - Every failure case recovers as spike 4 required.
  - The 1,000-edit hosted backlog clears in ≤ 10 s.
  - Late join at a ~1.4 MB document completes in ≤ 3 s.
  - Compaction runs within Worker CPU and memory limits.

### S7: iOS and Tauri smoke (portability check; not blocking launch)

- **Question:** Does the same crate and API really drop into iOS and Tauri?
- **Method:**
  - Build the xcframework for iOS arm64 and simulator. Open a Quick
    Checklist document in a minimal SwiftUI + `WKWebView` host with the same
    SDK.
  - Build a minimal Tauri app that links the crate natively, with the same
    SDK over Tauri IPC, on Windows (WebView2) and Linux (WebKitGTK).
  - Run the S1 fixture table on each platform.
- **Pass:**
  - Fixtures are identical on every platform.
  - Checklist edit, save and reopen work.
  - The platform-specific code is limited to lock, path, lifecycle and IPC
    adapters.

## 7. Agreed production cutover

[LoroRustCutover.md](LoroRustCutover.md) owns the complete milestone sequence:

1. M0: async contracts, reset policy and archival preparation.
2. M1: native/WASM builds, bindings and signed app/helper packaging.
3. M2: SDK and Swift-owner integration, publication delivery, drafts and barriers.
4. M3: Quick Checklist end-to-end slice and production measurements.
5. M4: new compatibility consumers, coverage audit and trial packaging.
6. M5: limited signed trial, then additional types/slops and broad-release gates.

The reset/archival decision is approved; do not repeatedly request it. Keep the
existing native client and update active contracts alongside implementation.
Retire old compatibility promises explicitly while preserving their sealed
artifacts. Remove obsolete implementations only after replacement paths are wired
and their still-promised behavior has an independent test owner.

## 8. Risks

| Risk | Mitigation |
|---|---|
| Rust/UniFFI toolchain adds build and CI complexity | S1 requires a reproducible scripted build; `swift-bridge`/cbindgen fallback; a prebuilt xcframework cached in CI |
| Rust panics crash or invalidate the owner | Use unwind-capable native builds and boundary containment; never keep using a potentially mutated owner after a panic. Retain the lock and available drafts; explicit recovery distinguishes durable state from potentially lost unsaved edits. Do not silently reload and call it a save retry. Catching panics does not catch OOM/abort; document platform-specific limits and fuzz bounded imports. |
| Decoding untrusted bytes in the host's crash boundary | Allocation limits, size caps before import, and fuzzing `open`/`import` in S1 and S6 |
| Full history growth | Capacity checks stay; history pruning remains deferred to the sync design |
| Peer-ID reuse | Peer IDs are never persisted (unchanged); fresh per session |
| Loro version pins | Pin in `Cargo.lock`; record the storage-revision decision per pin (unchanged rule) |
| Two languages (Rust core, TS SDK) | Clean split: semantics in Rust only; the TS SDK is presentation (intent builders, snapshot store, bindings) with no document rules |

## 9. Open questions

- Should undo (deferred) use Loro's `UndoManager` in the core, scoped per
  origin so a CLI edit isn't undone by Cmd-Z in the window? Decide before
  freezing the batch `origin` field.
- Text publications currently set the changed field's full string; draft replies
  also carry reconciled selection. A future text-delta optimization must preserve
  the same SDK semantics and be justified by long-field measurements.
- Does the Durable Object validate schema on append, or is that deferred
  until abuse appears? Proposed: room-level auth only at first; the core can
  validate later without a protocol change.

Signing and notarization remain in GitHub Actions. Per the maintainer, do not make
a local app/release build yet. `bun run apple:build` is unsigned and is not evidence
of notarization. Follow the cutover’s early real-app checkbox/save/reopen slice.
