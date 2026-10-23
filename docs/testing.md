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
| Rust storage | `crates/hitslop-core/tests/{store,file}.rs` (feature `storage`) | The file: packing, hostile layouts and rows refused before a value is read, stored values bounded as writes bound them, newer markers, templates never opened as documents, copies never overwriting, the registry lock and discovery, renames and hard links (and saving and reloading once moved back), attachments, artwork written by the writer, ranged asset reads, crash recovery. Saving: storage identity, limits before blob reads, busy and full saves, a failed write never advancing the saved version, snapshots that never write, free-page reclamation, theme overrides, a history that does not start with a snapshot refused. Faults are real and deterministic: another connection holding the database, a moved file, a child process killed mid-commit, after each kind of save and while a save waits, and a save retried after a lost reply |
| File engine | `crates/slop-engine/tests` | `pack` (with the artwork it renders), `inspect` and `schema` as the CLI runs them; a refused build publishes nothing; a closed export that never replaces a file, and `open` refusals |
| Shell over WASM | `packages/hitslop/tests/shell`, `bun run verify bun` | Async write timing, snapshot identity, collectors, bindings, barriers, attachments, the shared fixture replay (`fixtures.test.ts`) |
| Author SDK | `packages/hitslop/tests/sdk` | Descriptor types, cross-bundle errors, framework-neutral helpers and `EditableText` browser behavior |
| Rust owner and commands | `crates/hitslop-core/tests/{owner,command}.rs` | Ordered admission and publications, autosave, edits during slow persistence, failed-close retention, discard fencing, data/theme undo, live socket routing, deadlines and unknown outcomes |
| Swift integration | `apps/apple/Packages/HitSlopApple/Tests`, `bun run verify swift` | Native event delivery, save/reopen, failure UI, CLI live and closed paths, WebView bridge, saved-state capture, window lifecycle |
| Browser | `*.browser.test.ts`, `verify browser` (macOS qualification) | Chrome durable copies and WebKit native-owner preview, without building Swift |
| Sharing service | `scripts/sharing/check-accounts.ts`, `verify sharing` (explicit qualification; a pull request runs it when sharing code changes) | Real Rust WASM with SQLite Durable Objects/R2: verified identities, room creation/open, replaceable uploads, owner recovery, quota release and Stop/upload races; local certificate substitution only |
| Engine renders | `tests/native/{render,landing}.native.test.ts`, `packages/hitslop/tests/cli/build.native.test.ts`, `bun run verify render` | Engine artwork and PNG/PDF exports on macOS and Linux, without Swift |
| Native tools | Remaining `tests/native`, `bun run verify native` | Engine or host killed mid-edit and corpus replay on macOS |
| Packed packages | `tests/packed`, `bun run verify packed` | The published npm tarballs installed outside the checkout without Node: SDK types, init, check, build, preview and the getting-started tutorial |
| App bundle | `bun run verify app` (also in `release:check`) | Builds and checks the complete app, including named commands through the raw Xcode Debug app without an evaluator override; release native tests also exercise host process death |
| Release tooling | `tests/release`, in `verify bun` | Artifact identity, publication recovery and promotion rules |
| Presentation | `tests/presentation`, prepared for Swift/native tiers | Window shapes and capture fixtures |
| Verification runner and CI policy | `tests/verification`, in `verify tooling` | Tier selection, evidence, subprocess lifecycle and attribution; no product build |
| Landing/docs | `apps/landing`, `verify landing` | Public documentation and website checks; independent website workflow builds/deploys |
| Compatibility corpus | Immutable release assets, replayed by the three tiers [below](#compatibility-corpus) | Every released template and saved document still opens, renders, edits and reopens |

The core and runner Rust integration tests each build as one binary, `tests/main.rs` (`autotests = false`),
so a change links once rather than once per file; a new test file is a `mod` there, or it
never compiles. Test names carry their file as a module (`store::…`). `hitslop-runner`'s
`broken_pipe.rs` stays its own binary because it changes the process's SIGPIPE handling.

Capture regressions cover mixed-success batches and unchanged source bytes in Rust, and
speech, media admission, file selection, downloads, resize and close in Swift. Saved-source
capture must preserve live page eligibility; retiring the page cancels transcription.
Runner budget tests accept either the child interrupt or parent watchdog winning, then
verify a fresh job succeeds. The budgets themselves remain fixed by job kind.
The CLI preview test decodes captured PNG pixels with `pngjs`; required PR jobs
need no Playwright browsers. Browser and release qualification install their browsers.

`tests/fixtures/*` are small host apps (`document/`, a build stage, with `expected.json` and
`scenario.json`) replayed by both the Bun fast tier and the Swift host-path test, which
packs each stage and creates a document from it.
`tests/abi/{owner-svelte,probe}` are Svelte and probe consumers used by Swift tests.
`tests/apps` holds authored infrastructure fixtures: a small document app and an
`EditableText` consumer. They are never shipped as starter templates. Test media
belongs to fixtures, not examples or the landing page.

## Running tests

`bun run verify` is the one runner (`scripts/verify.ts`). Without arguments it runs each
tier whose inputs changed since that tier last passed on this machine (recorded in
`.hitslop/verify/`, by content), so a repeated run after a green one does nothing:

```sh
bun run verify                 # the tiers this change touches
bun run verify --native        # the same, with the native (macOS) tiers
bun run verify --all           # all ordinary tiers (add --native for macOS tiers)
bun run verify browser         # optional browser qualification (macOS)
bun run verify sharing         # optional Cloudflare account/room qualification; worker-build required
bun run verify rust store::    # one tier, with its own arguments (here a nextest filter)
bun run verify --list          # what would run, and why
bun run verify --list --json --native --base origin/master # CI selection; no builds/tools
bun run release:check          # release acceptance, shipped builds, and a retained report
```

| Tier | Runs |
|---|---|
| `compat`, `contracts`, `types` | Frozen corpus integrity; generated Rust contracts; Prettier for `packages/hitslop/src` (excluding generated files), Oxlint for `packages/hitslop/src` and `scripts`, TypeScript (the `*.types.ts` type tests in their own project, without the unused-declaration rules), Svelte types for the templates and the SDK's components, and `scripts/ci/hygiene.ts` (no placeholder invitation link in any tracked file, one iPhone download address in the site, README and Swift links, and a landing template catalog that matches the active templates; regenerate it with `bun run build:templates && bun apps/landing/scripts/templates.ts`). Together, concurrently |
| `tooling` | Verification runner and CI policy tests, without Rust, WASM or native builds |
| `bun` | SDK, shell and release tests over SDK WASM; up to four isolated file workers |
| `cli` | CLI integration tests with engine/evaluator and shell; one file worker, 120-second default test deadline |
| `render` | Engine-only artwork and export tests with engine/evaluator and shell, on macOS or Linux; one file worker, 120-second default test deadline |
| `rust` | `cargo fmt --all -- --check`, Clippy with warnings denied (the workspace, plus the WASM adapter on Linux or release runs), then the Rust suite with cargo-nextest, one process per test; a test running two minutes is a named hang. A filtered run (`verify rust store::`) runs only the tests |
| `landing` | The site's type check; `website.yml` builds and deploys separately |
| `packed` | Engine/evaluator, shell, durable-browser WASM and skills, then `tests/packed` (120-second default deadline), when what the npm package ships changes (its sources, starter, skills, page shell or packing) |
| `swift` | Swift formatting, native bindings, shell and fixtures, then one serial `swift test` invocation after compilation, with an isolated temporary directory and registry. Requires at least one executed test. Filtered runs (`verify swift --filter X`) pass Swift’s filters through, including `name\(` patterns |
| `app` | Complete macOS app build and bundle acceptance; selected explicitly or by `release:check` |
| `browser` | `*.browser.test.ts` in Playwright WebKit and Google Chrome, with the Rust engine/evaluator, browser WASM and shell; no Swift build. Runs when named (`verify browser`), with `HITSLOP_NIGHTLY=1 --native`, and during release acceptance; nightly/manual qualification in CI, never a required PR check |
| `sharing` | Builds the browser and Rust Worker, then runs the account/room boundary in pinned Miniflare with SQLite DOs and R2. Requires `worker-build 0.8.7` in `generated/core-tools` (see `apps/sharing/README.md`). Selected explicitly or in a full nightly/release run. A pull request runs it (the `sharing` job) when it changes any of the tier's own inputs (`apps/sharing`, `scripts/sharing`, any crate or Cargo build input) or `packages/hitslop/src/{sharing,browser}`; `sharing.yml` runs it on master pushes and release tags |
| `native` | Remaining native tests with engine/evaluator and shell; no Swift preparation |

Oxlint (`.oxlintrc.json`, `bun run lint`) adds type-aware promise rules to its correctness
rules: a promise must be awaited, returned, handled or `void`-prefixed, and never used as a
condition. It covers the SDK, shell, CLI and scripts. It does not lint Svelte templates, and its
type-aware rules skip `.svelte` files, so `svelte-check` stays the Svelte type check. Tests are
out of scope: Bun's types declare `expect(...).rejects` matchers as returning `void`.

Each tier prepares only its prerequisites: SDK WASM for `bun`, native bindings and
shell for `swift`, engine/evaluator and shell for CLI/render/native, and durable-browser
WASM for browser/package consumers. Actual shared outputs are prepared once per run. A
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
documents. Whenever corpus-consuming tiers run, they replay every published entry with
full coverage. Matching markers or embedded apps does not prove saved scenarios behave
equivalently. Historical-writer randomized differential tests run nightly and for releases.

`HITSLOP_NIGHTLY=1` (set by scheduled/manual CI and hotfix-branch pushes, and implied by
`--release`) adds expensive boundary cases and broader qualification:

| Nightly and release only | Where it is selected |
|---|---|
| Presentation, pixel and telemetry checks: window shapes, toolbar, glass, theme panel, export size and colour, catalog thumbnails, telemetry wiring | Swift `.nightly` trait (`HitSlopTestSupport/Nightly.swift`) |
| Storage-growth and history-trim budgets, a busy display read, acceptance vectors above 1 MiB of decompressed JSON | `default-filter` in `.config/nextest.toml` (`--ignore-default-filter` adds them) |
| Full randomized budgets: model 8 seeds (4 otherwise), merge 40 seeds (8), compat writers 2 seeds (1), publications 100 rounds (10) | `scripts/verify.ts` |
| Large live/closed replies over 48 MiB with near-limit batches | `DocumentOwnerTests.largeDocumentsReadTheSameLiveAndClosed` |
| The type fixtures' fresh native render (their saved documents are replayed every time) | `tests/native/render.native.test.ts` |
| Landing generation, skills install, update notice, help wording, documentation examples, the tutorial build | `test.if(nightly)` in those files |
| The `browser` tier (Playwright WebKit and Chrome) | the tier's `nightly` flag; `verify browser` runs it on demand |

Full compatibility is independent of nightly checks. Every selected corpus-consuming
tier replays all entries; native replay exports every saved document to PNG and PDF.
There is no smoke mode or per-marker sampling. `bun run verify --native` selects the
native tiers whose inputs or corpus inventory changed, without enabling the extra
nightly cases above.

Explicit Rust filters bypass nextest's default exclusions, so
`bun run verify rust a_busy_file_is_reported_busy_not_as_having_no_artwork` runs that test.

A merged failure scenario keeps each failure it replaced named in its comment. Tests that
wait out SQLite's busy timeout run their cases at once on separate documents, so a run
pays that wait once.

Use `bun run verify cli agents.test.ts` for a CLI case. `bun run test` runs `tooling`,
`bun` and `cli`; `verify tooling` runs the build-free infrastructure tests. Discovery rejects
unclassified test files rather than silently leaving them out.

While changing Rust, iterate with `bun run verify rust <filter>` (or `cargo clippy
--workspace --all-targets`). While changing Swift, use `bun run verify swift --filter <name>`.
Formatting is an explicit editing command: `cargo fmt --all` for Rust or
`bun run swift:format` for Swift. It is not a prerequisite for running tests. Run
`bun run verify --native` once at the end when Swift or the FFI surface changed.
`bun run check`, `bun run test`, `bun run core:test`, `bun run swift:test` and
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

`merge.rs` checks merge closure: three replicas of each fixture edit concurrently with every
intent kind, refusals, undo and redo, insert rows from a small pool of IDs, and exchange
whole histories in random order without resetting live undo. Every merge must pass
acceptance, both merge orders must read the same, and no row may read twice. It runs 40
seeds of 120 steps by default (8 on an ordinary `verify`, 400 in the extended run;
`HITSLOP_MERGE_{SEEDS,STEPS}`).
The deterministic creation-race test checks page publications against fresh snapshots
after both imports and later edits, including a replaced row's nested list. Storage tests
check that clean copies discard deleted nested content and close-time trimming bounds
deleted rows with both string and text fields while preserving the live value.

`sync.rs` runs three replicas through the relay's rules (`tests/support/relay.rs`) with
the real session and codec: dropped connections, relay restarts and crashes that reopen
from the last save. Every replica must converge, every page view must equal its document,
nothing saved or held by the relay may be lost, and nothing may pause. It shares the merge
seeds (300 steps each). The golden frames (`tests/sync-frames.json`) are the wire's
contract with the relay; `HITSLOP_WRITE_SYNC_FRAMES=1` rewrites them when the wire changes
on purpose. `owner.rs` syncs two owners through their hooks alone.

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
compiles the engine. `tests/native/crash.native.test.ts` is stress coverage beside the
deterministic Rust storage cases: it kills a real document engine at points spread across
an edit's open, apply, save and close, and checks that the document reopens as it was or
as edited, never torn, and that an acknowledged edit is never lost. With `HITSLOP_APP_BINARY` (a
release builds one) it also kills a running app after an acknowledged CLI edit.
`bun run verify render` runs `tests/native/render.native.test.ts` with the engine and
shell on macOS or Linux; `HITSLOP_RENDER=all`
renders every bundled template (release).

Startup diagnostics: Debug `HITSLOP_STARTUP_TIMINGS=1` logs native preparation, page
readiness (fonts included), presentation and page-relative `hitslop:*` boot marks, without
document values or paths. `HITSLOP_STARTUP_BENCH=1 bun run swift:test --filter documentStartupTimings`
measures fresh documents. For saved ones, use
`HITSLOP_STARTUP_BENCH=1 bun run swift:test -c release --filter savedDocumentStartupTimings`:
it seeds saved edits in a separate engine process, then reopens the document fixture, a
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

Other performance diagnostics:
`HITSLOP_BENCH=1 HITSLOP_BENCH_ROWS=1000,5000 HITSLOP_BENCH_WINDOWS=1 bun run bench:windows`.
`HITSLOP_BENCH_CSS` appends CSS to the measured checklist and
`HITSLOP_BENCH_LABEL` names the report, for attributing a cost to one rule; the
`row_text_split_ms` field separates style from layout. Record a result under
`docs/evidence/` when a doc cites it. Superseded runs may be kept locally under
`archive/docs/evidence/`; remove or replace their tracked citations when untracking them.

`bun run bench:growth` runs the checklist conformance fixture through the native owner
for 30 simulated days by default (1–365 via `HITSLOP_GROWTH_DAYS`). Each day inserts
100 synthetic strokes, with 2,048 points serialized into each row's text, and removes
rows in groups of 25; each insert and removal batch is flushed. Two equal workloads
compare daily close/reopen with one continuously open owner. The Swift runner prepares
the infrastructure fixture; restoring archived examples is unnecessary.

The report records checkpoint/update bytes, row counts, flush time, generated geometry
bytes and exact saved-value checks. It makes no lifetime projection. Inspect each
result's `completedDays` and optional `stoppedError`; failures also fail the test.
Temporary documents are removed when the harness finishes.

For a smoke run, use `HITSLOP_GROWTH_DAYS=1` and set `HITSLOP_GROWTH_OUTPUT` to a temporary
JSON path. Without an override, the launcher writes
`docs/evidence/document-growth-<today>.json`. A short run does not establish a year's
capacity; these synthetic workloads are not forecasts of individual users' behavior.

## Compatibility corpus

The required compatibility checks answer: can this build open a saved slop, run its
embedded app, edit it, save it, and reopen the result without losing data or attachments?
Native page replay uses the actual WKWebView host, not Playwright. Keep these checks in
the required Rust, Swift and native tiers whenever their inputs change.

Capture a separate frozen collection for **each released version**, starting with v1.0.0.
Its corpus hash is permanently pinned in `scripts/compat/required-history.json`; missing
or changed public history fails even on a fresh machine. There is no prelaunch fallback.
Later builds replay every collection; never
rebuild old samples with current tools. These artifacts are immutable GitHub release assets, downloaded and verified for testing. The format markers can remain at 1 across
many releases; release versions and format requirements are different things.

Each corpus stores the original compiled programs as well as their saved files: built
templates, saved documents (with their attachments), expected state, page scenarios,
and writer metadata `{version, platform, sha256, buildId}` in `release.json`. The
darwin-arm64 writer is the engine shipped in that release's npm package; engine binaries
are not committed in the corpus. Before historical-writer replay, `scripts/compat/writers.ts` resolves
it into `~/.cache/hitslop/compat-writers/<sha256>` (`HITSLOP_COMPAT_WRITERS` overrides the
cache). A local `HITSLOP_ENGINE` or checkout engine must match both build ID and SHA-256;
otherwise the resolver extracts it from the exact npm tarball attached to the immutable
GitHub release and verifies the tarball and engine hashes.
Cached bytes are checked again; a missing or corrupt writer of a published release fails
replay. Every historical writer must resolve from its exact local bytes or its retained
npm package; unavailable writers are never skipped on darwin-arm64.

Documents are written both ways a release writes them:
through its CLI, and by its own app's page (`NAME.page.slop`, saved by the page
scenario, so text splices against older versions and page-minted IDs are replayed too). It
covers the runtime, not individual slops: the conformance app (every `ctx` member and
descriptor kind, stored commands covering every intent emitted by the prelude,
all 26 app asset types and all 16 attachment types, dedicated Preview/Icon/Export/Widget
views, theme variants and independent content assertions) with its saved, page-saved
and compacted documents, and a template for each window
kind (1× and 2× PNG skins, glass, a transparent ellipse and a path shape), which later
builds create, edit and render documents from. Shipped templates are not in it; the
template render checks cover what ships now. It is regression evidence, not proof of all
possible authored apps.

The candidate workflow runs `bun run compat:capture VERSION --writer generated/engines/darwin-arm64/slop-engine --out dist/corpus`. It builds the producing tools, records their
commit and core/writer identities, and captures outside Git. It records
template file digests, each template and saved document's producing-reader `appDigest`,
per-file hashes and the required case inventory. Replay pins app identity independently
of file hashes through edit/save/close/reopen.
The entry also records
`limits` (each opening/reading/saving bound and its direction), raw stored app rows in
`acceptance/*.json`, and the producing reader's typed re-encoding of those rows. The
row payloads are Brotli-compressed JSON encoded as base64 within each vector file;
replay checks the decoded rows, preserving full byte-bound cases within a 3 MiB
aggregate vector budget. Vectors above 1 MiB of decompressed JSON run only in
nightly/release checks (`large_acceptance_vectors_`); ordinary runs retain small
vectors and the limits ratchet, and capture self-checks every vector. Every entry
must carry these records. Vectors cover
open readers with future optional members, window kinds, widgets, capability spellings,
and boundary-sized metadata, definitions, descriptors, themes and commands. Resource and
saved-state size/count bounds are pinned by the limits table; raw app rows do not contain
resource blobs or saved Loro state. A later build may only raise a maximum, lower a
minimum, or retain an exact bound. A recorded limit may disappear only through the
explicit retired-limit list. The limits-module completeness test requires every constant
to be recorded or explicitly classified as not persisted. The physical `storage-1.sql`
definition remains hash-pinned; format-reader source is free to refactor and authoring
rules are free to tighten while recorded acceptance remains unchanged. Only a completed
capture is published and frozen. Capture requires a release version, a candidate writer
and a new output directory; an existing destination is refused.

- Rust (`crates/hitslop-core/tests/compat.rs`, in every `verify rust`) replays saved
  values, themes, app digests, edits, save/close and reopen. It also checks every entry’s
  limits ratchet
  and decodes its raw acceptance vectors, comparing typed re-encoding semantically with
  the producing reader’s known members; later readers may expose additional optional
  members that were unknown when captured. ResourceReader checks every recorded app
  asset and attachment for bytes, media type, ranges and security headers, including
  retained attachments after edit/save/close/reopen. Stored command probes run in the
  current evaluator with the frozen prelude, clock and seed, then cross owner admission
  and saving; invalid arguments must never reach the evaluator.
- Rust (`compat_writers.rs`) also replays every release's writer, not only its sample: each
  entry's candidate writer creates documents from the entry's templates and applies
  generated batches in its own protocol, and this core must read exactly what that engine
  reads, then edit, save, trim and reopen them. This differential replay runs only on
  darwin-arm64 and prints a visible skip elsewhere. Requests in older protocols live only in
  this test. Historical-writer differential tests run nightly and for releases, or when
  explicitly selected; ordinary PRs keep deterministic corpus replay. The weekly `Core model` run uses
  16 × 40 (`HITSLOP_COMPAT_SEEDS`, `HITSLOP_COMPAT_STEPS`).
- Native replay (`tests/native/compat-replay.native.test.ts`) checks original app
  rendering, PNG/PDF content, attachments, theme changes/reset, stored
  Preview/Icon/Widget programs and template creation, through this build's CLI
  (the `compat` tier checks inventory and hashes). It exports every saved document,
  including different values saved with the same app. Stored command programs replay
  twice: deterministically through the evaluator with their recorded clock and seed,
  and through the owner with `slop call` (argument refusal first, then an edit).
  Release verification explicitly requires its frozen candidate entry before replay.
- Swift (`CompatCorpusTests`) runs the old conformance app's own scenario, which includes a
  page edit and a live agent command, requires an actual saved edit and checks reopen.
  With producing tools already built, run it with
  `bun scripts/verify.ts --no-build swift --filter CompatCorpusTests`.

Each storage-version bump adds two Rust tests: a frozen file migrated by a write equals a
newly created file in exact layout and value, and an interrupted migration leaves the
older file intact.

The `compat` tier verifies every resolved corpus's content hashes, immutable storage SQL,
required case inventory and versioned command prelude. `scripts/compat/corpora.ts` discovers
all public immutable releases, checks history closure, and verifies each archive.
It extracts once into `~/.cache/hitslop/compat-corpora/trees/<sha>/`, verifies extracted
files again on reuse and makes them read-only, then atomically activates a snapshot of version links at
`.hitslop/compat/current`. Candidate entries live in a separate snapshot. Editing tests explicitly make only
their disposable document copies writable; templates are read directly from the cache. Preparation
is serialized and a delayed discovery response cannot regress remembered history.

`bun run verify` prepares history automatically. Before plain `cargo nextest`, Xcode
or standalone native tests, run `bun run compat:resolve`; these readers resolve the
`current` pointer once for their process. They fail with preparation instructions if
it is missing. Raw tests use the last prepared inventory, so prepare again after new
releases. Verifier and capture subprocesses receive a fixed `HITSLOP_COMPAT_ROOT` to
keep a run isolated from other preparations. Historical writer tests additionally
need `bun scripts/compat/writers.ts` when run outside the verifier.

CI requires fresh inventory; explicit local `HITSLOP_COMPAT_OFFLINE=1` replay is visibly
non-certifying. The corpus inventory forms part of verification pass digests, so a new
public release invalidates relevant passes. Listings and filtered Rust tests that
exclude compatibility do not resolve history or writers. Cache corruption is an error;
remove the named cache entry and resolve again without changing published evidence.

Release checks additionally require the candidate corpus to be captured and frozen, its
commit to be the workflow commit, and its writer to match the darwin-arm64 engine in the
retained npm tarball byte for byte. The candidate's signed-app smoke opens each historical
saved document in the actual packaged owner, checks recorded call probes including refusals,
then edits/saves/reopens it. Swift remains responsible for the old compiled page's behavior.
Historical entries never need current build identities. CLI protocol versions evolve with
the current host; old file reads use today's protocol, and mismatches must return exit 2
without changing file bytes. See [releasing](guides/releasing.md).

## CI

| Job | Runs |
|---|---|
| `select` (Ubuntu 24.04) | Selects affected tiers without installing dependencies or compiling; records the selection |
| `fast` (Ubuntu 24.04) | Affected compatibility integrity, tooling, generated-contract, type, Bun, CLI, engine-render, installed-package and landing checks |
| `native` (macOS 15 ARM64) | Affected `swift,native` tiers: native page edits, the CLI, and the app-level old-file replay |
| `app` (macOS 15 ARM64) | Compiles the unsigned Debug macOS app, its extensions and embedded starter templates (`bun run apple:build --compile-only`); selected by app build inputs and always on nightly/manual/hotfix runs; required, and a skipped run satisfies the requirement |
| `native-rust` (macOS 15 ARM64) | The affected `rust` tier, in parallel with `native`: platform SQLite, Darwin sandbox and deterministic historical-file replay. WASM lints run on Linux only |
| `linux-smoke` (Ubuntu 24.04) | When Rust inputs change: full workspace tests/lints, WASM lint, no-storage configuration and bundled-SQLite engine coverage |
| `Gitleaks` (Ubuntu) | Introduced commits on PRs/master; full history weekly, manually, or when scanner rules change |
| `Attribution` (Ubuntu) | Every incoming commit's identities and attribution lines, plus PR title/description; trusted default-branch policy, including fork PRs |
| `sharing` (macOS 15 ARM64) | `verify sharing` on a pull request that changes the relay, any Rust crate or Cargo build input, or the sharing and browser hosts (Miniflare qualification is proven on macOS only); master pushes and release tags run it in `sharing.yml` |
| `qualification` (nightly/manual) | Playwright browser integration and the macOS portable CLI/package matrix; not required for merging |
| `release-templates` (Ubuntu, nightly/manual) | Builds, caches and renders the full template corpus with the engine and shell, without Swift/Xcode or WASM tools |
| Release (`workflow_dispatch`) | Candidate builds/tests/signs/notarizes once; environment approval publishes the retained app/npm/corpus bytes; website deployment is independent |

CI runs on pull requests, pushes to `master` or `hotfix/*`, nightly at 09:17 UTC
(03:17 Saskatchewan time), and manual dispatch. Feature-branch pushes don't repeat PR
checks. PRs into `master` and master pushes select affected tiers. PRs into `hotfix/*`,
hotfix-branch pushes, nightly and manual runs select **all required tiers**, regardless
of changed paths. Required PR jobs do not install Playwright browsers. Portable CLI/package suites run once on Linux; their macOS repeat lives in
nightly/manual qualification. Rust runs on both operating systems because SQLite,
locking, sandboxing and the historical macOS writers have platform-specific behavior.
The full release gate, including the app bundle and browser acceptance, remains separate.

`app`, `native`, `native-rust` and `linux-smoke` skip at job level when none of their tiers
are affected, so an unrelated change allocates no Mac runner. On a pull request into
`master`, `native` also waits for a change that can affect opening an existing document
(`nativeGateInputs` in `scripts/lib/verification-inputs.ts`: the Rust crates, the Mac app,
the page shell and wire types, the corpus and fixtures, and the native checks themselves).
Compatibility-sensitive inputs additionally open the gate, including SDK/runtime contracts
and the CLI's document/export transport. Other template and authoring-only changes have
their Swift/native tiers recorded as `deferred` in `selection.json` until the master push.
Changes to the native tests and their support scripts run their consumers on the PR.
Selection includes both paths of a rename and deleted files. The main CI workflow, shared preparation action,
selector, verifier implementation and shared toolchain/dependency inputs select all tiers.
Policy workflows and verification tests select tooling; they do not invalidate the product.
Native-cache changes select the render/Swift/native tiers and app build. Example source changes select authoring/type checks and app packaging;
example Markdown changes select no product tiers. Test fixture changes select their
consumers, and packaged starter changes still select CLI/browser/packed acceptance.
Other product and build dependencies remain conservatively selected. For example, PR #5's attribution policy changes select
only tooling and types, allocating no macOS runner. Documentation and repository-settings
edits select no product tiers. The compatibility integrity tier runs when corpus files,
its scripts or the core’s acceptance/storage modules change. The selector routes affected
tiers to required jobs and records optional qualification separately in `selection.json`.
Tier jobs execute exactly their assigned tier names, without consulting the local pass cache.
`fast` always reports
and fails if selection failed or was cancelled; a skipped selector cannot make a PR green.
A successful empty selection reports success without checking out or installing tools in
the fast job.
The required-check policy is `fast`, `native`, `native-rust`, `linux-smoke`, `Gitleaks` and `Attribution`;
activate it only after the corresponding workflows are installed (see
[release rules](guides/releasing.md#github-rules-rollout)). The full
`release:check` runs only in the Release macOS workflow. Reports live in
`.hitslop/evidence/` and are uploaded even on failure. Linux jobs have 30-minute
limits; native has 45 minutes. UniFFI-only and WASM/full generator sets have separate versioned caches, with pins
from `scripts/build/tool-versions.json`,
separate from Cargo artifacts and dependency downloads. CI does not cache successful
verification results or retry failed tests automatically. Job summaries report cache
restoration, setup duration, build/preparation and test duration. GitHub Actions displays
complete job wall times.
Nightly runs have a separate concurrency group, so a master push cannot cancel cache warming.
Successful default-branch runs populate caches that other branches can restore. Pull
requests restore caches but never save them: a cache saved on a PR merge ref serves only
that PR, and saving each one evicted the default branch's caches from the repository's
10 GB budget. Before a push or nightly run saves its Rust cache,
`scripts/ci/prune-cargo-target.sh` drops incremental state and executables. Rust caches separate checks, template builds and release builds; SwiftPM caches
cover native fixtures only. Template caches fingerprint Rust and renderer inputs,
without Swift or Xcode. This prevents a smaller concurrent job
from filling an immutable cache key before the full suite finishes.
Playwright WebKit and Chrome are installed only for nightly/manual qualification and
release verification; required PR jobs do not download browsers.

The attribution workflow covers PRs into `master` and `hotfix/*`, uses
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

Rust package tests cover the exact table layout, app-row seal and replacement bypasses,
marker-first refusals, streaming PNG checks, descriptor argument validation, the JSON
Schema output projection, resource inventory and first-touch attachment hashes. A SQLite
authorizer proves `summary` never reads definition or asset/attachment payloads. Swift's
page-policy probe proves path-scoped CSP blocks attachment scripts while canvas and
ranges still work. The owner command tests cover stale-base retry and lifecycle fences.

SDK browser tests exercise `EditableText` composition, focus, caret placement and
autosizing with real document handles. Native export tests use controlled content to
check saved-state capture, color, selectable text and editor isolation. App acceptance
launches the actual app with no `HITSLOP_EVALUATOR`, executes the document fixture's
insert, row-ID and remove commands through its live owner, and checks saved
state after reopening. `apple:build` runs this against the raw
Xcode product before copying it to `generated/app`; release artifact acceptance runs
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
- SDK/shell tests own handles, async publication, composition and recovery. Minimal
  browser fixtures own component interaction and rendering.
- CLI tests own compilation, declaration discovery, assets, command separation and
  protocol translation. Syntax permutations belong to the transform table; full
  builds retain module resolution, registration and emitted-artifact cases.
- Native tests own real window, catalog, WebView, save/close and export integration.
  Controlled native-client doubles test coordination, not document semantics.
- Package/release tests own installed artifacts, evaluator discovery and relocation.
  Generic template smoke checks have no template-specific selectors or actions.
- Frozen compatibility cases retain their original artifacts and expectations.

No example test directory is accepted by discovery. Shared component contracts live
in the SDK fixture; no test quota is associated with a slop.

Performance samples in browser correctness scenarios are opt-in:
`HITSLOP_BENCH_PREVIEW=1 bun run verify browser large-preview`. Ordinary runs still check
large-list identity without repeated timing samples.
`HITSLOP_BENCH_EDITS=1` enables repeated page-edit latency samples in
`verify browser dev.browser`. Native `cliCommandsPublishToTheNativeWindow` covers
agent command acceptance, refusal, publication and undo/redo.

The `browser` tier includes the local Chrome host boundary in
`tests/browser/local-host.browser.test.ts`: real WASM edits, OPFS save/reload,
independent origins and document identities, exclusive tab ownership, bounded resource
streams, wake-lock state, container-marker refusal, Save As cancellation and cleanup,
Rust flush-before-export (including save failure), and native reopening. Run it with `bun run verify browser local-host`. It launches installed
Google Chrome with an isolated persistent profile; Safari qualification is deferred.
The Rust tier also compiles the durable-browser WASM feature set.
