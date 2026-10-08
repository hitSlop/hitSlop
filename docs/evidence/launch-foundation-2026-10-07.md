# Launch foundation implementation evidence

This records the first checkpoint. The subsequent
[authority and maintenance continuation](live-authority-2026-10-08.md) contains the
live two-view proof, bounded owner history, additional conformance fixtures and final
qualification. The remaining-work statements below describe this earlier checkpoint.

Verified implementation checkpoint on `feat/launch-foundation`, based on master `8dec1847`. The existing
staged authoring refactor was preserved and reapplied without conflicts. No commit or
release was made. Historical plans and reference archives remain local, as master now
requires.

## Before the fixes

The verifier selection included compat, tooling, contracts, types, Bun, CLI, Rust,
landing and packed packages. Swift/native qualification is also required for capture
changes. A filtered run is not a full-tier pass.

Rust regression run `2026-10-08T05-00-29-808Z-23792` selected 15 tests: 12 passed and
three failed for the intended reasons:

- An incomplete replacement accepted `{}` and deleted existing rows/text.
- Creating from a template with corrupt checkpoint bytes published a document.
- A nested/optional row command argument was accepted despite the top-level SDK rule.

An earlier fail-fast run selected the row-argument failure but skipped the other
regressions; a no-match filter run is not counted as evidence.

After the fixes, the targeted Rust suite passed 116 tests across defaults, arguments,
file acceptance, replacement, metadata, conformance, model and storage. Four existing
opt-in cases were skipped. This is not a full workspace/native pass.

The identity tests corrected an assumption in the proposal: undoing a deleted row
restores its public `$id` and content, but the pinned Loro recreates the row map and
nested text containers. A text edit based on the old incarnation refuses; a new binding
works. Complete replacement of a surviving row preserves its containers. No mergeable
container conversion was introduced to force a different undo result.

The compiler/type baseline caught two undefined-default type regressions and six
command/capture-check failures. The subsequent type and full CLI selection passed
86 tests (671 expectations), with the explicit debug evaluator and builds skipped.
Run: `2026-10-08T05-13-46-054Z-40537`. Later review added further declaration guard
cases: declarations in Svelte markup, repeated command factories, and loop/class sites.
All three failed before the fix; the focused 17-case CLI run and types passed in
`2026-10-08T05-24-18-809Z-54923`. The integrated runs below supersede this filtered evidence.

Two native regressions also failed before their fixes: a 4096-square fallback preview
exceeded the raster budget at 2×, and a 520-point dedicated export in a 280-point window
was clipped to 560 PNG pixels / 280 PDF points, losing the right-edge text. The fixed
renderer passes both and the checklist 0/1/40-row PDF/PNG cases in
`2026-10-08T05-29-10-447Z-60997`. Explicit PNG output stays 2×; a large fallback preview
can use 1×. Dedicated export layout is measured at its own width.

The existing counter probe reproduced `source=9007199254740991`,
`imported=9007199254740990`, `same_vv=true`. The repository now retains the diagnostic
as `crates/hitslop-core/tests/counter_exactness.rs` (ignored; run with `--ignored`). It reports the result;
it is separate from passing product tests. Exact authoritative counters remain i64.

## Integrated qualification

The unfrozen `tests/compat/dev` corpus was regenerated from distribution-profile tools.
The first page-recording attempt caught a stale driver action: it assigned `.value` to
Checklist's display DIV before the textarea mounted. The new scenario focuses first,
and the driver now rejects value actions on non-native controls. The saved-edit
assertion remains intact; all 13 page scenarios passed on recapture. Frozen entries
were not changed.

`bun run verify` passed in `2026-10-08T05-41-38-957Z-81636`, after updating one test-only
SQL insert to supply the newly required UUID. It includes 88 CLI tests, 85 SDK/shell
tests, 308 Rust tests (4 existing opt-in skips), landing checks and 4 packed-package
tests (the native-only case skipped here). Compatibility integrity, contracts and 22
tooling tests had already passed and were reused by the verifier; types passed again.
`bun run verify --native` passed in `2026-10-08T05-47-21-406Z-88038`: 198 Swift tests
across three complete shards and 82 native tests, with one existing opt-in host-kill
case skipped. This includes development-corpus render/edit/reopen replay, capture
regressions, crash atomicity, relocated-helper exports and the active examples.
The existing native preview command test measured 40 fresh evaluations at p50 53 ms,
p95 54 ms; that is local-owner evidence, not a shared-room latency result.

Phases 0–2 are complete for this checkpoint. Phase 3 has metadata and the core benchmark;
Store/owner/page workloads remain. Phase 4a's room proof and Phase 5's additional
representative fixtures/freeze review remain open. No room implementation, live-history
rebuild, storage-2 tables, production sharing or release qualification is claimed.

## Capture artifacts

Before artifacts for Quick Checklist, Hourglass and Shape Lab were written to a
temporary directory on the recording machine (not retained): PNG, PDF and preview PNG
for each.
These use the existing built templates. They are visual references, not a claim that
the exact staged source was recaptured: that build encountered a temporary mismatch
between the renamed command prelude and the old native helper.

After artifacts, also kept only on the recording machine, were built from the
current source: PNG, PDF and preview for all three active slops. Visual inspection kept
Checklist's three rows, completed state and progress; Shape Lab retained all four
corners. Hourglass's title is now whole: its PNG grew from the clipped 560×638 baseline
to 1040×638 without changing the export CSS. Extracted PDFs contain all task labels,
Hourglass's title/ready text, and Shape Lab's corner labels. These temporary artifacts
are local review evidence, not released assets.

Checklist native tests passed 6/6 in `2026-10-08T05-31-26-277Z-65467`, including Escape
after the accepted snapshot and editor-height updates after an external multiline edit.

## Core history measurement

`HITSLOP_FOUNDATION_ROUNDS=2000 /usr/bin/time -l target/release/examples/bench_foundation`
(a core-only harness since superseded by `bench_owner_history` and kept outside the
repository) ran after compiling the release example, with other agent builds, tests and browsers
paused. The fixed-seed workload has 4,000 rows, each with text and a nested child list,
then repeatedly creates and deletes a record containing 16 KiB of text and an optional
text field. Live data is checked unchanged at each sample.

[Raw samples](foundation-history-2026-10-07.json) record environment and limitations.
Initial open p50 was 40.6 ms, projection 8.6 ms and checkpoint export 0.7 ms. After
2,000 churn rounds, the full retained snapshot was 67,179,398 bytes while a current-state
checkpoint was 537,099 bytes. Full export took 80.6 ms at that point. Process peak RSS
was 892 MB, including oracle imports and repeated reopen allocations; this is not an
isolated live-owner memory measurement.

This core-only diagnostic intentionally samples full snapshots beyond the persisted
32 MiB limit and never writes them to `.slop` files. Production Store bounds the saved
checkpoint, but currently does not rebuild the long-lived LoroDoc when it trims. The
result supports a follow-up on live-history maintenance; it does not qualify a new
owner maintenance state transition. Measure that path and its failures before landing it.
Also, the full-snapshot probe occurs on checkpoint construction, not every append save:
`Store::job` normally appends until the existing row/byte thresholds are reached.
