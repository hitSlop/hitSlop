# Testing runtime contracts

A shipped slop pins `assets/runtime.json`. Protect its data and host contract across runtime releases. Template business logic belongs to the authored app; new slops need no bespoke platform test code. The approved [test ledger](test-ledger.md) records the cleanup decisions.

## Everyday checks (Bun only)

```sh
bun run check
bun run test
```

`check` verifies generated schemas, runtime provenance, immutable history, skills and TypeScript/Svelte types. Discovered slops with standard settings share one Svelte typecheck program; custom project configurations keep their own checks. Shared dependencies are loaded once. The check needs source and installed dependencies, not a native helper or generated template packages. `test` runs kernel/schema/CLI tests, builds the candidate runtime in a temporary directory, replays every preserved fixture through SQLite, and compiles and opens every bundled template headlessly. No Swift build or PNG rendering runs in this tier.

Kernel tests name their failure modes at the top of each file. Domain rules use literal or independently modeled outcomes. Bindings, handles, atomic batches, failed saves, attachments and convergence are platform contracts; application formulas, decorative CSS and private engine bookkeeping are not.

Each conformance fixture has frozen package bytes, `expected.json` and a scripted `scenario.json` with independent expected results. The runner compares the initial state, edits, flushes or checkpoints, then closes and reopens. The candidate runtime is loaded once per artifact path and reused across sequential fixture cases and template opens. Published byte seals are verified before comparing hashes. Byte-identical releases are listed in each candidate result’s `equivalentReleases`, not counted as independent reader executions. Each distinct historical artifact reads separate copies of the candidate-written bytes in one isolated batch using its own JS and WASM. Runtime paths are immutable during a run; mutation checks use separate disposable artifact directories. Template specimens receive read/reopen coverage; the dedicated conformance corpus supplies semantic mutations.

The launch baseline fixtures (`2-1`, `2-1-saved-state`, `2-1-issues`, `2-1-svelte`; see [their README](../tests/compatibility/README.md)) remain byte-for-byte unchanged. `2-1` and `2-1-svelte` embed frozen ABI consumers that check `ctx` while mounting in WebKit; `2-1-issues` pins exact issues for merged anomalies; `collaboration.json` scripts mixed-version peers that edit, exchange, edit again and reopen until both converge. Fixture seals detect byte changes; state hashes describe canonical logical state, not nondeterministic SQLite/Loro bytes.

`bun scripts/v1/test-sensitivity.ts` deliberately breaks disposable copies of text handles, Unicode splice, preview persistence and collection identity. The designated behavior tests must fail. The normal compatibility tests also corrupt expected state and suppress a compiled handle edit to prove the replay's oracle is sensitive. These tools never mutate production sources or preserved specimens.

## Native boundaries (macOS)

```sh
bun run build
bun run swift:test
bun run test:native
bun run test:native-helper
bun run test:storage
bun scripts/v1/crash-matrix.ts --native
```

`build` generates contracts/runtime resources, builds skills, and compiles the helper; it does not build the template corpus. Native owner runners prepare only Quick Checklist, Small Expenses, and the three presentation fixtures, with black-box packages under `generated/v1/native-fixtures`.

Run this tier when Apple code, the bridge, document/runtime code, compatibility fixtures, template inputs or build scripts change. Swift tests cover envelopes, isolation, ownership, WebKit lifecycle, draft/save/export ordering, render recovery and host interaction. Cancellation observes an acquired lease and subsequent release. Storage faults distinguish rejection before writing from loss of the reply after a successful write. They are scoped to one store bridge.

Native-only Bun tests use `.native.test.ts`; `test:native` runs those owners. `bun run test:render --fixtures` renders the native fixtures and the contract specimens (`tests/compatibility/2-1*`, which run the frozen ABI consumers); everyday CI runs this. The explicit full corpus, run by releases, is:

```sh
bun run build:templates
bun run check:built
bun run test:render
```

The generic native smoke takes disposable copies of every bundled and preserved package, opens headlessly, renders PNG/PDF with the authored app, reopens and checks unchanged state and master bytes. Both local and CI template builds use the validated cache in `.hitslop/template-cache`; `HITSLOP_TEMPLATE_CACHE_DIR` overrides its location. Entries are keyed on the template's own files plus the inputs every build shares: the compiler's import closure, copied `hitslop-document` skill, inherited `tsconfig.v1.json`, document/schema SDK sources, the built runtime, the native renderer sources and the toolchain. CLI routing, help and unrelated skills are excluded. A cache miss rebuilds the complete package and logs the changed inputs; `.hitslop/v1-evidence/template-cache-*.json` records hits and miss causes. Images and PDFs are review evidence, not pixel snapshots. Exports that cannot complete fail the gate.

Render evidence at `.hitslop/v1-evidence/render/results.json` includes per-package and aggregate seconds for `initialRead`, `png`, `pdf`, `finalRead`, and `total`. Each stage includes helper startup and execution; it does not isolate WebKit startup cost. Failed attempted stages retain their timings, and each attempted package records `passed`. Fresh templates and preserved saved documents remain distinct cases even when their app bytes match.

The shared crash matrix runs writer exclusion and process-death cases before/after append and checkpoint commit against both SQLite implementations. `test:native-crash` adds actual host/WebContent death. Startup/window benchmarks remain opt-in diagnostics, without hardware-dependent performance assertions. Bounded waiting remains only where native/WebKit asynchronous completion has no observable completion event.

## Packed consumers and rendering

`packages:pack` produces the npm tarballs. `test:packed` installs them outside the checkout
and checks dependency/type resolution, CLI initialization, source checks, and preview
serving without Node or native rendering. `test:packed --native` additionally exercises
the real packaged build/register/theme/PNG/PDF workflow. Release validation runs the
native mode, which includes the fast checks.

Native captures use the platform PNG encoder directly. Smaller compressed bytes are
not an output contract; dimensions, color, transparency, cancellation and the publication
deadline remain contracts. Keep package/image bounds intact. Run render-heavy suites
sequentially, retain stage timings and failures, and investigate a timeout instead of
adding automatic retries or lengthening deadlines.

## CI and evidence

`fast` always runs on Ubuntu. `native` always reports a check on macOS; an in-job path filter skips its expensive steps when unaffected. Configure branch protection to require both `fast` and `native` after the workflow has reported those check names.

| Tier | Trigger | Runs |
|---|---|---|
| `fast` | PRs, master pushes, manual runs; Ubuntu | hygiene, check, Bun tests, compatibility replay, packed npm packages (`test:packed` fast mode), landing check |
| `native` | relevant changes, macOS | build, Swift tests, native owners, fixture render, helper, storage, native crash matrix |
| `release-templates` | master pushes, or manual `render_corpus`; not required | warms the full template cache and checks sealed authored content; manual profiling also verifies a second build has all hits and renders the complete corpus |
| Release macOS | `macos-v*` tag, or manual dry run | full `release:check` once, then sign, accept, notarize and publish |

The complete template corpus, `check:built` and full render run once per release, not on every push. A tag runs `release:check --skip-app`: instead of building a Debug app, packaging runs host-crash acceptance against the signed Release app before notarization, and Gatekeeper verification runs `release-artifact.ts` on the notarized app. A manual dry run executes the full `release:check`, including the Debug app, without signing or publishing.

Feature branches use PR checks instead of a second push-triggered run. For a full render profile without release packaging, dispatch Runtime contracts with `render_corpus=true`; the `release-templates-evidence` artifact retains first/warm build reports, render timings, PNGs and PDFs. See [the Nx decision](nx-review.md) for the measured reason to retain the existing cache.

GitHub scopes caches by ref: tag runs restore only caches saved on the default branch, which is why master keeps the release template cache warm. Fixture and release template caches use separate keys. SwiftPM caches include toolchain, lockfile, package and source identity, with a compatible restore prefix. SwiftPM still validates the graph after restore.

CI sets `HITSLOP_COMPAT_BASE` to the PR base or previous pushed commit. A tag/manual run uses the prior release/ancestor. The history guard compares previously recorded fixture files and release records against that commit, so changing both a specimen and its seal cannot bless the change. Local checks prefer the previous release tag, falling back to HEAD before the first release. `bun run compatibility:restore` restores missing historical runtime archives and verifies their ledger hashes; missing history is a failure, never a skipped comparison.

Reports live in `.hitslop/v1-evidence/`, uploaded even on CI failure. `release-check.json` records stages, exit status, timing, commit, platform and compatible state hashes. It runs the replay twice and requires identical logical hashes. See [releasing](guides/releasing.md) for explicit fixture sealing and the complete shipping gate.

## Writing and auditing tests

Before writing or changing a test, identify its observable contract, a credible failure, the independent expected result and the gap in existing coverage. This is a reasoning check, not a required report. Extend an existing case table or shared fixture where it already owns the contract. An additional layer must prove a distinct risk, such as native transport or lifecycle failure.

Literal CSS/HTML snapshots, private fields, internal call sequences, copied inventories and mocks that implement the asserted behavior are suspect. Frozen runtime bytes, bridge envelopes, package isolation and save-before-close ordering are real contracts. Source inspection can be the cheapest independent guard when it protects a contractual byte or path and survives identifier-only refactors. A meaningful test failing on the baseline is a possible product bug, not a deletion opportunity.

When a behavior-preserving refactor breaks a test, rewrite it at the owning boundary or delete it if equivalent proof remains. Do not add test-only exports, globals or wrapper layers just to preserve assertions. Narrow per-instance fault injection at a real store, clock or transport boundary can be justified when real failures are impractical to trigger; document the specific risk it exposes. Do not mistake relocating a hook for simplifying the design.

For an audit, inspect the candidate and its production owner before recommending a disposition. Record changed contracts in five columns: **contract, oracle, tier, duplicate-of, verdict**. Link the relevant tests in those cells and group cases sharing a contract. Keep existing approval and validation evidence. Investigate callers, history and support-code consequences in depth only when removal is uncertain or would delete a production seam. Untouched tests need no individual survival essay.

Before removing or replacing consequential coverage, introduce the relevant fault in a disposable copy and verify that the remaining owner test fails for that reason. Run these checks for changed coverage, not every retained test. Application-only or obsolete assertions with no platform requirement need an explicit reason for deletion, not a contrived replacement. Bug regression cases must demonstrably fail before the owner-boundary fix and pass afterward.

Run the smallest owner and sibling tests first, then the affected tier above; inspect the final diff and run `git diff --check`. Retain useful replay/render evidence without making generated output a golden file. Audit findings do not themselves authorize deletion, commits or publication. Keep follow-ups coherent rather than expanding one cleanup into a repository-wide campaign.
