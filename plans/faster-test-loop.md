# Faster test loop: macOS-first, nextest, one dependency graph, test less often

> **Done, superseded (2026-10-05).** Implemented with `bun run verify`; results and what
> was measured and not adopted (the single integration binary, incremental release builds)
> are in [the evidence](../docs/evidence/test-loop-2026-10-05.md), and the runner in
> [testing](../docs/testing.md#running-tests). Kept for its measurements.

## Context

Test runs make refactors slow. Session transcripts for the last 20 sessions show agents
ran `cargo test` 202 times (5.3 h of waiting). Full `--workspace` runs (38) had a
**median of 280 s** and several hit the 10-minute tool timeout. Including swift test ×99,
`bun run check` ×72, `bun run test` ×42 and `bun run build` ×37, agents spent about 9.6 h
waiting on tests and builds. The problem is cost per run multiplied by frequency, so this
plan cuts both. macOS is the priority: local builds are Mac arm64 only, and CI keeps a small
Linux smoke check. Nx and cargo-hakari are not needed. Cargo already caches compiles, and the
time goes to the things listed below.

## Where the time goes (measured 2026-10-05, M1 8-core; scratch copy, other agents active)

1. **macOS checks each new executable on its first launch: 5–19 s per binary.** Same
   binary, first vs second launch: `conformance` 16.5 s vs 0.11 s of tests; `collections`
   19.1 s vs 0.06 s; the core's `build.rs` 13.2 s vs 0.026 s (it reruns on every core edit,
   once per profile). Each core edit relinks ~25 executables. Developer mode is disabled,
   and Warp is not a Developer Tool.
2. **Cargo runs the 22 test binaries serially.** Test time sums to about 44 s. The
   largest are `store` 20 s, `publications` 9 s and `file` 6 s. In parallel, the floor
   is CPU saturation plus real SQLite busy timeouts (2 s and 5 s): about 15–19 s.
3. **Different invocations rebuild Loro and ~40 deps** (`cargo test -p hitslop-core` after
   a workspace build: 41 crates, 242 s; it also drops `storage`, which skips the storage
   tests). `cargo tree` shows two incidental leaks. uniffi's default `cargo-metadata`
   (only its bindgen CLI needs it) flips `serde_json`/`serde_core`/`semver`. The wasm
   crate's `wasm-bindgen`, compiled for the host though it has no tests, flips `syn/visit`,
   which changes every proc-macro and therefore Loro. In a scratch copy, fixing both makes
   all host graphs feature-identical.
4. **Release compiles of the core repeat.** A release core rebuild costs 22.9 s (engine) and
   8.5 s (wasm). `bun run build` compiles it in 4 graphs/dirs: host `--features cli`,
   `--target aarch64-apple-darwin`, `x86_64-apple-darwin` and the engine. The Xcode embed
   adds a fifth (`--target aarch64`). The release app is arm64-only:
   `macos-release.yml:104-106` asserts `lipo -archs = arm64` for the app, helper and engine.
5. **CI runs the full Rust suite in every job**, because `.github/actions/prepare-checks`
   ends with `bun run core:test`. The main tier is Ubuntu, but the app links macOS SQLite.
6. `target/` is 49 GB with 296,519 files in `target/debug/deps`. Clippy is not in the
   pinned toolchain (`profile = "minimal"`), so agents' clippy calls fail instantly.

## Spike: nextest × one integration binary (wall seconds, after a one-line core edit / repeat)

| Layout | After edit | Repeat (no rebuild) |
|---|---|---|
| Today: 26 binaries, `cargo test` | 78.0 | 43.7 |
| 26 binaries, nextest | 58.1 (build 24.8, run 20.4) | 16.0 |
| One integration binary, `cargo test` | 33.2 (build 8.9) | 40.0 (libtest long tail: 19–32 s) |
| One integration binary, nextest | 57.0 (build 16.3, run 20.3) | 19.4 |

Nextest roughly halves or better the run time. The merged binary halves link time. The
remaining after-edit gap (~15–20 s) is nextest listing new binaries, and each pays the macOS
first-launch check. The Developer Tools exemption (step 0) removes that.
Projected after all steps: about 25–35 s after an edit and 15–20 s on repeat, against a
280 s median in real sessions today.

- **Flake check** (merged layout, 5× each runner, load average 17–31 on 8 cores from other
  agents): zero unexpected failures, with no SLOW or TIMEOUT. Nextest ran a steady
  17.5–22.4 s. `cargo test` swung between 18.3 and 47.2 s because the engine tests launch
  `slop-engine`, and macOS re-checks it per launching context: the same unchanged binary took
  10.8 s on its first launch from a shell after dozens of launches by the tests, then 12 ms.
  Full parallelism is safe, and nextest is the predictable runner.
- **Incremental release (`CARGO_INCREMENTAL=1`) after a core edit:** wasm 8.1 s → 1.4 s;
  engine 11.0 s → 10.4 s (mostly linking plus the build-script launch check).
- **Load:** three agent sessions are running on this machine (load average 31 over
  15 min), which also explains part of the 280 s median.

## Plan

### 0. Machine (you, once)
- `sudo spctl developer-mode enable-terminal`, then System Settings › Privacy & Security ›
  Developer Tools: add **Warp** (and any editor whose terminal runs builds) and enable
  it, then restart Warp. Processes started there skip the first-launch check.
- After step 2 lands: `cargo clean` (49 GB, feature variants that no longer occur).
- Run one heavy suite at a time. Concurrent agent sessions share `target/` (cargo's
  build lock) and 8 cores.

### 1. One dependency graph for every host invocation
- `crates/hitslop-core-ffi/Cargo.toml`: `uniffi = { version = "=0.31.1", default-features = false }`;
  delete `[[bin]] uniffi-bindgen`, the `cli` feature and `src/bindgen.rs` (32 lockfile
  entries leave).
- Install the generator like wasm-bindgen: `cargo install uniffi --version 0.31.1
  --features cli --locked --root generated/core-tools` (uniffi 0.31.1 publishes its
  `Cargo.lock`, so it is reproducible). `scripts/core-build.ts` resolves
  `generated/core-tools/bin/uniffi-bindgen` with the same version check as `buildCoreWasm`.
  `.github/actions/prepare-checks` installs it beside wasm-bindgen-cli, and
  `crates/README.md` names both installs.
- `crates/hitslop-core-wasm/Cargo.toml`: move `wasm-bindgen`/`js-sys` under
  `[target.'cfg(target_arch = "wasm32")'.dependencies]`; add `#![cfg(target_arch = "wasm32")]`
  to its `src/lib.rs`; `[lib] test = false, doctest = false`; depend on the core with
  `default-features = false`.
- `crates/hitslop-core/Cargo.toml`: `default = ["storage"]`, so `cargo test -p hitslop-core`
  runs the storage tests and shares the graph. `[lib] doctest = false` (no doctests exist),
  also on `hitslop-core-ffi`. CI checks the WASM-shaped core with `--no-default-features`.
- Verified in scratch: `--workspace`, `-p hitslop-core --features storage`,
  `-p slop-engine`, `-p hitslop-core-ffi` and engine+FFI all have identical features.

### 2. nextest and one integration binary
- `git mv crates/hitslop-core/tests/*.rs` into `tests/integration/` and `support/mod.rs` to
  `tests/integration/support.rs`. `main.rs` declares the modules, with
  `#[cfg(feature = "storage")]` on `file`, `store`, `compat` and `command` (the others
  already carry inner `#![cfg]`). In each module, `mod support;` becomes
  `use crate::support;`; fixture includes become `../../fixtures/…`; `support::child`
  callers use qualified names (`file::crash_mid_commit`, `file::exclusive_sqlite_writer`,
  `store::save_then_die`, `store::save_while_held`). Delete the four `[[test]]
  required-features` entries (keep the `[[example]]`). Include the new `trimmed.rs`
  (added since the spike copy). Spike: this compiled unchanged and ran all 164 tests.
- `.config/nextest.toml`: `[profile.default] slow-timeout = { period = "20s",
  terminate-after = 6 }`, so a hang is a named TIMEOUT after 2 min instead of a 10-minute tool
  timeout. Add `[profile.ci] fail-fast = false`. No test-group overrides are needed: the
  flake check was clean.
- `package.json`: `core:test` = `cargo nextest run --locked --workspace` (positional
  filters pass through: `bun run core:test store::`); `core:test:extended` =
  `… cargo nextest run --locked --release -p hitslop-core -E 'test(/^model::/)'`.
- Install: `brew install cargo-nextest` locally. In CI, use `taiki-e/install-action`
  pinned by SHA with `cargo-nextest@<latest, verified on the registry>`.

### 3. Mac-only release artifacts, one release compile per edit
- `scripts/core-build.ts::buildCoreNative`: drop `x86_64-apple-darwin`, `lipo` and the
  `--target` builds. Use one `cargo build --locked --release -p slop-engine -p hitslop-core-ffi`
  (host `target/release`, the same graph `buildEngine` uses after step 1). Make the XCFramework
  from `target/release/libhitslop_core_ffi.a`. Refuse a non-arm64 host with a clear
  message. `buildEngine` builds the same two packages, so `bun run test` and `bun run build`
  share one compile.
- `scripts/embed-hitslop-native.sh`: for the host architecture, build without `--target`
  into `target/release`, sharing that compile. Keep `--target` only for a non-host arch.
- Set `CARGO_INCREMENTAL=1` in core-build's env when `CI` is unset: the local wasm rebuild
  drops from 8.1 s to 1.4 s. Shipped builds come from CI and stay non-incremental.

### 4. CI: macOS main, Linux smoke (public repo, so macOS minutes are free)
- `prepare-checks`: remove `core:build:wasm` and `core:test`. It installs tools only.
- `fast` → `macos-15`: hygiene, check, `bun run core:test` (nextest, `--profile ci`),
  `bun run test`, pack/test:packed, landing. Drop its two `cargo check` lines.
- New `linux-smoke` (ubuntu-latest): `cargo check --locked -p hitslop-core
  --no-default-features --tests` (the WASM-shaped core) and `cargo nextest run --locked
  -p slop-engine --features bundled-sqlite` (the CLI's engine builds and works on Linux).
- `native` and `release-templates` no longer re-run the Rust suite. `model.yml` keeps its
  extended run (install nextest). Branch protection: require `fast`, `native` and
  `linux-smoke`.

### 5. Test less often: when to run what (AGENTS.md "Testing" + docs/testing.md)
- While changing Rust: `cargo check --workspace --tests`, then `bun run core:test <filter>`
  for the module touched.
- Before calling a step done: `bun run core:test`. Add `bun run check && bun run test`
  only when TypeScript, schema, shell or WASM-facing code changed.
- Native (`bun run build && bun run swift:test && bun run test:native`) only when Swift,
  the FFI surface or the helper changed, and once at the end of a refactor, not per step.
- Never `--all-features` (it compiles bundled SQLite: release-only). Clippy is not part
  of the checks.
- docs/testing.md: update the paths (`tests/integration/{store,file,owner,command,model}.rs`),
  the runner (nextest), the macOS Developer Tools note and the CI table. Update the
  comments that cite old paths in `SlopFileTests.swift`, `AttachmentTests.swift` and
  `packages/schema/tests/manifest.test.ts`.

## Not doing
Nx/Turborepo (task caching doesn't address any of the measured costs), cargo-hakari (step 1
makes the graphs identical without it), sccache/mold (one machine; ld-prime is already
fast), and splitting long tests (the parallel run is CPU-bound). Swift tests run with
`--no-parallel`. That is the next candidate, but it is out of scope here.

## Verification
- `cargo tree` feature diff (the scratch script) shows no differences across `--workspace`,
  `-p hitslop-core`, `-p slop-engine` and `-p hitslop-core-ffi`. After `bun run core:test`,
  `cargo test -p hitslop-core` compiles no dependency.
- Parity: nextest runs the same test count as today's `cargo test` (180 + `trimmed`). All
  pass in the real repo, and the four child-process tests run their children (their
  parents assert on the child's exit).
- Timing protocol (edit `publication.rs`, then `bun run core:test`; then repeat):
  ≤ 35 s / ≤ 20 s, against 78 s / 44 s today. A first launch of a fresh `rustc` hello binary
  takes about 10 ms with Developer Tools on, against about 300 ms off.
- `bun run core:test` 10× with no failures. `bun run test`, `bun run build`,
  `bun run swift:test` and `bun run test:native` pass with the arm64-only XCFramework.
  `bun run build` after a core edit compiles `hitslop-core` twice (wasm plus one native),
  not five times.
- CI: `fast` and `native` on macOS and `linux-smoke` on Ubuntu are green, and no other job
  runs the Rust suite.
