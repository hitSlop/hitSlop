# Engine placement: Mac experiment and decision

This experiment evaluates a native owner, not a production engine replacement.
The current Mac application, sealed runtimes, templates and document SDK are
unchanged. iOS and Tauri have an architectural path, not new targets.

## Decision

**Native ownership is a viable performance direction in this experiment.** At
5,000 rows in one window, matched-WASM/native checkbox acceptance was 16/5 ms and
move acceptance was 35/6 ms. Ten such windows used about 889/424 MiB of reported
host-plus-content footprint. The additional transport hop added 0–1 ms to the
median round-trip p95 in the echo comparison. The bridge did not erase the native
candidate's local speed advantage.

Opening was mixed: native opened 1,000- and 5,000-row single windows faster, but
40,000 rows took 1.40 s native versus 1.10 s matched WASM. One 40,000-row/ten-window
native run also had a 182 ms checkbox p95, despite that cell's median of 35 ms.
Do not turn favorable medians into a promise that every interaction is smooth.

Treat native ownership as a serious candidate for a shared Rust document core.
The bridge is not, by itself, a reason to keep the authoritative replica in the
WebView. Native ownership can remove per-renderer WASM initialization and keep
document work independent of renderer lifetime. Whether that improves a particular
interaction depends strongly on indexing, publication size and save scheduling.

Do not promote this Swift interpreter into the production runtime yet. Its measured
checklist operations have focused correctness tests, but it lacks substantial
document semantics. [The coverage inventory](COVERAGE.md) records those gaps;
the conformance command intentionally stays red. Sharing only Loro while separately
reimplementing hitSlop semantics for Swift and Tauri would make the foundation harder
to maintain. Share the semantic core as well.

## Measurements

The final results are generated into [the table](evidence/table.md) and
[structured evidence](evidence/results.json). They contain five fresh processes per
candidate, dataset size and window count: 135 processes in total. Each process runs
20 samples of checkbox, text splice, move and paste, followed by 60 scripted
characters at 20 characters/second. All windows hold a document; only the front
document is edited. The matrix is not a simultaneous multi-document editing test.
The machine was an Apple M1 running macOS 26.6.2; the final run completed on
2026-09-28. Latency values below are medians of five per-process p95s, not pooled
p95s across the entire dataset.

| Rows, one window | Checkbox ms, WASM/native | Move ms, WASM/native | MiB, WASM/native | Open ms, WASM/native |
|---|---|---|---|---|
| 1,000 | 7 / 2 | 7 / 2 | 152 / 104 | 427 / 354 |
| 5,000 | 16 / 5 | 35 / 6 | 181 / 124 | 501 / 448 |
| 40,000 | 72 / 29 | 253 / 33 | 541 / 185 | 1,100 / 1,397 |

All six ordinary cells (1,000/5,000 rows × 1/5/10 windows) passed the structural
acceptance, render-opportunity and queue-drain gates. Native structural p95s stayed
below 50 ms in every ordinary run. All stress cells passed on median p95, but the
five- and ten-window stress cells fail an every-run gate: their worst native move
p95s were 54 and 100 ms, and the ten-window checkbox outlier was 182 ms.
At ten 5,000-row windows, checkbox render opportunity was 34/35 ms and sustained
draft rendering was 33/33 ms:
faster acceptance does not necessarily change the next available frame.

The comparison gates are: native checkbox/move acceptance p95 at most 50 ms;
median render-opportunity p95 no more than 16.7 ms behind matched WASM for any
scenario; and less than 50 ms to drain pending edits after sustained typing in
every native run. The structured results include per-run ranges so these median
gates cannot hide tail variability.

The comparison has three candidates:

| Candidate | Document semantics | Loro core |
|---|---|---|
| Baseline WASM | Current production TypeScript source | 1.16.1 |
| Matched WASM | Same TypeScript source | 1.16.2 |
| Native | Partial Swift interpreter over native bindings | 1.16.2 |

Use **matched WASM versus native** for placement comparisons. The baseline shows
whether upgrading the control changed the result. Both controls keep their existing
atomic fork and native storage bridge. Native also retains fork staging; this does
not measure the benefit of removing atomicity machinery.

All candidates use the same seed snapshots, Svelte view, 40 rendered rows including
the final row, local SQLite implementation, full-sync policy and periodic 200 ms
save schedule. Every run asserts at least eight commits during sustained typing.
The native page requests no Loro/WASM resources. The executable and web assets are
frozen for the entire run, with hashes and machine details in the evidence.

Acceptance means the authoritative engine accepted the edit and its view was
published. It is separate from local flush and server durability. Two animation
frames after the Svelte update provide a render-opportunity proxy; they do **not**
measure compositor paint or physical keyboard-to-photon latency. Scripted local
typing is not an IME or concurrent text-editing test.

Opening is measured from engine/view setup to all views ready, in a fresh process,
after seed creation and SQLite handle acquisition. It includes reading/importing
state. It excludes process launch, catalog startup and cold filesystem caches.
Memory is host plus identified WebContent physical footprint after the workload;
GPU and network helper processes are excluded. These are comparison-shell numbers,
not promised production application startup or memory totals.

### Calibration and causality

An initial native version crossed Swift/Rust for every row lookup and republished
whole lists after moves. It performed poorly. The measured native version uses a
container-ID index and confirmed move patches. Consequently, its advantage is an
achievable adapter design, **not proof that placement alone causes the difference**.
Equivalent indexing can also improve the current WASM runtime.

An intermediate run revealed an unfair save schedule: native reset its 200 ms timer
on every edit while production schedules from the first unscheduled edit. That
reduced native persistence work during continuous typing. Both incomplete runs were
discarded from final statistics. The corrected run asserts periodic saves for every
candidate and retains its own frozen executable/assets. Local raw evidence retains
the discarded runs for diagnosis.

## Does the extra bridge make collaboration slower?

The separate transport experiment sends exact payloads through either a direct
WebView WebSocket or a Swift URLSession WebSocket reached over the bridge. It uses
five fresh processes, 20 measured round trips per cell, 256/4,096/65,536-character
payloads and 0/50/150 ms simulated RTT. Full ranges are in the evidence. This is a
loopback echo experiment; it does not exercise Cloudflare, authentication, persistence,
CRDT import or a remote user's frame.

Local feedback does not wait for the network in either architecture. Native ownership
adds an asynchronous UI-to-owner boundary, but moves persistence and the future sync
service next to the authoritative replica. WASM ownership still needs a bridge for
native persistence and host-owned credentials. Counting arrows alone does not predict
user latency; payloads, crossings per operation and queueing matter more.

A separate durable-log component test passed concurrent native text edits, native/WASM
convergence, lost-acknowledgement deduplication, refusal of changed bytes under a retry
ID, server-log reopening and native document reopening. It replayed 1,000 offline
edits in bounded pages in about 432 ms on this machine. The controller delivered pages
directly, so this is **not** a WAN or WebSocket synchronization result. Pending
dependencies must complete within a page; durable buffering across restart remains
unimplemented. The synthetic backlog is persisted in the relay, not a client-side
outbox, and the controller's receive cursor is in memory. Production cursor/outbox
crash recovery is not covered by this test.

## Smaller slops, easier updates and faster opening

Slop bundles already exclude the engine. The current host supplies the WASM runtime;
native ownership does not remove an embedded engine from each slop file. It can remove
per-view loading, initialization and working memory. Engine distribution is already
centralized today. A native library update normally travels with the host application;
it does not eliminate SDK or stored-data contract management after launch.

The useful architectural simplification is a renderer that can be recreated from a
host-owned document, plus one semantic core for Mac, future iOS and future Tauri. That
also moves snapshot decoding into the host's crash boundary, which requires deliberate
allocation limits, malformed-input tests and binding/panic handling.

## What the earlier native experiments actually established

The local `_docs/hitslop-sync/docs/native-loro-bridge-spike.md` recommended retaining
native ownership and text-draft ancestry. Its follow-up
`native-loro-incremental-spike.md` reported two-process collaboration against a
deployed Worker and recommended batching the relay. Those reports do not establish
that native Loro failed or that WebView WASM made collaboration intrinsically easier.
They used Loro 1.13.3 and a different journal/projection architecture; their durable
acknowledgement figures cannot be substituted for this experiment's acceptance times.

The most useful inherited lesson is that replacing a live input with every confirmed
host frame is incorrect: draft ancestry and composition need an explicit owner.
This spike does not claim to have re-proved that earlier work. It keeps those missing
text semantics visible and avoids restoring the archived persistent JSON projection.

## What changed in the recommendation from the earlier review

- **Do not simply remove the fork and roll back on failure.** Loro transactions group
  operations; they do not undo already-applied operations after an error. The native
  atomicity test was deliberately broken by switching staging to live mutation, and
  it caught both changed state and a changed version. A future typed-intent engine
  may validate a complete batch before applying it, but must prove that its apply
  phase cannot partially fail. See [Loro's transaction model](https://loro.dev/docs/concepts/transaction_model).
- **Do not drop counter checkpoints as a pure performance cleanup.** The pinned
  1.16.2 core reproduces a counter export/import regrouping failure: the live value
  is 1, replay is 0, and the version vectors match. The probe uses cancellation around
  10^16. Checkpoints preserve local reopen behavior; they do not solve peer convergence.
  Switching bindings or transport does not fix this. A counter policy or upstream fix
  is required before collaboration.
- **Do not infer durability from idempotent import or a version vector alone.** Our
  relay experiment persists entries with retry IDs and advances an in-memory receive
  cursor only after native flush.
  A different protocol may avoid an outbox, but must separately prove crash/reconnect,
  missing dependencies, acknowledgement loss and discard behavior.
- **A stable peer ID is not a free optimization.** Loro defaults to a fresh ID per
  document instance. Reuse requires restoring that peer's durable operation history
  and preventing concurrent allocation of overlapping operation IDs. A copied package
  or restored old checkpoint makes a naive device-plus-document-ID scheme unsafe.
  Keep the safe default until a host-local identity/ownership protocol proves those
  cases. See [Loro's peer-ID rules](https://loro.dev/docs/concepts/peerid_management).
- **Avoid separate semantic implementations.** The native prototype establishes a
  feasible boundary. A shared Rust interpreter with thin Swift/Tauri adapters is a
  cleaner production direction than maintaining this partial Swift interpreter beside
  the full TypeScript implementation indefinitely. The upstream [Swift bindings](https://github.com/loro-dev/loro-swift)
  are explicitly experimental; a production binding and toolchain policy need ownership.

## Type safety and the authoring boundary

The spike generates a tagged Swift operation union from TypeBox and uses a recursive
`JSONValue` for dynamic application data. This avoids passing arbitrary `Any` through
document operations. It does not pretend arbitrary app schemas are statically known
to the host: untrusted bytes still require decoding and runtime validation.

The proposed SDK collects typed intents synchronously and returns an asynchronous
acceptance promise. A throwing callback submits nothing. `flush()` waits for earlier
acceptance and local persistence. Text input needs an SDK-owned draft/composition
binding, not a hand-written promise chain in every slop. Structural edits need stable
application IDs and text anchors, not stale array indices. The prototype's small
scalar/text/counter SDK demonstrates the boundary; it is not the complete API.

For the manifest question, Foundation's `[String: Any]` is a dynamic JSON boundary,
not end-to-end type safety. A typed read DTO/custom decoder can keep tolerant enum
fallbacks while preserving original bytes, followed by the strict application model.
That cleanup is independent of where Loro runs; this spike does not change production
manifest decoding.

## Delivery and remaining decision gates

Implemented: isolated Mac harness, matched-core controls, native interpreter subset,
typed intent SDK, real SQLite durability, fault-injection tests, atomicity sensitivity,
projection audit, transport echo, durable-log replay, repeatable matrix and future
platform boundary design. No production engine, release record or template was replaced.

Before a cutover, require the full [coverage inventory](COVERAGE.md): identity and
anomaly issue parity, nonempty trees, native creation/mergeable initialization, complete
collection APIs, request idempotency, persisted pending imports, undo, real text/IME
and remote-draft tests, renderer crash/remount and process-death durability. Then
measure the fully conformant core again. Performance from a partial interpreter is
evidence to continue, not permission to weaken semantics.

The proposed [architecture](ARCHITECTURE.md) plans iOS and Tauri without building them
now. A prelaunch cutover can update the SDK/templates together and delete the old engine
path; no compatibility layer is proposed. Existing Mac product capabilities still
need to survive that replacement.

Validation for this spike: root check passed; 154 repository tests, 115 compatibility
replay cases and 52 template open/reopen checks passed; the spike's ten Swift tests,
SDK test, strict TypeScript, Svelte checks and generated-operation drift check passed.
The conformance audit and numeric counter probe intentionally report an incomplete/
failing gate. No full production native/release gate or template sealing is claimed.
