# Pre-launch simplification

This is a breaking pre-launch cleanup. Development documents are recaptured; no released
format adapter or migration is introduced. Changes remain uncommitted for review.

## Resulting ownership

One Rust owner admits edits, publishes state, schedules persistence, handles discard and
close, and routes Unix socket commands. The app, native renderer helper and CLI engine
use that implementation. Swift retains the window, page lifecycle, native services and
ordered delivery. Authored JavaScript never runs for closed data edits.

This does not remove Loro's replica semantics. Mergeable containers, per-peer counters,
anomaly reporting and convergence tests remain. Future sync needs a transport,
authentication, retention policy and remote-edit undo policy; it does not need another
document engine.

Theme overrides live in Loro and share the document's sequence, publications, undo and
saving. JSON data replacement preserves the palette. A color-panel gesture forms one
undo step, and independent agent edits remain separate.

Exports, previews and icons use a saved document in a disposable renderer. For an open
document, only draining, saving and copying hold the editor's capture barrier. Rendering
can continue after the editor closes. Optional `Export.svelte` remains; the fallback is
a fresh `App.svelte` using its default transient view state.

## Removed duplication

- Swift owner scheduling and socket transport; command routing now lives in Rust.
- Theme storage outside the CRDT and separate page theme delivery.
- Loose socket success envelopes and their second result-shape table.
- Individual native JavaScript command strings, replaced by generated host requests.
- Separate native/browser CSP definitions.
- Custom authoring entry points and CLI WASM validation.
- Public live compaction; automatic retention and storage limits remain.
- Empty new documents that acquire their first checkpoint only when opened.

Scalar bindings, previews, `doc.at`, JSON import, descriptor kinds, TCA and the macOS
integrations remain. The page's `resync` message also remains: the host actually sends it
when its publication queue overflows.

## Measurements

- [Snapshot capture](capture-snapshot-2026-10-04.md): 741 ms paired added p95 at 5,000
  rows across 20 captures, below the one-second gate.
- [Document growth](document-growth-2026-10-04.md): a 30-day continuously open workload
  generated 84.7 MB across 3,120 commits. Sampled file size peaked at 16.4 MB and ended
  at 0.68 MB with the existing automatic retention policy.
- [Synthetic theme gesture](theme-drag-2026-10-04.json): all 60 changes accepted and
  one Undo restored the starting palette at both 10 and 1,000 rows. At 1,000 rows,
  acceptance p95 was 0.27 ms and observed CSS p95 was 41.63 ms. This Debug measurement
  does not establish physical picker or paint latency.

## Regressions caught during integration

- macOS can reject a receive-timeout socket option after the peer closes. The shared
  transport now uses nonblocking I/O with deadline-bounded `poll`, including tests for
  buffered replies after close, stalled reads and backpressured writes.
- An accepted command whose final state became uncertain could carry the correct
  `unknown_outcome` code with an incorrect "not applied" message. The regression test
  failed before the fix. Confirmed acceptance now reports that its final state could
  not be confirmed; transport loss does not assert acceptance that the client never saw.
- A native export queued across discard/reopen could capture the replacement owner's
  state. The live request's epoch now reaches snapshot acquisition and is checked across
  its asynchronous flush/copy. The regression failed before the fix. Once acquired, the
  copy can still finish rendering after discard or close.

## Verification

The following checks passed against the implementation:

- `bun run check`, `bun run hygiene`, landing documentation checks and `git diff --check`.
- `bun run test`: 128 tests, no failures.
- `cargo test --locked --workspace`: 226 tests, no failures; four subprocess test
  helpers remain intentionally ignored.
- `bun run build`, `bun run check:built` and `bun run build:templates`.
- `bun run test:native`: 19 tests, no failures.
- `bun run swift:test --disable-sandbox`: 189 tests, including native compatibility,
  save failures, stale epochs, captures and close. Six focused tests then passed after
  fixing the export epoch regression, including source lifetime after discard/close,
  deadline expiry and failed-save refusal.
- `bun run test:render --fixtures`: PNG/PDF and document/master integrity for all four
  fixtures.
- `bun run test:native-helper`: the relocated helper and engine work without the checkout
  or Bun; damaged rendering resources do not prevent data-only commands.
- `bun run compat:capture dev` and `bun run test:compat`: the development corpus was
  replaced and replayed (eight documents, five templates and its CLI transcript).

- Local arm64 Release Xcode build and `scripts/release-artifact.ts`: packaged templates,
  matching core IDs and page shells, Quick Look declarations, and installed editing and
  PNG/PDF export without Bun/Node passed.
- `scripts/crash-matrix.ts --host` with the Release app and bundled helper: 24 kill
  timings, seven interrupted edits, no torn documents; an acknowledged live edit survived
  killing its host. The harness now honors `HITSLOP_NATIVE_CLI` for every operation.

The actual Release UI opened an isolated document, accepted typing and CLI edits to its
title, and restored the preceding text through native Edit > Undo. Quit saved preview
and icon artwork and released the writer lock. One earlier CLI reply expired during a
long machine/task pause; read-back confirmed the saved edit and a fresh live mutation
returned successfully in 0.16 seconds. No mutation was automatically replayed.

Physical color-picker dragging, system IME composition and Finder/Quick Look behavior
on quarantined Mail/AirDrop copies remain manual launch checks. The synthetic theme
gesture and integration tests cover their document semantics, not physical interaction
or quarantine handling. Restoring archived examples and selecting the launch collection
remain the next product work.

This is a local unsigned Release build; no release has been signed for distribution or
published. The final incremental Release build and artifact/crash checks passed after
the export-epoch fix.
