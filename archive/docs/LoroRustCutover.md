# Host-owned Rust core: async SDK and Quick Checklist cutover

Status: agreed implementation plan, 2026-09-29. **The breaking change and slop
archival are approved.** Target runtime contract 4 / ABI 2, with no contract-3
migration. Preserve sealed historical bytes and release records. This document
owns the cutover decisions; [LoroHostPlan.md](LoroHostPlan.md) records the
architecture rationale and proving spikes. It supersedes the JavaScriptCore
direction in [NextPhasePlan.md](NextPhasePlan.md).

The implementation has not landed merely because the plan is agreed. Update
`AGENTS.md`, engineering/versioning/testing contracts and the test ledger alongside
each milestone's actual behavior, rather than postponing them to the end.

## Implementation checkpoint

Contract 4 / ABI 2 is selected in development. The native owner is wired into the
existing WebView host and native CLI; native page boot no longer initializes Loro.
The public SDK and Svelte adapter use asynchronous acceptance, immutable patches,
transaction collectors, DOM text drafts, boolean bindings/previews and save barriers.
Quick Checklist and the starter await acceptance. Browser development uses the same
Rust core through WASM. JSON replacement/import is explicitly refused in the trial.

The production WebView/public SDK and CLI cutover is implemented. Automated checks
cover native ownership/reopen, failed-save retention, real WebKit SDK and live/closed
CLI integration, plain-JS and sealed Svelte ABI-2 consumers, synthetic composition,
attachments, themes, PNG/PDF, renderer recovery and SQLite crash boundaries.
Stable derived row identities retain addressability after duplicate-ID merges.
The universal macOS XCFramework, WASM binding, helper and test bundles build locally.
No app/release bundle, signing, notarization, publication or commit was made.

Local evidence: `bun run check`; 168 Bun tests; 60 compatibility replay cases and
one bundled-template open/reopen; 25 Rust core tests plus the FFI panic-containment
test; 143 Swift tests; eight native CLI tests; ten SQLite crash-matrix cases;
relocated-helper checks; plain-JS/Svelte/Checklist PNG/PDF smoke; and packed npm
consumer installation/type/dev checks. Closed 1k helper edits measured **121.98 ms
p95** across 20 samples in the debug helper, within 150 ms. Ten concurrent helper
pairs preserved every counter increment. These are local checks, not a claim that
GitHub Actions or the installed signed app has run.

Remaining gates: production performance measurements (including matched host/
WebContent memory and isolated complete publication cost), actual Japanese/Chinese
system IME, GitHub Actions signing/notarization, and the installed limited trial.
The production window diagnostic exposed repeated row lookup and textarea layout
work; immutable row indexes, batched sizing and lazy menu portals reduce that work.
**The performance gate still fails:** one 1k-window diagnostic measured 441 ms
acceptance p95 and 183 ms drain; 5k rows repeatedly exceeded the startup deadline.
A WebContent sample during opening was dominated by DOM insertion. This needs
further production UI/SDK profiling; it is not a native-Rust language comparison.
See the [local evidence record](evidence/loro-cutover-2026-09-29.json). Do not substitute
the spike's faster numbers or claim the 1k/5k performance envelope is ready.
The installed trial and release checks remain deliberately deferred; functional
implementation and automated integration testing do not require a release.

## 1. Decision and current evidence

One host-owned `hitslop-core` (Rust on Loro) owns document semantics. Swift owns
native lifecycle, locking, persistence and transport. Svelte sends typed edits and
renders an immutable snapshot maintained by the SDK. CLI edits reach the same owner.
The Apple WebView contains no Loro replica. The same Rust crate builds as WASM for
browser development and SDK tests; this is a separate deployment target.

Quick Checklist is the first port and limited trial app. Archive the other slops
and restore them as their required capabilities land. The new SDK need not emulate
the synchronous ABI or retain its complete descriptor vocabulary at launch.

Evidence: [native-owner report](../spikes/hitslop-core/NATIVE-OWNER.md) and its
frozen run records. These compare implementations, not Rust versus Swift language
performance. Both native candidates use Rust Loro through UniFFI.

| Gate | Spike evidence | Remaining cutover/release proof |
|---|---|---|
| Edit latency | Five trials per ordinary cell: 5k checkbox/move median acceptance p95 3/3 ms versus Swift control 8–10/9–10 ms. All three exploratory 40k runs succeed | Repeat with production SDK and host; retain ordinary latency/render/drain gates |
| Publication correctness | 100k randomized steps, chaos imports, nested lists, post-rejection lifecycle | Production snapshot/issue/identity contract and new SDK tests |
| CLI ownership | Stand-in owner passes 9 race/kill scenarios; closed 1k apply 20 ms p95 | Real app/helper routing, publication delivery, retry identity and lifecycle tests |
| Integer counters | Exact convergence, replay and overflow tests | Declare the new numeric domain, projection and storage-reader policy |
| Memory | Total misses +10% budget by 0.4 percentage points at 1k × 1 window; attributed to WebContent | Still open: production measurement or an explicit evidence-backed budget decision |
| Complete publication cost | Core construction 0.002 ms at 5k; transport/application estimate is not an isolated measurement | Still open: measure build + response transfer + SDK apply against 2 ms p95 |
| Signing and packaging | Spike native/WASM bindings build | Production app/helper signing, notarization and matching runtime packaging |
| System Japanese/Chinese IME | Synthetic composition and native keyboard checks pass; system IME unverified | Required before broad release; not a blocker for the limited trial |

The old Swift-host spike supplies useful draft ancestry and queueing evidence.
Its schema-aware reconciler did preserve list moves with `list.mov`; the reason to
prefer typed intents is explicit edit intent and less reconciliation work. The old
native checklist variant also added `await` and special text bindings: unchanged
synchronous authored code was not the native experiment.

## 2. Ownership and publication flow

```text
Svelte handle / CLI
    → typed intent batch
    → Swift DocumentOwner serial queue
    → Rust validation + atomic Loro edit
    → publication {session, previous, sequence, version, ops, issues}
    → private WebView bridge
    → SDK immutable snapshot
    → Svelte reactive update

Rust checkpoint/updates → Swift → SQLite
```

| Component | Owns |
|---|---|
| Rust core | Descriptor interpretation, identity, validation, atomic batches, Loro events → publications, preserve-and-flag issues, text ancestry, counters, byte export/import |
| Swift `DocumentOwner` | One core handle and OS writer lock per owned package; SQLite format 2; save coalescing, barriers, recovery UI, socket discovery and publication broadcast |
| Page runtime / SDK | Intent builders, ordered submissions, snapshot store, draft/selection state, preview overlays, acceptance and durability promises |
| Svelte adapter | Reactive `doc.current`/status/issues backed by the runtime; keyed authored components render the snapshot |
| Existing native client | Catalog/Recents, windows/toolbar, theme, attachments, PNG/PDF, Finder icons, Sparkle and Firebase |

### Delivery and applying patches

- Authors send `set`, `insert`, `move`, `remove`, `increment` and text edits through
  typed handles. They never construct Loro operations or manage transport versions.
- Swift delivers publications to attached views through the runtime's private
  bridge. A request reply can carry its publication; CLI and later peer edits push
  events. **Replies and events enter the same SDK handler.**
- Each publication has an owner session, preceding/current sequence, CRDT version,
  operations and issues. Ignore duplicates. A sequence gap requests fresh state;
  an owner change invalidates outstanding assumptions. Delayed replies or resyncs
  cannot replace a newer snapshot. CRDT versions are not delivery sequence numbers.
- Field patches copy the changed object and its ancestors; row patches use `$id`.
  Freeze new snapshot objects and retain unchanged object identities. The Svelte
  adapter assigns the next snapshot to `$state.raw`. The SDK changes data; Svelte
  changes the DOM. Isolate observer exceptions from acceptance, saving and others.
- Full snapshots are for mount/remount and resynchronization. An anomalous subtree
  may require an exact replacement patch. Ordinary edits do not resend the whole
  document. Rendering snapshots stay in memory, never in a persistent JSON mirror.
- Request IDs, content hashes and owner sessions stay internal. Cache identical
  accepted retries within a session and reject ID reuse with different content.
  After an unknown outcome or owner restart, reconcile rather than blindly resend;
  do not promise exactly-once replay without a durable receipt.

TypeBox in `@hitslop/schema` owns batch, text, state, publication, status and socket
contracts and generates Swift/Rust models. Use `bun run schema:generate`.

### CLI routing

Try `writer.lock` first. If acquired, open → apply → durable flush → release in the
native helper, without WebKit or authored code. Otherwise use discovery to forward
to the live owner. Publish discovery only after listening. Retry unapplied
starting/closing states with bounded backoff up to 2 s. An uncertain sent request
returns `unknown_outcome`, never an automatic replay. A live `get` participates in
the document barrier before returning its saved snapshot. Never steal or unlink
the writer lock. Spike routing is the starting point, not the production proof.

## 3. Author-facing DSL and async SDK

Keep `defineDocument` / `s` as pure-data descriptors, `useDocument(schema)`,
`doc.current`, `doc.fields`, `doc.at`, `bindText`, `bindValue` and `<Slop>` with its
capture snippets. Apps access the runtime only through `ctx`; they do not embed
or import the engine. Revision tokens and bridge messages are private.

| API | Result and timing |
|---|---|
| `handle.set/move/remove/increment/decrement(...)` | `Promise<void>`; resolves after host acceptance and incorporation of its publication into this view |
| `list.insert(value, at?)` | `Promise<{ id: string }>` with the same acceptance boundary |
| `doc.change<R>(tx => R, options?)` | `Promise<R>`; one atomic batch, resolving with the callback result after acceptance |
| `doc.flush()` | `Promise<void>`; drains drafts, previews and queued writes and waits for durable saving |
| `doc.current` | Immutable, reactive snapshot; no same-tick read-your-write guarantee |
| `doc.at(snapshotRow)` | Typed handle resolved from snapshot identity |
| `list.item(id)` | Typed by-ID handle, also available on transaction lists |
| `decrement(n = 1)` | SDK sugar for `increment(-n)`; no separate wire intent |

An accepted write is visible in the SDK snapshot before its promise resolves.
Svelte DOM rendering may still require `await tick()`. Acceptance does not mean
saved, and a later legitimate edit may already supersede the accepted value.

### Transactions and IDs

- `change` invokes its collector once, immediately. Reads use the last published
  snapshot; `tx.fields` and `tx.at` are synchronous, write-only intent builders.
- In a collector, `insert` returns `{ id }` synchronously so later operations can
  reference the new row. Rust executes the collected operations sequentially in
  one atomic batch, observing earlier operations in that batch.
- IDs use the existing random Crockford application-ID algorithm. The SDK allocates
  IDs for its intents; Rust validates them. CLI callers may supply an ID or ask
  the core to mint one. Allocating an ID does not imply acceptance.
- Reject async callbacks, nested transactions and use of escaped transaction
  handles. A thrown callback or returned thenable sends nothing. Ordinary document
  writes inside an active collector reject; use its `tx` handles instead.
- Outside collectors, structural writes are ordered per view. Validation failures
  do not stall later independent work. No automatic retry of a rejected edit.

```ts
const { id } = await doc.fields.tasks.insert({
  text,
  done: false,
  archived: false,
});

await doc.change(tx => {
  tx.fields.tasks.item(id).done.set(true);
  tx.fields.tasks.item(id).archived.set(true);
});

const nextId = await doc.change(tx => {
  const { id } = tx.fields.tasks.insert({ text: "Next", done: false, archived: false });
  tx.fields.tasks.item(id).done.set(true);
  return id;
});
await doc.flush();
```

### Bindings, previews and failures

`<textarea use:bindText={doc.fields.title} />` keeps local DOM text immediately
responsive. The SDK retains draft ancestry and selection, serializes submissions
per binding, sends nothing mid-composition and reconciles accepted text/cursors.
A removed target never resurrects. Failed submissions retain the draft; an owner
change requires reconciliation. Normal input authors never supply splice bases.

`bindValue` submits the control's requested scalar value and handles acceptance or
rejection. Scalar previews are synchronous local overlays, with no history write
until an explicit set or flush. Keep the confirmed snapshot separately from the
presented overlay; incoming patches update confirmed state beneath it. A failed
preview commit remains visible for recovery, not silently marked accepted.

- `status` is `pending / saved / save-failed`: queued work, drafts/previews or
  unsaved accepted edits keep it pending; persistence failures set save-failed.
- Rejections use typed errors and centralized reporting. A command rejection does
  not masquerade as a save failure. Awaiting callers still receive rejection.
- Remove the authored `full` boolean. Capacity failure stays an explicit native
  recovery state, blocks successful close/export, and never discards live work.
- Port Checklist actions deliberately: keep composer input until insert succeeds;
  show remove/file/restore success notices only after acceptance. Do not clear
  newer composer text when an earlier insertion resolves.

## 4. Durability, capture and data semantics

### Barriers and recovery

Close/export must establish a barrier, drain renderer drafts and previews, finish
queued edits, persist, and only then capture or close. Swift coordinates; it cannot
save characters that still exist only in the DOM. Block new independent edits
while permitting the drain's requests. Pending composition must finish or leave
the barrier pending/failed; never silently discard it.

On successful close, destroy views, remove discovery and release the lock. On
failure retain live work, ownership and native retry. Restore normal service if
close is cancelled; do not leave a failed close with a permanently dead socket.
Capture uses the saved barrier snapshot, and read-only export/icon views cannot
mutate the owner. Resume normal editing after capture takes its snapshot.

A Rust panic invalidates the owner: block further mutation, retain the lock and
available DOM drafts, and present explicit recovery. Reloading durable state does
not recover accepted-but-unsaved work and must not be labelled a save retry.
Explicit discard/reload communicates that distinction; do not silently replace
unsaved state. Catching unwind does not contain OOM/abort.

Attachment import resolves only after the immutable blob is durable, its reference
edit is accepted, and that edit is saved. Adapt the existing commit callback to
await acceptance rather than rejecting promises; a rejected reference edit may
leave an unreferenced immutable blob, never a dangling saved reference.

### Supported data

- First trial vocabulary: `text`, `boolean`, `object`, `list(object)` and the tested
  integer counter. Other kinds reject explicitly until core, SDK and binding
  fixtures cover them. Restore strings/bounded numbers/enums/optionals for Small
  Expenses; records, scalar lists, trees and rich text follow their first consumer.
- Counters use exact safe-integer increments/decrements and the tested overflow
  policy. There is no reset API in this milestone: decrementing a stale observed
  total is not a concurrent reset. Counter snapshots expose exact safe integers or `null` plus an issue; anomalous
  stored maps remain preserved internally and are never exposed as numeric values.
- Preserve stored anomalies. Stable derived identities and the complete issue
  contract are production coverage requirements, not implied by the checklist
  subset. Reads must not repair data or mint stored identities.
- JSON replacement of existing documents is explicitly unsupported in contract 4
  during the trial; reject before mutation. Creation from immutable `initial.json`
  remains supported. A later replacement port must preserve destination-version
  checks, matching identities and atomic translation, with dedicated fixtures.
- Keep SQLite format 2. Record the new Loro/counter layout's minimum-reader storage
  revision separately from runtime contract 4 and ABI 2; refuse old packages before
  opening storage. A format bump is not implied by moving the engine.

## 5. Packages and contracts

No new npm packages. Keep the existing app, feature graph, host integration and
native CLI; rename the engine module when its replacement is wired.

```text
crates/hitslop-core/          # Rust semantics and tests
crates/hitslop-core-ffi/      # UniFFI / native bindings
crates/hitslop-core-wasm/     # wasm-bindgen / browser and test binding
packages/schema/             # authoritative TypeBox contracts and generation
packages/document/           # ctx ABI, SDK, Svelte adapter and page runtime
packages/cli/                # authoring, native forwarding, runtimes/4/ for dev
HitSlopDocument              # native owner; replaces former HitSlopWasm module
examples/slops/quick-checklist/ # first port and limited trial
archive/slops/               # other slops, restored as capabilities land
```

Carry pinned Rust 1.96.1, Loro 1.16.2, UniFFI 0.31.1 and wasm-bindgen 0.2.127
from the spike with lockfiles and reproducible build inputs. App and helper link
matching native cores. Browser dev hosts the WASM core through the same SDK
semantics. Authored app bundles contain neither engine.

As milestones land, update ownership, async timing, descriptor coverage, identity,
reader gates and reset history in the active engineering/versioning contracts.
Clarify that an ephemeral rendering snapshot is allowed while persistent JSON
mirrors and JSON-diff write reconciliation remain prohibited. Retire contract-3
support through explicit refusal tests; leave its sealed bytes/records untouched.

## 6. Milestones

Keep changes isolated from unrelated working-tree edits and keep the appropriate
checks green. This agreed scope does not need a new approval for every routine
implementation step. Do not commit, publish or release merely to complete a gate.

### M0: contract and reset preparation

Finalize the SDK/wire contracts above and record the approved reset. Archive other
slops; keep Quick Checklist as the trial target. Update discovery/bundled inventory
and archive provenance. Separate retired compatibility promises from coverage
moving to a new owner; do not claim every removed test has an equivalent replacement.
**Exit:** contracts/types express the async API, old support is explicitly retired,
and checks discover the intended active set without rewriting sealed history.

### M1: core builds and bindings

Promote the Rust core and native/WASM adapters into `crates/`; wire generated
contracts and repeatable XCFramework/WASM builds into repository scripts. Add the
native binding to app/helper and exercise literal fixtures through both bindings.
Validate matching runtime packaging locally; signing and notarization run through
GitHub Actions before the trial. Do not make a local app/release build yet;
ordinary build commands remain distinct from release/notarization commands.
CI installs pinned Rust, the WASM target and matching wasm-bindgen CLI. Ubuntu
builds WASM before SDK tests through a platform-neutral command; macOS builds both
bindings. Include crates/toolchain/lockfiles in native path filters and cache keys,
and prepare Rust in release jobs too. `apple:build` is unsigned; use the release
GitHub Actions release path for Developer ID/notarization evidence. Local app/release
packaging is deferred by the maintainer; no public release is implied.
**Exit:** native/WASM fixtures and byte replay pass, CI runs Rust tests, and the
app/helper packaging path is verified. Toolchain fallbacks are evidence-driven.

### M2a: first real-app slice

Create/open Quick Checklist in the existing app, render the native owner snapshot,
accept one async checkbox edit, save, close and reopen it. Include failed-save
ownership/retry evidence. Implement the minimal owner on production SQLite/writer
locking, initial state/publication bridge, `useDocument` and `doc.current`.
Contract-4 documents use only the native owner, never an old-engine fallback. Old
paths may remain in source only until their replacements are wired.
**Exit:** real-app checkbox acceptance updates the snapshot before promise resolution;
save/reopen retains it and failed saving retains ownership.

### M2b: SDK and owner integration

After M2a, implement lists/collectors, then text drafts/previews, real helper routing,
barriers/capture/attachments, old-path removal, and finally the complete authored
Checklist/dev/starter/skill port. Build the async handles, collector, snapshot store, bindings, previews and new ABI
alongside the Swift owner, socket routing, publication broadcast and save barriers.
Port onto production lock/storage/socket infrastructure: `state/host.lock` is
discovery, `state/writer.lock` is ownership; never use the spike’s `host.json`. Implement read-only
capture and attachment-reference durability. Remove obsolete engine, headless
WebKit and storage-bridge paths only after replacements are wired and tested.
**Exit:** SDK contract tests and real app/helper race, failure, kill and lifecycle
tests pass; closed 1k apply is ≤150 ms p95 and starts no WebKit/authored code.

### M3: Quick Checklist vertical slice

Port actions to async acceptance; wire `slop dev`, CLI commands, starter and embedded
skill. Exercise window edits, open/closed CLI edits, immediate close/export after
input, save retry, reopen, PNG/PDF, Finder icons, themes and attachments. Re-measure
ordinary workloads and split host/WebContent memory. Measure the complete publication
budget instead of inferring it from core construction time.
**Exit:** the production vertical slice passes, including synthetic composition;
remaining trial limitations and gate results are recorded.

### M4: contract fixtures and trial packaging

Add plain-JS and Svelte ABI-2 consumers and contract-4 compatibility fixtures. Audit
coverage retirement/relocation, verify immutable release history, run the native
and release checks, and produce the signed trial package. Contracts have been
updated with their owning milestones, not first rewritten here.
**Exit:** automated trial gates pass; memory/publication budgets remain explicitly
open or have an evidence-backed decision. System IME remains a broad-release gate.

### M5: limited signed trial, then broader coverage

Use the installed Quick Checklist build daily for roughly a week. Exercise rapid
add/check/reorder/file/rename, CLI edits during open/close, force-quit and helper
kills, disk-full/read-only save failure, Finder copies, update/reopen, and 1k/5k task
sets. Check saved state against icons/exports. Inspect Crashlytics for native/FFI
failures and Analytics for save failures.
**Exit:** no observed data-loss/crash issue over the trial; this does not substitute
for automated evidence. Then restore Small Expenses and further slops as their
vocabulary lands. Complete actual Japanese/Chinese IME testing, remaining semantic
coverage for advertised types, and unresolved release gates before broad release.

## 7. Verification and coverage ownership

Before adding regression coverage, name the observable failure, independent oracle
and gap. Retain independent literal expectations alongside snapshot/property tests.

| Owner | Required cases |
|---|---|
| Rust semantics | Atomic rejection and continued use; insert-then-edit batches; nested/chaos imports; values, stable identities and issues; exact counter replay/overflow; text ancestry; explicit JSON-replacement refusal |
| Bun SDK / WASM | Async results and snapshot-before-resolution; ordered writes; callback throw/thenable/nesting/escaped handles; duplicate/stale/gapped publications and delayed resync; structural sharing; centralized errors without observer interference |
| Bindings and authored view | 0/20/100/500 ms text delays; Unicode/selection/composition; deleted focused row; retained failed drafts/composer input; acceptance-dependent notices; unaffected-row render counts |
| Swift integration | Writer ownership/discovery; same-session duplicate/conflicting requests; unknown outcomes; live `get` barrier; app/CLI open/close races; process kill; save/capacity failure; panic recovery; renderer teardown; read-only capture; attachment-reference durability |
| New compatibility consumers | Plain-JS and Svelte app/ctx timing and errors; contract-4 reopen/replay; old-contract refusal; storage-reader gate; matching app/helper; frozen historical artifact integrity |

Run `bun run check && bun run test`, Rust tests, native build/Swift/native suites,
and `bun run release:check` at their relevant milestones. For behavior still
promised after a refactor, use break-then-verify before removing old coverage and
record its new owner in the five-column ledger. For intentionally retired
contract-3 or archived-template promises, record retirement rather than asserting
new subset tests prove the old contract. Do not edit sealed artifacts.

## 8. Deferred and open gates

Collaboration/outbox/relay, iOS/Tauri, undo UI, history pruning, counter writer-key
compaction and schema evolution remain later work. Do not disguise descriptor
omissions as supported capabilities. Large-document cold open is measured separately
from the ordinary 1k/5k trial envelope.

Memory and full publication-cost gates stay open until measured or explicitly
adjusted with evidence. Actual Japanese/Chinese IME is mandatory before broad
release. The limited trial does not authorize publishing npm packages or a public
release; the established compatible-app-first release process still applies.
