# Testing

Tests live at the boundary that owns the behavior. Nothing has shipped, so there are no
compatibility or sealed-artifact suites; delete a test together with the code it
protects, and never bend production code to keep an old test compiling.

| Boundary | Where | Proves |
|---|---|---|
| Rust semantics | `crates/hitslop-core/tests`, `cargo test --locked --workspace` | Descriptors, validation, atomic batches, row identity, publications equal a fresh snapshot, counters, text merges, byte export/import, FFI panic containment |
| SDK over WASM | `packages/document/tests`, `bun run test` | Async write timing, snapshot identity, collectors, bindings, barriers, attachments, the shared fixture replay (`fixtures.test.ts`) |
| Swift integration | `apps/apple/Packages/HitSlopApple/Tests`, `bun run swift:test`, `bun run test:native` | Writer lock, save/reopen, failed-save retention, lost-reply recovery, CLI live and closed paths, WebView bridge, export, window lifecycle |

`tests/fixtures/*` are small host packages (`document/`, `expected.json`,
`scenario.json`) replayed by both the Bun fast tier and the Swift host-path test.
`tests/abi/{owner-svelte,probe}` are Svelte and probe consumers used by Swift tests.

## Everyday checks

```sh
bun run check   # generated contracts, skills, TypeScript/Svelte types
bun run test    # Bun SDK/CLI/schema tests over the WASM core
cargo test --locked --workspace
```

## Native (macOS)

```sh
bun run build
bun run swift:test
bun run test:native
bun run test:native-helper
bun scripts/v1/crash-matrix.ts           # add --host for host death (test:native-crash)
bun run test:render --fixtures
```

`build` generates contracts, builds the Rust bindings and the page shell, and compiles
the helper. The crash matrix pauses a real native write at each storage phase, kills it
and checks writer exclusion and old-or-new recovery through `hitslop-native get`.

Performance diagnostics are opt-in and not CI gates:
`HITSLOP_BENCH=1 HITSLOP_BENCH_ROWS=1000,5000 HITSLOP_BENCH_WINDOWS=1 bun run bench:windows`,
`scripts/v1/bench-webkit.ts` (Playwright WebKit) and `scripts/v1/bench-wkwebview.swift`
(plain system WebKit). Record results under `docs/evidence/`.

## CI

| Tier | Runs |
|---|---|
| `fast` (Ubuntu) | hygiene, check, Rust tests, Bun tests, packed npm packages, landing check |
| `native` (macOS, path-filtered) | build, Swift tests, native CLI tests, fixture render, helper, crash matrix |
| `release-templates` (master) | builds and caches the full template corpus |
| Release macOS (`macos-v*` tag) | `release:check`, sign, notarize, publish |

Reports live in `.hitslop/v1-evidence/` and are uploaded even on failure.

## Writing tests

- Name the observable failure, an independent expected result and the gap in existing
  coverage before adding a test. Extend the owning boundary's existing case table first.
- Private fields, internal call sequences, literal CSS/HTML and mocks that implement the
  asserted behavior are not contracts. Bridge envelopes, storage durability and
  save-before-close ordering are.
- A bug regression test must fail on the old code for the intended reason, then pass.
- Keep fault injection narrow and at real I/O boundaries (for example
  `Storage.testingPhase`). Prefer observable completion over sleeps.
