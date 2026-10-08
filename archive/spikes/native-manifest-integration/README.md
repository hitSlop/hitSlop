> **Archived 2026-09-30.** `candidate.patch` has been applied to production and later
> extended to socket and bridge envelopes (see
> [envelope validation evidence](../../../docs/evidence/envelope-validation-2026-09-30.md)).
> The harness is kept for its measurements, not to be rerun; the patch no longer applies.
> Paths below predate the move to `archive/spikes/`.

# Native manifest integration spike

This experiment replaces native manifest schema evaluation with the compiled
`jsonschema` validator, while keeping TypeBox validation in authoring and keeping
manifest validation out of WASM. The implementation lives in an isolated workspace;
`candidate.patch` is the reviewable production delta. It is not applied to the main
working tree automatically.

See [the report](../../plans/native-manifest-integration-spike.md) for results and
limitations. `measurements.json` retains corpus outcomes and raw timing samples.

## Reproduce

Use the current macOS development toolchain, installed Bun dependencies, and built
native/WASM artifacts (`bun run build`). The harness uses APFS clone copies of build
caches and installed dependencies. Clones are independently writable; package
symlinks resolve within the experiment. Relocated Swift caches may need recompiling.

The 106-case corpus comes from the preceding manifest-library spike. Generate it
with `bun spikes/manifest-validation/prepare.ts` if its generated cases are absent;
there is no need to rebuild that spike's three variants.

From the repository root:

```sh
bun spikes/native-manifest-integration/prepare.ts
bun spikes/native-manifest-integration/install-harness.ts
bun spikes/native-manifest-integration/run.ts baseline-regressions swift test --no-parallel --package-path apps/apple/Packages/HitSlopApple --filter 'nativeManifestParityRegressions|manifestInputBoundaryRemainsStrict'
# Expected: five assertion failures in the parity test; the input-boundary test passes.
bun spikes/native-manifest-integration/run.ts baseline-wasm bun run core:build:wasm
bun spikes/native-manifest-integration/run.ts baseline-release swift build --package-path apps/apple/Packages/HitSlopApple -c release --product hitslop-native
bun spikes/native-manifest-integration/run.ts baseline-probe swift build --package-path apps/apple/Packages/HitSlopApple -c release --product manifest-integration-probe
bun spikes/native-manifest-integration/measure.ts baseline

bun spikes/native-manifest-integration/apply-candidate.ts
bun spikes/native-manifest-integration/run.ts candidate-schema bun run schema:generate
bun spikes/native-manifest-integration/run.ts candidate-lock cargo check -p hitslop-core-ffi --offline
bun spikes/native-manifest-integration/run.ts candidate-rust cargo test --locked --workspace
bun spikes/native-manifest-integration/run.ts candidate-build bun run build
bun spikes/native-manifest-integration/run.ts candidate-check-final bun run check
bun spikes/native-manifest-integration/run.ts candidate-test bun run test
bun spikes/native-manifest-integration/run.ts candidate-swift bun run swift:test
bun spikes/native-manifest-integration/run.ts candidate-core-final swift test --no-parallel --package-path apps/apple/Packages/HitSlopApple --filter HitSlopCoreTests
bun spikes/native-manifest-integration/run.ts candidate-native bun run test:native
bun spikes/native-manifest-integration/run.ts candidate-restored bun run test:restored
bun spikes/native-manifest-integration/run.ts candidate-release swift build --package-path apps/apple/Packages/HitSlopApple -c release --product hitslop-native
bun spikes/native-manifest-integration/run.ts candidate-probe swift build --package-path apps/apple/Packages/HitSlopApple -c release --product manifest-integration-probe
bun spikes/native-manifest-integration/measure.ts candidate
bun spikes/native-manifest-integration/run.ts wasm-normal-tree cargo tree --locked --target wasm32-unknown-unknown -p hitslop-core-wasm -e normal
bun spikes/native-manifest-integration/run.ts wasm-feature-tree cargo tree --locked --target wasm32-unknown-unknown -p hitslop-core-wasm --features hitslop-core/manifest-validation -e normal
bun spikes/native-manifest-integration/run.ts wasm-feature-build cargo build --locked --release --target wasm32-unknown-unknown -p hitslop-core-wasm --features hitslop-core/manifest-validation
bun spikes/native-manifest-integration/run.ts wasm-feature-bindgen generated/core-tools/bin/wasm-bindgen --target web --out-dir ../feature-wasm target/wasm32-unknown-unknown/release/hitslop_core_wasm.wasm
# Capture evidence needs the native macOS session and Poppler's pdftoppm.
bun spikes/native-manifest-integration/run.ts candidate-visual bun "$PWD/spikes/native-manifest-integration/visual.ts"
bun spikes/native-manifest-integration/run.ts baseline-visual bun "$PWD/spikes/native-manifest-integration/visual.ts" --baseline
bun spikes/native-manifest-integration/compare-visual.ts
bun spikes/native-manifest-integration/summarize.ts
```

`run.ts` executes in `generated/native-manifest-integration/workspace` and saves
stdout, stderr, command, exit status and elapsed time in the adjacent `logs/`.
Build and test commands need normal Swift compiler-cache access and UI-capable
macOS execution. Preparation and candidate application are one-time steps. Do not
rerun them over an existing experiment.

The visual commands use an absolute script path from the main repository to preserve
a command log while running against the isolated workspace. They refuse existing
document copies; use a fresh experiment for a new complete capture comparison.
`measure.ts` saves the resource
bundle beside each helper so the saved baseline can export documents later.

For explicit Cargo feature-unification verification, run these in the isolated
workspace and inspect the trees for absence of `jsonschema`:

```sh
cargo tree --locked --target wasm32-unknown-unknown -p hitslop-core-wasm -e normal
cargo tree --locked --target wasm32-unknown-unknown -p hitslop-core-wasm --features hitslop-core/manifest-validation -e normal
cargo build --locked --release --target wasm32-unknown-unknown -p hitslop-core-wasm --features hitslop-core/manifest-validation
```

Compare the resulting WASM after the same `wasm-bindgen --target web` step. The
feature and target gates are compile-time boundaries; there is no runtime validator
selection flag. The probe's Swift define selects which diagnostic API to measure
and is not part of the production patch.

## Measurement boundaries

The native size comparison uses the actual arm64 Release `hitslop-native` executable,
copied before and after the candidate and stripped identically with `strip -S -x`.
It includes retained Swift code and linked Rust code. It is not a universal archive,
a dylib estimate, or a signed app-size measurement.

Timing starts immediately before `SlopPackage(rootURL:)`. Each of 20 fresh processes
opens the package once, then averages 100 further opens. Standard, explicit-radius
and PNG-skin packages are measured separately. First-open timings include lazy
validator initialization, Codable, native geometry, package checks and image decoding
where applicable. Filesystem caches are warm. Process launch, window creation and
WebKit are outside the timer.

Corpus packages contain their original manifest JSON, minimal app/state files and
real PNG files for safe skin paths. The probe records native FFI acceptance separately
from complete package opening. Expected FFI acceptance is TypeBox plus the existing
shape parser. Native filesystem policy deliberately remains stricter for unsafe
paths accepted by the current schema. The Swift regressions separately cover invalid
UTF-8, malformed JSON and the exact 64 KiB boundary.

The saved patch includes integration and regression tests, generated schema-output
changes and Cargo dependency changes. It excludes the probe product, its conditional
define, measurement scripts and binaries. The Swift socket/page schema interpreter
remains; only its embedded manifest schema and native manifest caller are removed.
