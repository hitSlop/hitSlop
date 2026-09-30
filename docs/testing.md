# Testing

Tests live at the boundary that owns the behavior. Earlier 1.x builds are unsupported;
tests cover refusal of unsupported storage and mismatched core identities, not legacy
compatibility. Delete a test together with the code it protects, and never bend
production code to keep an old test compiling.

| Boundary | Where | Proves |
|---|---|---|
| Rust semantics | `crates/hitslop-core/tests`, `cargo test --locked --workspace` | Descriptors, validation, atomic batches, row identity, publications equal a fresh snapshot, counters, text merges, byte export/import, FFI panic containment |
| Rust storage | `crates/hitslop-core/tests/store.rs` (feature `storage`) | Writer lock, storage identity, limits before blob reads, busy and full saves, lost acknowledgements, moved packages, snapshots that never write, free-page reclamation, duplicate identity, theme overrides, saved updates without a checkpoint refused |
| SDK over WASM | `packages/document/tests`, `bun run test` | Async write timing, snapshot identity, collectors, bindings, barriers, attachments, the shared fixture replay (`fixtures.test.ts`) |
| Swift integration | `apps/apple/Packages/HitSlopApple/Tests`, `bun run swift:test`, `bun run test:native` | Save scheduling, save/reopen, failed-save retention, lost-reply recovery, CLI live and closed paths, WebView bridge, export, window lifecycle |

`tests/fixtures/*` are small host packages (`document/`, `expected.json`,
`scenario.json`) replayed by both the Bun fast tier and the Swift host-path test.
`tests/abi/{owner-svelte,probe}` are Svelte and probe consumers used by Swift tests.

## Everyday checks

```sh
bun run check   # generated contracts, skills, TypeScript/Svelte types
bun run test    # Bun SDK/CLI/schema tests over the WASM core
cargo test --locked --workspace
```

`crates/hitslop-core/tests/model.rs` is the descriptor-driven model: one test per fixture (checklist,
scalars, collections, nested), three peers, 8 seeds × 150 steps by default. Each step
applies a generated batch, including boundary and out-of-range values, anchors, whole
scalar-list `set` and multi-element removes. It checks that accepted and refused batches
match an independent model, that most batches are accepted, and that patch replay,
convergence, checkpoint reopen and seed-plus-update-log reopen all agree. The targeted
workloads keep their own budgets: `publications.rs` 1,000 rounds × 100 steps,
`nested.rs` 300 × 60 and `chaos.rs` 300 × 40, each overridable with
`HITSLOP_{PUBLICATIONS,NESTED,CHAOS}_{ROUNDS,STEPS}`.

The default model took 44s in the debug build (M1, 2026-09-30). Cost grows about
quadratically with steps: 2,000 steps would take about six hours per fixture. The
extended run is sized for CI; the `Core model` workflow runs it weekly and on demand:

```sh
bun run core:test:extended   # HITSLOP_MODEL_SEEDS=64 HITSLOP_MODEL_STEPS=500, release build
```

It passed in 29 minutes on the same M1 while other builds ran (2026-09-30).

## Native (macOS)

```sh
bun run build
bun run swift:test
bun run test:native
bun run test:native-helper
bun scripts/crash-matrix.ts           # add --host for host death (test:native-crash)
bun run test:render --fixtures
bun run test:restored                    # restored slops in `slop dev` under Playwright WebKit
```

`build` generates contracts, builds the Rust bindings and the page shell, and compiles
the helper. The crash matrix pauses a real native write at each storage phase, kills it
and checks writer exclusion and old-or-new recovery through `hitslop-native get`.

Capture timing (one warmup and five samples at 1k/5k rows):
`HITSLOP_BENCH_CAPTURE=1 bun run swift:test --filter previewCaptureCost`.
This writes `.hitslop/evidence/preview-capture.json`; preparation and total native
capture are measured separately with the complete authored DOM.

Startup diagnostics: Debug `HITSLOP_STARTUP_TIMINGS=1` logs native preparation, page
readiness (fonts included), presentation and page-relative `hitslop:*` boot marks, without
document values or paths. `HITSLOP_STARTUP_BENCH=1 bun run swift:test --filter documentStartupTimings`
measures fresh documents. For saved ones, use
`HITSLOP_STARTUP_BENCH=1 bun run swift:test -c release --filter savedDocumentStartupTimings`:
it seeds saved edits in a separate helper process, then reopens Quick Checklist, a
1,000-row checklist and a skinned fixture ten times each, recording
preparation/readiness/reveal durations and whether progress appeared.
`HITSLOP_STARTUP_CASE` measures one case first in a fresh process,
`HITSLOP_STARTUP_SAMPLES` sets the count and `HITSLOP_STARTUP_FOREGROUND=1` activates the
benchmark app. Report the first sample separately, then the warmed median and p95.

Performance diagnostics are opt-in and not CI gates:
`HITSLOP_BENCH=1 HITSLOP_BENCH_ROWS=1000,5000 HITSLOP_BENCH_WINDOWS=1 bun run bench:windows`,
`scripts/bench-webkit.ts` (Playwright WebKit) and `scripts/bench-wkwebview.swift`
(plain system WebKit). `HITSLOP_BENCH_CSS` appends CSS to the measured checklist and
`HITSLOP_BENCH_LABEL` names the report, for attributing a cost to one rule; the
`row_text_split_ms` field separates style from layout. Record results under
`docs/evidence/`.

`bun run bench:growth` simulates up to 365 days of heavy Doodle Board, Pixel Art,
and Morning Pages use through the native owner, with normal checkpoint thresholds
and daily save/close/reopen checks. Build the templates first. The report records
logical checkpoint/update bytes separately from physical SQLite size, retained
content, and measured versus projected lifetime. A storage-full or reopen failure
stops that workload and blocks release in the report; it does not change storage
limits. Other workloads still run. Failed packages are retained under
`.hitslop/evidence/document-growth/` for inspection.

For a harness smoke, use `HITSLOP_GROWTH_DAYS=1` and set `HITSLOP_GROWTH_OUTPUT`
to a temporary JSON path. A short run does not establish a year's capacity. Without
an output path the report goes to `docs/evidence/document-growth-<today>.json`. These
are synthetic workloads, not forecasts of individual users' behavior. A successful benchmark test means the measurements
completed; inspect `releaseBlocked` and each workload's stopping reason before release.

## CI

| Tier | Runs |
|---|---|
| `fast` (Ubuntu) | hygiene, check, Rust tests, Bun tests, packed npm packages, landing check |
| `native` (macOS, path-filtered) | build, Swift tests, native CLI tests, fixture render, helper, crash matrix |
| `release-templates` (master) | builds and caches the full template corpus |
| Release macOS (`macos-v*` tag, or manual dry run) | `release:check`, sign, notarize, publish |

`fast` runs on pull requests, master pushes and manual runs; feature-branch pushes don't
repeat PR checks. `native` always reports and filters paths inside the job. Branch
protection requires both. The full `release:check` runs only in the Release macOS
workflow. Reports live in `.hitslop/evidence/` and are uploaded even on failure.

## Writing tests

- Name the observable failure, an independent expected result and the gap in existing
  coverage before adding a test. Extend the owning boundary's existing case table first.
- Private fields, internal call sequences, literal CSS/HTML and mocks that implement the
  asserted behavior are not contracts. Bridge envelopes, storage durability and
  save-before-close ordering are.
- A bug regression test must fail on the old code for the intended reason, then pass.
- Keep fault injection narrow and at real I/O boundaries (for example
  the store's `StorePhases` hook, set from Swift as `DocumentOwner.testingPhase`). Prefer
  observable completion over sleeps.
