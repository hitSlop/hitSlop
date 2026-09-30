> **Archived 2026-09-30.** The native-only validator this harness evaluated is now in
> production (`crates/hitslop-core/src/manifest.rs`). The harness imports code that has
> since been deleted, so it is kept for its measurements and corpus, not to be rerun. The
> corpus lives on as `packages/schema/tests/fixtures/manifest-cases.json`, checked by both
> TypeBox and the native validator. Paths below predate the move to `archive/spikes/`.

# Manifest validator spike

An isolated comparison of `jsonschema` 0.58.3 runtime and compile-time validators
against the actual TypeBox-generated manifest schema. Production callers and the
root Cargo manifest/lockfile are unchanged.

From the repository root, with Bun, Cargo, Swift and the matching wasm-bindgen CLI:

```sh
bun run schema:check
bun spikes/manifest-validation/prepare.ts
cargo fetch --locked --manifest-path generated/manifest-validation-spike/workspace/Cargo.toml
bun spikes/manifest-validation/build.ts
bun spikes/manifest-validation/measure.ts
```

The existing `generated/core/wasm` artifact must match the current core; build it with
`bun run core:build:wasm` if needed. It supplies the reference shape validator.
Run preparation once, then build and measure sequentially. A build mode can be
selected with `build.ts baseline`, `build.ts runtime`, or `build.ts compiled`.

`prepare.ts` copies the current Rust workspace into
`generated/manifest-validation-spike/workspace`, adds a probe to the full native and
WASM adapters, and injects the experimental validation module. The lockfile here
pins that disposable workspace, including the added library. Default dependency
features are disabled; the compile-time variant enables only `macros` additionally.
Both variants select Draft 7 and enable format checks. The production generated
schema is consumed unchanged.

The baseline retains the same adapters, JSON parsing, shape validation and probe
serialization, but has no manifest-schema validator. It is a size/control baseline,
not a candidate manifest acceptance implementation. Native sizes are arm64 release
dylibs, not the universal static archive or final signed app. WASM sizes are the
same `wasm-bindgen --target web` output used by the existing build; no extra optimizer
is applied. Gzip figures are provided separately.

Native and WASM probes return schema acceptance separately from complete acceptance
(schema plus the existing Rust shape parser). Compare with TypeBox plus the existing
WASM shape validator. The Swift reference compiles the current generated interpreter
source directly; it measures schema validation, not complete package opening.
Filesystem checks, image decoding and Codable decoding are outside this experiment.

Timings use 20 fresh processes per variant. Cold validation includes JSON parsing
and first-use initialization. Warm samples average 1,000 valid-manifest calls.
WASM module import/compilation/instantiation is measured separately from its first
validation. These are local Bun and arm64 results, not WebKit startup measurements.
Build durations are diagnostic only: the first build has a cold dependency cache,
while later variants reuse it.

All disposable binaries, compiler logs, dependency trees, generated case inputs and
full measurements remain under `generated/manifest-validation-spike/`. Measurement
also writes `measurements.json` here as durable evidence; `summarize.ts` can regenerate
that summary from an existing full result. The report in
`plans/manifest-validation-spike.md` records the result and limitations.
