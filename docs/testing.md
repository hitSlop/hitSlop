# Testing

Tests live at the boundary that owns the behavior. Delete a test together with the code
it protects, and never bend production code to keep an old test compiling. The one
exception is the [compatibility corpus](#compatibility-corpus): released documents stay
openable, so its frozen entries never change.

| Boundary | Where | Proves |
|---|---|---|
| Rust semantics | `crates/hitslop-core/tests`, `bun run verify rust` | Descriptors, validation, atomic batches, row identity, publications equal a fresh snapshot, counters, text merges, byte export/import, FFI panic containment |
| Rust storage | `crates/hitslop-core/tests/{store,file}.rs` (feature `storage`) | The file: packing, hostile layouts and rows refused before a value is read, stored values bounded as writes bound them, newer markers, templates never opened as documents, copies never overwriting, the registry lock and discovery, renames and hard links (and saving and reloading once moved back), attachments, artwork written by the writer, ranged asset reads, crash recovery. Saving: storage identity, limits before blob reads, busy and full saves, a failed write never advancing the saved version, snapshots that never write, free-page reclamation, theme overrides, saved updates without a checkpoint refused. Faults are real and deterministic: another connection holding the database, a moved file, a child process killed mid-commit, after each kind of save and while a save waits, and a save retried after a lost reply |
| File engine | `crates/slop-engine/tests` | `pack`, `inspect` and `schema` as the CLI runs them; a refused build publishes nothing |
| Shell over WASM | `packages/shell/tests`, `bun run verify bun` | Async write timing, snapshot identity, collectors, bindings, barriers, attachments, the shared fixture replay (`fixtures.test.ts`) |
| Author SDK | `packages/document/tests` | Descriptor types, cross-bundle errors and framework-neutral helpers |
| Rust owner and commands | `crates/hitslop-core/tests/{owner,command}.rs` | Ordered admission and publications, autosave, edits during slow persistence, failed-close retention, discard fencing, data/theme undo, live socket routing, deadlines and unknown outcomes |
| Swift integration | `apps/apple/Packages/HitSlopApple/Tests`, `bun run verify swift` | Native event delivery, save/reopen, failure UI, CLI live and closed paths, WebView bridge, saved-state capture, window lifecycle |
| Native tools | `tests/native`, `packages/cli/tests/*.native.test.ts`, `bun run verify native` | The CLI against the helper, the helper relocated into an app bundle, every template's native render, a helper or host killed mid-edit, and the corpus replay |
| Packed packages | `tests/packed`, `bun run verify packed` | The published npm tarballs installed outside the checkout without Node: SDK types, init, check, build, preview and the getting-started tutorial |
| Examples | `tests/examples`, run with the package tests (`verify bun`, or `verify native` for `*.native.test.ts`) | An example's own behavior in WebKit through `slop dev`: editing, composition and captures. Kept outside the example, so a copied example stays self-contained |
| Compatibility corpus | `tests/compat`, replayed by the three tiers [below](#compatibility-corpus) | Every released template and saved document still opens, renders, edits and reopens |

`tests/fixtures/*` are small host apps (`document/`, a build stage, with `expected.json` and
`scenario.json`) replayed by both the Bun fast tier and the Swift host-path test, which
packs each stage and creates a document from it.
`tests/abi/{owner-svelte,probe}` are Svelte and probe consumers used by Swift tests.

## Running tests

`bun run verify` is the one runner (`scripts/verify.ts`). Without arguments it runs each
tier whose inputs changed since that tier last passed on this machine (recorded in
`.hitslop/verify/`, by content), so a repeated run after a green one does nothing:

```sh
bun run verify                 # the tiers this change touches
bun run verify --native        # the same, with the native (macOS) tiers
bun run verify --all           # every tier but the native ones (with --native: every tier)
bun run verify rust store::    # one tier, with its own arguments (here a nextest filter)
bun run verify --list          # what would run, and why
bun run release:check          # verify --release: every tier, the shipped builds, a report
```

| Tier | Runs | Typical (M1) |
|---|---|---|
| `hygiene`, `contracts`, `types` | Repository rules; generated contracts and skills; TypeScript and template types. Together, concurrently | 5 s |
| `bun` | Package and example tests over the WASM core, in parallel (`bun test --parallel`) | 25 s |
| `rust` | The Rust suite with cargo-nextest, one process per test; a test running two minutes is a named hang | 25 s after an edit |
| `landing` | The site's type check (and build, on release) | — |
| `packed` | `tests/packed`, when what the npm packages ship changes (their sources, starter, skills, page shell or packing) | 20 s |
| `swift` | The Swift package in three concurrent process shards, balanced by recorded durations; every listed test must run | 75 s |
| `native` | `*.native.test.ts` against the debug helper | 60 s |

Each tier builds what it needs first (the WASM core and shell, or the native build), and a
build whose inputs did not change rewrites nothing, so nothing downstream recompiles: a
repeated `bun run build` takes seconds. Durations and outcomes go to
`.hitslop/evidence/verify.json` (`release-check.json` for a release); a tier slower than
its budget says so.

While changing Rust, iterate with `bun run verify rust <filter>` (or `cargo check
--workspace --tests`), then run `bun run verify` before calling the step done. Run
`bun run verify --native` once at the end when Swift, the FFI surface or the helper
changed. `bun run check`, `bun run test`, `bun run core:test`, `bun run swift:test` and
`bun run test:native` remain as names for single tiers.

On a Mac, let the terminal skip the first-launch check of every freshly linked test
binary (seconds each under load): run `sudo spctl developer-mode enable-terminal`, then
enable the terminal app under System Settings › Privacy & Security › Developer Tools and
restart it.

`crates/hitslop-core/tests/model.rs` is the descriptor-driven model of the single writer:
one test per fixture (checklist, scalars, collections, nested), 8 seeds × 150 steps by
default, over the edits one owner actually receives. Those are the agent's and the page's
batches (including boundary and out-of-range values, anchors, whole scalar-list `set`,
multi-element removes and `replace`), page text clients whose edits arrive late against
older versions, undo and redo. After every step a refusal has changed nothing,
publications replay to a fresh snapshot, the maintained state equals a recomputed one, and
the document reopens the same: reopening checks the saved state against its descriptor,
so a write that stored invalid state fails the run. `publications.rs` keeps its own budget
(100 rounds × 100 steps, `HITSLOP_PUBLICATIONS_{ROUNDS,STEPS}`).

The default model takes under two seconds in the debug build (M1, 2026-10-05). The
extended run is sized for CI; the `Core model` workflow runs it weekly and on demand:

```sh
bun run core:test:extended   # HITSLOP_MODEL_SEEDS=64 HITSLOP_MODEL_STEPS=500, release build, nextest
```

It passed in under three minutes on the same M1 (2026-10-05).

## Native (macOS)

```sh
bun run verify swift native             # builds first, as each tier needs
bun run verify native crash             # one native test file, by path
bun run verify swift --filter Compat    # one process, Swift's own filter
```

`bun run build` generates contracts, builds the Rust bindings and the page shell, and
compiles the helper. `tests/native/crash.native.test.ts` is stress coverage beside the
deterministic Rust storage cases: it kills a real helper at points spread across an edit's
open, apply, save and close, and checks that the document reopens as it was or as edited,
never torn, and that an acknowledged edit is never lost. With `HITSLOP_APP_BINARY` (a
release builds one) it also kills a running app after an acknowledged CLI edit.
`tests/native/render.native.test.ts` renders the native fixtures; `HITSLOP_RENDER=all`
renders every bundled template (release).

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
`scripts/dev/bench-webkit.ts` (Playwright WebKit) and `scripts/dev/bench-wkwebview.swift`
(plain system WebKit). `HITSLOP_BENCH_CSS` appends CSS to the measured checklist and
`HITSLOP_BENCH_LABEL` names the report, for attributing a cost to one rule; the
`row_text_split_ms` field separates style from layout. Record a result under
`docs/evidence/` when a doc cites it; a run the code has since superseded moves to
`archive/docs/evidence/`.

`bun run bench:growth` (it needs Doodle Board, Pixel Art and Morning Pages back in
`examples/slops`) simulates up to 365 days of heavy use of those three through the native owner, with normal checkpoint thresholds
and daily save/close/reopen checks; each daily close trims history as the app does. Build the templates first. The report records
logical checkpoint/update bytes separately from physical SQLite size, retained
content, and measured versus projected lifetime. A storage-full or reopen failure
stops that workload and blocks release in the report; it does not change storage
limits. Other workloads still run. Failed documents are retained under
`.hitslop/evidence/document-growth/` for inspection.

For a harness smoke, use `HITSLOP_GROWTH_DAYS=1` and set `HITSLOP_GROWTH_OUTPUT`
to a temporary JSON path. A short run does not establish a year's capacity. Without
an output path the report goes to `docs/evidence/document-growth-<today>.json`. These
are synthetic workloads, not forecasts of individual users' behavior. A successful benchmark test means the measurements
completed; inspect `releaseBlocked` and each workload's stopping reason before release.

## Compatibility corpus

`tests/compat/<release>/` stores original built templates, saved documents (with their attachments),
expected state, CLI transcripts and explicit page interactions. Its documents are written
both ways a release writes them: through its CLI and helper, and by its own app's page
(`NAME.page.slop`, saved by the page scenario, so text splices against older versions and
page-minted IDs are replayed too). A small sample includes
the conformance app, three type fixtures and selected real templates. It is regression
evidence, not proof of all possible authored apps.

`bun run compat:capture VERSION --frozen` builds the producing tools and templates (the
shipped ones in `examples/slops/bundled.json`, or `--templates slug,slug`), records their
source fingerprint and identities, and captures into a temporary directory.
It records template/archive content digests, per-file hashes, dependency installation lock,
and required case inventory. Only a completed capture is published and frozen. Recording
an existing frozen entry is refused. Before launch `dev` may be recaptured.

- Rust (`crates/hitslop-core/tests/compat.rs`, in every `verify rust`) replays saved
  values, themes, edits, save/close and reopen.
- Rust (`compat_writers.rs`) also replays every release's writer, not only its sample: the
  file engine each entry's CLI shipped creates documents from the entry's templates and
  applies generated batches, and this core must read exactly what that engine reads, then
  edit, save, trim and reopen them. 2 seeds × 8 batches per template by default; the
  weekly `Core model` run uses 16 × 40 (`HITSLOP_COMPAT_SEEDS`, `HITSLOP_COMPAT_STEPS`).
- Native replay (`tests/native/compat-replay.native.test.ts`) checks inventory/hashes,
  original app rendering, PNG/PDF, attachments, template creation, and the commands each
  release's CLI ran, through this build's CLI and helper. A release also sets
  `HITSLOP_COMPAT_RELEASE`, which requires the tagged frozen entry.
- Swift (`CompatCorpusTests`) runs frozen explicit UI actions or the old conformance app's own scenario,
  requires an actual saved edit and checks reopen. Missing controls fail.
- `HITSLOP_COMPAT_INSTALLED=1 bun run verify native compat-replay` installs each archived
  CLI with its frozen lockfile and replays reads, writes, refusals, themes and attachments through its public executable.
  Envelope metadata can grow; document contents compare exactly. Session metadata is
  ignored only at documented envelope locations, never inside user values.

Hygiene compares frozen entries with protected Git history and verifies their content
hashes, and refuses an archived package that carries a machine path (a home folder or a
mounted volume): a release publishes the captured packages as they are, and release
builds name dependency sources by a fixed prefix (`scripts/build/core.ts`). Release checks
additionally require the tagged entry to match current producing inputs, selected shipped
templates and npm package contents. Package contents compare byte for byte except the
file engines: native executables differ by machine even from the same sources, so each
engine is checked by the core build its `engine.json` records. A corpus-only commit does
not change the input fingerprint. Historical entries never need current build identities.
The release retains the exact tested frozen npm archives. Signed-helper acceptance also
runs archived CLIs before notarization.

## CI

| Job | Runs |
|---|---|
| `fast` (macOS) | `verify --all`: every tier but the native ones, including frozen corpus hygiene and the Rust corpus replay |
| `native` (macOS) | `verify swift,native`, when the change touches what they cover (verify's tier table decides) |
| `linux-smoke` (Ubuntu) | The core as WASM builds it (no storage) and the CLI's file engine with bundled SQLite |
| `release-templates` (master) | builds and caches the full template corpus |
| Release macOS (`macos-v*` tag, or manual dry run) | `release:check` (`verify --release`, including the Rust suite), sign, notarize, publish |

`fast` runs on pull requests, master pushes and manual runs; feature-branch pushes don't
repeat PR checks. `native` always reports; it skips its tools when the change touches no
native tier. Branch protection requires `fast`, `native` and `linux-smoke`. The full
`release:check` runs only in the Release macOS workflow. Reports live in
`.hitslop/evidence/` and are uploaded even on failure.

## Writing tests

- Name the observable failure, an independent expected result and the gap in existing
  coverage before adding a test. Extend the owning boundary's existing case table first.
- Private fields, internal call sequences, literal CSS/HTML and mocks that implement the
  asserted behavior are not contracts. Bridge envelopes, storage durability and
  save-before-close ordering are.
- A bug regression test must fail on the old code for the intended reason, then pass.
- Faults are real, at real I/O boundaries: another connection holding the database, a
  moved or read-only file, a killed process. Production code has no fault hooks. Prefer
  observable completion over sleeps.
