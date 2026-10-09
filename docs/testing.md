# Testing

**Do not create an automated test or test suite for each slop.** Creating, styling,
animating, or updating a slop does not require new tests. Validate it with existing
checks/builds and hands-on preview/export review. Shared SDK, storage, or host
regressions belong in the existing tests at their owning boundary; do not duplicate
that coverage in example-specific tests. Ordinary tests use deliberate infrastructure
fixtures, never live examples. Generic shipped-template smoke checks and frozen
compatibility replay protect artifacts without asserting a slop's workflow or design.
Manual review and temporary diagnostic scripts do not become permanent tests.

Tests live at the boundary that owns the behavior. Delete a test together with the code
it protects, and never bend production code to keep an old test compiling. The one
exception is the [compatibility corpus](#compatibility-corpus): released documents stay
openable, so its frozen entries never change.

| Boundary | Where | Proves |
|---|---|---|
| Rust semantics | `crates/hitslop-core/tests`, `bun run verify rust` | Descriptors, validation, atomic batches, row identity, publications equal a fresh snapshot, counters, text merges, byte export/import, FFI panic containment |
| Rust storage | `crates/hitslop-core/tests/{store,file}.rs` (feature `storage`) | The file: packing, hostile layouts and rows refused before a value is read, stored values bounded as writes bound them, newer markers, templates never opened as documents, copies never overwriting, the registry lock and discovery, renames and hard links (and saving and reloading once moved back), attachments, artwork written by the writer, ranged asset reads, crash recovery. Saving: storage identity, limits before blob reads, busy and full saves, a failed write never advancing the saved version, snapshots that never write, free-page reclamation, theme overrides, saved updates without a checkpoint refused. Faults are real and deterministic: another connection holding the database, a moved file, a child process killed mid-commit, after each kind of save and while a save waits, and a save retried after a lost reply |
| File engine | `crates/slop-engine/tests` | `pack`, `inspect` and `schema` as the CLI runs them; a refused build publishes nothing |
| Shell over WASM | `packages/hitslop/tests/shell`, `bun run verify bun` | Async write timing, snapshot identity, collectors, bindings, barriers, attachments, the shared fixture replay (`fixtures.test.ts`) |
| Author SDK | `packages/hitslop/tests/sdk` | Descriptor types, cross-bundle errors, framework-neutral helpers and `EditableText` browser behavior |
| Rust owner and commands | `crates/hitslop-core/tests/{owner,command}.rs` | Ordered admission and publications, autosave, edits during slow persistence, failed-close retention, discard fencing, data/theme undo, live socket routing, deadlines and unknown outcomes |
| Swift integration | `apps/apple/Packages/HitSlopApple/Tests`, `bun run verify swift` | Native event delivery, save/reopen, failure UI, CLI live and closed paths, WebView bridge, saved-state capture, window lifecycle |
| Browser | `*.browser.test.ts`, `verify browser` (macOS qualification) | Chrome durable copies and WebKit native-owner preview, without building Swift or the helper |
| Native tools | `tests/native`, `packages/hitslop/tests/cli/*.native.test.ts`, `bun run verify native` | The CLI against the engine and helper, both relocated into an app bundle, every template's native render, an engine or host killed mid-edit, and the corpus replay |
| Packed packages | `tests/packed`, `bun run verify packed` | The published npm tarballs installed outside the checkout without Node: SDK types, init, check, build, preview and the getting-started tutorial |
| App bundle | `bun run verify app` (also in `release:check`) | Builds and checks the complete app, including named commands through the raw Xcode Debug app without an evaluator override; release native tests also exercise host process death |
| Release tooling | `tests/release`, in `verify bun` | Artifact identity, publication recovery and promotion rules |
| Presentation | `tests/presentation`, prepared for Swift/native tiers | Window shapes and capture fixtures |
| Verification runner and CI policy | `tests/verification`, in `verify tooling` | Tier selection, evidence, subprocess lifecycle and attribution; no product build |
| Landing/docs | `apps/landing`, `verify landing` | Public documentation and website checks; release includes the build |
| Compatibility corpus | `tests/compat`, replayed by the three tiers [below](#compatibility-corpus) | Every released template and saved document still opens, renders, edits and reopens |

`tests/fixtures/*` are small host apps (`document/`, a build stage, with `expected.json` and
`scenario.json`) replayed by both the Bun fast tier and the Swift host-path test, which
packs each stage and creates a document from it.
`tests/abi/{owner-svelte,probe}` are Svelte and probe consumers used by Swift tests.
`tests/apps` holds authored infrastructure fixtures: a small document app, an
`EditableText` consumer and the Shape Lab instrument. They are never shipped as
starter templates. Test media belongs to fixtures, not examples or the landing page.

## Running tests

`bun run verify` is the one runner (`scripts/verify.ts`). Without arguments it runs each
tier whose inputs changed since that tier last passed on this machine (recorded in
`.hitslop/verify/`, by content), so a repeated run after a green one does nothing:

```sh
bun run verify                 # the tiers this change touches
bun run verify --native        # the same, with the native (macOS) tiers
bun run verify --all           # all ordinary tiers (add --native for macOS tiers)
bun run verify browser         # optional browser qualification (macOS)
bun run verify dev-sync        # optional experimental collaboration qualification
bun run verify rust store::    # one tier, with its own arguments (here a nextest filter)
bun run verify --list          # what would run, and why
bun run verify --list --json --native --base origin/master # CI selection; no builds/tools
bun run release:check          # release acceptance, shipped builds, and a retained report
```

| Tier | Runs |
|---|---|
| `compat`, `contracts`, `types` | Frozen corpus integrity; generated Rust contracts; TypeScript and template types. Together, concurrently |
| `tooling` | Verification runner and CI policy tests, without Rust, WASM or native builds |
| `bun` | SDK, shell and release tests; up to four isolated file workers |
| `cli` | Non-native CLI integration tests; one file worker, 30-second default test deadline |
| `rust` | Clippy with warnings denied (the workspace and the WASM adapter), then the Rust suite with cargo-nextest, one process per test; a test running two minutes is a named hang. A filtered run (`verify rust store::`) runs only the tests |
| `dev-sync` | Experimental collaboration Clippy and Rust tests; explicitly selected only, outside ordinary and release runs |
| `landing` | The site's type check (and build, on release) |
| `packed` | `tests/packed`, when what the npm package ships changes (its sources, starter, skills, page shell or packing) |
| `swift` | The Swift package in three isolated process shards, bounded by available CPUs and balanced by full test identities. Shards run sequentially because even `swift test --skip-build` opens SwiftPM's shared build database; every listed test must run. A filtered run (`verify swift --filter X`) runs only the tests |
| `app` | Complete macOS app build and bundle acceptance; selected explicitly or by `release:check` |
| `browser` | `*.browser.test.ts` in Playwright WebKit and Google Chrome, with the Rust engine/evaluator, browser WASM and shell; no Swift build. Runs when named (`verify browser`), with `HITSLOP_NIGHTLY=1 --native`, and during release acceptance; nightly/manual qualification in CI, never a required PR check |
| `native` | `*.native.test.ts` against the debug helper |

Each tier builds what it needs first (the WASM core and shell, or the native build), and a
build whose inputs did not change rewrites nothing, so nothing downstream recompiles: a
repeated `bun run build` takes seconds. Each invocation retains commands, logs, inventory, toolchain identity and outcomes in
`.hitslop/evidence/runs/<run-id>/`. The latest report is also written to
`.hitslop/evidence/verify.json` (`release-check.json` for a release). Build and test
execution durations are reported separately, without fixed speed thresholds.
A filtered retry never deletes the original full-run report. Filtered and reused-build
runs are marked and never update the full-tier pass cache.

One verifier may run per checkout; the lock is released by the OS even after a crash.
Use a separate checkout for simultaneous runs. Each run has its own temporary directory
and writer registry (an explicit registry override is preserved). Test-process helpers
bound subprocess lifetimes, drain diagnostics and stop descendants on cancellation.
The packed preview uses an OS-assigned port and waits for its reported URL.

## Every change and nightly

PRs run affected critical correctness checks for opening, editing, saving and reopening
documents. Compatibility-sensitive changes also run full app-level corpus replay; other
changes use smoke coverage when native tiers are selected. Rust always replays every corpus
entry. Smoke coverage samples recent entries and representative app renders; matching
markers or embedded apps does not prove saved scenarios behave equivalently.

`HITSLOP_NIGHTLY=1` (set by scheduled/manual CI and release-branch pushes, and implied by
`--release`) adds expensive boundary cases and broader qualification:

| Nightly and release only | Where it is selected |
|---|---|
| Presentation, pixel and telemetry checks: window shapes, toolbar, glass, theme panel, export size and colour, catalog thumbnails, telemetry wiring | Swift `.nightly` trait (`HitSlopTestSupport/Nightly.swift`) |
| Storage-growth and history-trim budgets, rebuild scheduling, a busy display read | `default-filter` in `.config/nextest.toml` (`--ignore-default-filter` adds them) |
| Full randomized budgets: model 8 seeds (4 otherwise), compat writers 2 seeds (1), publications 100 rounds (10) | `scripts/verify.ts` |
| Large live/closed replies over 48 MiB with near-limit batches | `DocumentOwnerTests.largeDocumentsReadTheSameLiveAndClosed` |
| The type fixtures' fresh native render (their saved documents are replayed every time) | `tests/native/render.native.test.ts` |
| Landing generation, skills install, update notice, help wording, documentation examples, the tutorial build | `test.if(nightly)` in those files |
| The `browser` tier (Playwright WebKit and Chrome) | the tier's `nightly` flag; `verify browser` runs it on demand |

Full compatibility is independent of nightly checks. CI selects it for changes to Rust,
native hosting/export, SDK/runtime contracts, corpus/fixtures and their build dependencies
(`compatibilityInputs`). It replays every entry through Swift/native and exports every saved
document to both PNG and PDF. Other changes retain the newest entry per marker generation
and one representative render per embedded app. This is sampling, not an equivalence rule.
`HITSLOP_COMPAT_MODE=full bun run verify --native` requests full compatibility locally without
enabling expensive nightly cases. A local `--base REF` run expands automatically for sensitive
changes; otherwise ordinary local runs default to smoke. Reports record the mode, and smoke
passes cannot satisfy a full verification cache entry. Nightly and required-release replay
override inherited smoke restrictions.

Explicit Rust/dev-sync filters bypass nextest's default exclusions, so
`bun run verify rust a_busy_file_is_reported_busy_not_as_having_no_artwork` runs that test.

A merged failure scenario keeps each failure it replaced named in its comment. Tests that
wait out SQLite's busy timeout run their cases at once on separate documents, so a run
pays that wait once.

## Local authority proof

`bun scripts/dev/live-sync.ts examples/slops/quick-checklist` builds the opt-in
`dev-sync` engine and its `slop-room` binary in `target/dev-sync`, packs one app and
prints two Vite URLs and a
temporary directory. Open one browser page per URL. The room and each replica own a
different SQLite file; ordinary `slop get`/`slop call` against a replica path reaches
that live owner through the usual socket registry. Stop the script with Ctrl-C; its
files remain available for inspection. Do not use these files as shared documents after
the harness stops: storage 1 has no persistent room binding.

`bun run verify browser live-sync` builds those binaries and runs the two-view WebKit test
(it is skipped when run without them),
including CLI bursts, text and commands, duplicate/missing delivery, snapshot replacement,
disconnect fencing, retained drafts and reopen. With `HITSLOP_BENCH_SYNC=1` the test records
loopback command/text timings and screenshots in its verification evidence directory. `bun run verify rust`
also checks the feature-enabled owner, import and bounded framing tests alongside the
ordinary configuration. Released builds do not enable this feature.

All Loro bytes stay in Rust. Credentials are ephemeral and passed through stdin; the
page receives ordinary owner publications. Shared undo/redo and attachment imports are
refused. There is no automatic mutation retry after an unknown outcome, offline merge,
restart recovery or production endpoint in this proof.

Use `bun run verify cli agents.test.ts` for a CLI case. `bun run test` runs `tooling`,
`bun` and `cli`; `verify tooling` runs the build-free infrastructure tests. Discovery rejects
unclassified test files rather than silently leaving them out.

While changing Rust, iterate with `bun run verify rust <filter>` (or `cargo clippy
--workspace --all-targets`). While changing Swift, use `bun run verify swift --filter <name>`.
Formatting is an explicit editing command: `cargo fmt --all` for Rust or
`bun run swift:format` for Swift. It is not a prerequisite for running tests. Run
`bun run verify --native` once at the end when Swift, the FFI surface or the helper
changed. `bun run check`, `bun run test`, `bun run core:test`, `bun run swift:test` and
`bun run test:native` remain as aliases (`test` covers both Bun tiers).

On a Mac, let the terminal skip the first-launch check of every freshly linked test
binary (seconds each under load): run `sudo spctl developer-mode enable-terminal`, then
enable the terminal app under System Settings › Privacy & Security › Developer Tools and
restart it.

`crates/hitslop-core/tests/model.rs` is the descriptor-driven model of the single writer:
one test per fixture (checklist, scalars, collections, nested), 8 seeds × 150 steps by
default (4 on an ordinary `verify`; see [Every change and nightly](#every-change-and-nightly)), over the edits one owner actually receives. Those are the agent's and the page's
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
deterministic Rust storage cases: it kills a real document engine at points spread across
an edit's open, apply, save and close, and checks that the document reopens as it was or
as edited, never torn, and that an acknowledged edit is never lost. With `HITSLOP_APP_BINARY` (a
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
it seeds saved edits in a separate helper process, then reopens the document fixture, a
1,000-row variant and a skinned fixture ten times each, recording
preparation/readiness/reveal durations and whether progress appeared.
`HITSLOP_STARTUP_CASE` measures one case first in a fresh process,
`HITSLOP_STARTUP_SAMPLES` sets the count and `HITSLOP_STARTUP_FOREGROUND=1` activates the
benchmark app. Report the first sample separately, then the warmed median and p95.

Performance diagnostics are opt-in and not CI gates:
`cargo run --release -p hitslop-core --example bench_replace` measures whole-list
replacement at 1k/5k rows: unchanged values, field edits, append, reverse and mixed
membership/order changes. It prints five samples and a median after one warmup per
case, with fresh document creation outside the timer. Compare builds sequentially
without concurrent tests or compilation; these timings are not test assertions.
The [2026-10-07 samples](evidence/replace-2026-10-07.json) compare the same-order
row fast path with the original reconciler; shared-machine timing variation limits
the comparison.
`cargo run --release -p hitslop-core --example bench_owner_history` runs a live owner
through 20 checkpoint cycles of constant-size churn and records retained history, peak
RSS, flush latency and the pause each history rebuild holds edits for
([2026-10-08 results](evidence/owner-history-2026-10-08.md)).
`cargo test -p hitslop-core --test counter_exactness -- --ignored` reproduces Loro
counter rounding past 2^53, the reason counters stay exact integers.

Other performance diagnostics:
`HITSLOP_BENCH=1 HITSLOP_BENCH_ROWS=1000,5000 HITSLOP_BENCH_WINDOWS=1 bun run bench:windows`,
`scripts/dev/bench-webkit.ts` (Playwright WebKit) and `scripts/dev/bench-wkwebview.swift`
(plain system WebKit). `HITSLOP_BENCH_CSS` appends CSS to the measured checklist and
`HITSLOP_BENCH_LABEL` names the report, for attributing a cost to one rule; the
`row_text_split_ms` field separates style from layout. Record a result under
`docs/evidence/` when a doc cites it. Superseded runs may be kept locally under
`archive/docs/evidence/`; remove or replace their tracked citations when untracking them.

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

The required compatibility checks answer: can this build open a saved slop, run its
embedded app, edit it, save it, and reopen the result without losing data or attachments?
Native page replay uses the actual WKWebView host, not Playwright. Keep these checks in
the required Rust, Swift and native tiers whenever their inputs change.

Before launch there is one replaceable baseline, `tests/compat/dev`, not a historical
public release. Starting with the first public release, capture a separate frozen
collection for **each released version**. Later builds replay every collection; never
rebuild old samples with current tools. These artifacts live in the test repository,
not the installed application's bundle. The format markers can remain at 1 across
many releases; release versions and format requirements are different things.

Old files are checked, not old programs. `tests/compat/<release>/` stores original built
templates, saved documents (with their attachments), expected state, explicit page
interactions, and the candidate writer: the darwin-arm64 `slop-engine` the release built
and wrote its documents with (`engine/darwin-arm64/slop-engine`, with its build ID, commit
and hash in `release.json`). Its documents are written both ways a release writes them:
through its CLI and helper, and by its own app's page (`NAME.page.slop`, saved by the page
scenario, so text splices against older versions and page-minted IDs are replayed too). A
small sample includes the conformance app (every `ctx` member and descriptor kind, a page
command, app media: font, image and audio, and two attachment types), three type fixtures,
the presentation fixtures for every window kind (1× and 2× PNG skins, glass, a transparent
ellipse and a path shape) and selected real templates. It is regression evidence, not
proof of all possible authored apps.

`bun run compat:capture VERSION --frozen` builds the producing tools and templates (the
shipped ones in `examples/slops/bundled.json`, or `--templates slug,slug`), records their
source fingerprint and identities, and captures into a temporary directory. It records
template digests, per-file hashes and the required case inventory. Only a completed capture
is published and frozen. Recording an existing frozen entry is refused. Before launch `dev`
may be recaptured.

- Rust (`crates/hitslop-core/tests/compat.rs`, in every `verify rust`) replays saved
  values, themes, edits, save/close and reopen.
- Rust (`compat_writers.rs`) also replays every release's writer, not only its sample: each
  entry's candidate writer creates documents from the entry's templates and applies
  generated batches in its own protocol, and this core must read exactly what that engine
  reads, then edit, save, trim and reopen them. Requests in older protocols live only in
  this test. 2 seeds × 8 batches per template by default (1 seed on an ordinary `verify`); the weekly `Core model` run uses
  16 × 40 (`HITSLOP_COMPAT_SEEDS`, `HITSLOP_COMPAT_STEPS`).
- Native replay (`tests/native/compat-replay.native.test.ts`) checks original app
  rendering, PNG/PDF, attachments and template creation, through this build's CLI and
  helper (the `compat` tier checks inventory and hashes). Full coverage exports every saved
  document, including different values saved with the same app. Smoke coverage samples the
  entries `replayedEntries` chooses and renders each embedded app once; its other documents
  still read, edit and reopen. Sensitive changes, nightly and releases use full coverage.
  Stored command programs replay twice: deterministically through the
  evaluator with their recorded clock and seed, and through the owner with `slop call`
  (argument refusal first, then an edit). A release also sets `HITSLOP_COMPAT_RELEASE`, which requires the tagged
  frozen entry.
- Swift (`CompatCorpusTests`) runs frozen explicit UI actions (field edits or clicks) or the
  old conformance app's own scenario, which includes a page command, requires an actual
  saved edit and checks reopen. Missing controls fail.

Each storage-version bump adds two Rust tests: a frozen file migrated by a write equals a
newly created file in exact layout and value, and an interrupted migration leaves the
older file intact.

The `compat` tier compares frozen entries with protected Git history and verifies their content
hashes. Release checks additionally require the tagged entry to match current producing
inputs and selected shipped templates, and its writer to come from the captured core. A
corpus-only commit does not change the input fingerprint. Historical entries never need
current build identities. The CLI and the app meet only through the command protocol, whose
refusal path is fixed and tested in each build; old CLIs are never run against new apps.

## CI

| Job | Runs |
|---|---|
| `select` (Ubuntu 24.04) | Selects affected tiers without installing dependencies or compiling; records the selection |
| `fast` (Ubuntu 24.04) | Affected compatibility integrity, tooling, generated-contract, type, Bun, CLI, installed-package and landing checks |
| `native` (macOS 15 ARM64) | Affected `swift,native` tiers: native page edits, the CLI and helper, and the app-level old-file replay |
| `native-rust` (macOS 15 ARM64) | The affected `rust` tier, in parallel with `native`: platform SQLite, Darwin sandbox and old-writer compatibility replay. WASM lints run on Linux only |
| `linux-smoke` (Ubuntu 24.04) | When Rust inputs change: full workspace tests/lints, WASM lint, no-storage configuration and bundled-SQLite engine coverage |
| `Gitleaks` (Ubuntu) | Introduced commits on PRs/master; full history weekly, manually, or when scanner rules change |
| `Attribution` (Ubuntu) | Every incoming commit's identities and attribution lines, plus PR title/description; trusted default-branch policy, including fork PRs |
| `qualification` (nightly/manual) | Playwright browser integration, experimental `dev-sync`, and the macOS portable CLI/package matrix; not required for merging |
| `release-templates` (nightly/manual) | Builds, caches and renders the full template corpus |
| Release macOS (`v*` tag, or manual dry run) | Every run checks release acceptance; only tag runs sign, notarize, publish and deploy |

CI runs on pull requests, pushes to `master` or `release/*`, nightly at 09:17 UTC
(03:17 Saskatchewan time), and manual dispatch. Feature-branch pushes don't repeat PR
checks. PRs into `master` and master pushes select affected tiers. PRs into `release/*`,
release-branch pushes, nightly and manual runs select **all required tiers**, regardless
of changed paths. Required PR jobs do not install Playwright browsers or run experimental
sync tests. Portable CLI/package suites run once on Linux; their macOS repeat lives in
nightly/manual qualification. Rust runs on both operating systems because SQLite,
locking, sandboxing and the historical macOS writers have platform-specific behavior.
The full release gate, including the app bundle and browser acceptance, remains separate.

`native`, `native-rust` and `linux-smoke` skip at job level when none of their tiers
are affected, so an unrelated change allocates no Mac runner. On a pull request into
`master`, `native` also waits for a change that can affect opening an existing document
(`nativeGateInputs` in `scripts/lib/verification-inputs.ts`: the Rust crates, the Mac app,
the page shell and wire types, the corpus and fixtures, and the native checks themselves).
Compatibility-sensitive inputs additionally open the gate, including SDK/runtime contracts
and the CLI's document/export transport. Other template and authoring-only changes have
their Swift/native tiers recorded as `deferred` in `selection.json` until the master push.
Native test and helper changes run their consumers on the PR. Selection includes both
paths of a rename and deleted files. The main CI workflow, shared preparation action,
selector, verifier implementation and shared toolchain/dependency inputs select all tiers.
Policy workflows and verification tests select tooling; they do not invalidate the product.
Native-cache changes select the Swift/native tiers. Example source changes select authoring/type checks, not infrastructure suites;
example Markdown changes select no product tiers. Test fixture changes select their
consumers, and packaged starter changes still select CLI/browser/packed acceptance.
Other product and build dependencies remain conservatively selected. For example, PR #5's attribution policy changes select
only tooling and types, allocating no macOS runner. Documentation and repository-settings
edits select no product tiers. The compatibility integrity tier runs when corpus files,
its scripts or the core’s acceptance/storage modules change. The selector routes affected
tiers to required jobs and records optional qualification separately in `selection.json`.
Jobs execute exactly their assigned tier names, without consulting the local pass cache.
`fast` always reports
and fails if selection failed or was cancelled; a skipped selector cannot make a PR green.
A successful empty selection reports success without checking out or installing tools in
the fast job.
The required-check policy is `fast`, `native`, `native-rust`, `linux-smoke`, `Gitleaks` and `Attribution`;
activate it only after the corresponding workflows are installed (see
[release rules](guides/releasing.md#github-rules-rollout)). The full
`release:check` runs only in the Release macOS workflow. Reports live in
`.hitslop/evidence/` and are uploaded even on failure. Linux jobs have 30-minute
limits; native has 45 minutes. Pinned binding generators have their own versioned cache,
separate from Cargo artifacts and dependency downloads. CI does not cache successful
verification results or retry failed tests automatically. Job summaries report cache
restoration, setup duration, build/preparation and test duration. GitHub Actions displays
complete job wall times.
Nightly runs have a separate concurrency group, so a master push cannot cancel cache warming.
Successful default-branch runs populate caches that other branches can restore. Pull
requests restore caches but never save them: a cache saved on a PR merge ref serves only
that PR, and saving each one evicted the default branch's caches from the repository's
10 GB budget. Before a push or nightly run saves its Rust cache,
`scripts/ci/prune-cargo-target.sh` drops incremental state and executables. Rust caches separate checks, template builds and release builds; SwiftPM
caches separate fixtures and the release corpus. This prevents a smaller concurrent job
from filling an immutable cache key before the full suite finishes.
Playwright WebKit and Chrome are installed only for nightly/manual qualification and
release verification; required PR jobs do not download browsers.

The attribution workflow covers PRs into `master` and `release/*`, uses
`pull_request_target` and publishes a separate
`Attribution` commit status on the inspected PR head. Its checkout and validator come
from the default branch; incoming Git objects are read without checking out or executing
their files. It has read access to contents and PRs, and write access only to statuses.
New commits, metadata edits and reopened PRs trigger another check. Inspection errors
block merging. The policy catches known assistant identities and generated signatures,
not arbitrary aliases; human co-authors and ordinary AI discussion remain allowed.
Review the final merge message, which a maintainer can edit after validation.

Secret scanning uses the PR merge base or the previous master/release-branch commit through the
checked-out commit. It scans every introduced commit, including a secret later removed
in the same PR and merge-resolution changes. Scanner configuration changes trigger a
full-history scan, even if reverted before the tip. Missing bases fail the check;
new-branch pushes without a previous commit scan full history. CodeRabbit remains advisory.

Validate CI edits with the affected runner tests and workflow checks. When claiming a
performance improvement, compare cold and warm runs of the same source and test
inventory, reporting build, test and job wall time separately. An isolated
`cache_namespace` creates fresh cache keys without disturbing everyday caches.
Nightly/manual template runs build the corpus once and exercise its renders; cache
hit/miss behavior is covered by the template-cache tests.

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

### Packaging and native preview

The two-build Vite fixture covers imported component CSS, CSS fonts/images, unrendered
skins, export-only assets and command stripping. Browser integration uses a native Rust
owner, exercises HMR and command calls, checks attachment URL ranges/types, and kills an
owner to prove the page fences further edits. The local `slop open --browser` host uses the durable WASM build; `slop dev`
continues to use the native owner. The SDK tests use a separate WASM adapter.

Rust package tests cover the seven-table layout, app-row seal and replacement bypasses,
marker-first refusals, streaming PNG checks, descriptor argument validation, the JSON
Schema output projection, resource inventory and first-touch attachment hashes. A SQLite
authorizer proves `summary` never reads definition or asset/attachment payloads. Swift's
page-policy probe proves path-scoped CSP blocks attachment scripts while canvas and
ranges still work. The owner command tests cover stale-base retry and lifecycle fences.

SDK browser tests exercise `EditableText` composition, focus, caret placement and
autosizing with real document handles. Native export tests use controlled content to
check saved-state capture, color, selectable text and editor isolation. App acceptance
launches the actual app with no `HITSLOP_EVALUATOR`, executes the document fixture's
insert, row-argument and remove commands through its live owner, and checks saved
state after reopening. `apple:build` runs this against the raw
Xcode product before adding the standalone renderer; release artifact acceptance runs
it against the packaged app. Test-runner environment overrides cannot mask a missing
bundled evaluator in these checks.
The evaluator prelude and page context dispatch by runtime ABI; corpus replay uses the
stored ABI and original embedded programs.

### Fullscreen qualification

`bun run verify swift --filter fullscreenCapabilityAndLayoutReachTheNativeHost` checks
the declaration's native projection and fitted/reflowing content coordinates. On an
interactive desktop run `HITSLOP_FULLSCREEN=1 bun run verify swift --filter
nativeFullscreenKeepsThePageAndRestoresTheDesktop` to exercise actual macOS Spaces
transitions, the retained page, painted content, display-resolution zoom, toolbar
reachability, and restored frame/pinning for
standard, fixed, shaped and PNG-skinned windows. This explicit test changes the active
Space and must not run alongside other UI qualification. WebKit snapshots do not include
the outer AppKit frame: also inspect a real fullscreen window for background artifacts
and source-image scaling ([qualification evidence](evidence/native-fullscreen-2026-10-08.md)).

## Infrastructure coverage ownership

- Rust owns semantic matrices, refusals, model checks, storage, writer ownership and
  real I/O failures. Replaying literal fixtures through WASM or Swift additionally
  proves those bindings; this is not a reason to repeat the matrix through every UI.
- SDK/shell tests own handles, async publication, composition, recovery and capture
  coordination. Minimal browser fixtures own component interaction and rendering.
- CLI tests own compilation, declaration discovery, assets, command separation and
  protocol translation. Syntax permutations belong to the transform table; full
  builds retain module resolution, registration and emitted-artifact cases.
- Native tests own real window, catalog, WebView, save/close and export integration.
  Controlled native-client doubles test coordination, not document semantics.
- Package/release tests own installed artifacts, evaluator discovery and relocation.
  Generic template smoke checks have no template-specific selectors or actions.
- Frozen compatibility cases retain their original artifacts and expectations.

The removed Hourglass cases asserted illustration/countdown behavior. The removed
Quick Checklist suite mixed its workflow/export wording with four SDK component
contracts; only the latter survive, in the SDK fixture. No example test directory is
accepted by discovery. No test quota is associated with a slop.

Performance samples in browser correctness scenarios are opt-in:
`HITSLOP_BENCH_PREVIEW=1 bun run verify browser large-preview` and
`HITSLOP_BENCH_SYNC=1 bun run verify browser live-sync`. Ordinary runs still check
large-list identity and shared-authority correctness without repeated timing samples.
`HITSLOP_BENCH_COMMANDS=1` enables repeated command-latency samples in `verify browser dev.browser`
and `verify swift --filter pageAndCLICommandsShareTheNativeOwner`; normal runs still
exercise accepted commands, invalid arguments and publication to the page.

The `browser` tier includes the local Chrome host boundary in
`tests/browser/local-host.browser.test.ts`: real WASM commands, OPFS save/reload,
independent origins and document identities, exclusive tab ownership, bounded resource
streams, wake-lock state, container-marker refusal, Save As cancellation and cleanup,
Rust flush-before-export (including save failure), and native reopening. Run it with `bun run verify browser local-host`. It launches installed
Google Chrome with an isolated persistent profile; Safari qualification is deferred.
The Rust tier also compiles both durable-browser and evaluator WASM feature sets.
