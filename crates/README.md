# Shared document core

The document semantics every host uses: the Swift app and helper natively, and
`slop dev` and Bun tests through WASM. The original spike is archived in
`archive/spikes/hitslop-core`.

- `hitslop-core`: descriptor interpretation, Loro operations, publications and bytes.
- `hitslop-core-ffi`: UniFFI adapter used by the Apple host and native helper.
- `hitslop-core-wasm`: wasm-bindgen adapter for browser development and Bun tests.

The root toolchain/lockfile pin Rust 1.96.1, Loro 1.16.2, UniFFI 0.31.1 and
wasm-bindgen 0.2.127. Install the matching generator once:

```sh
cargo install wasm-bindgen-cli --version 0.2.127 --locked --root generated/core-tools
bun run schema:generate
bun run core:build:wasm
bun run core:build:native # macOS only
bun run core:test
```

`bun run build` prepares both bindings before Swift. `bun run test` refreshes the
WASM binding before SDK tests. CI caches artifacts by toolchain, lockfile and source;
a cache hit never substitutes for Cargo's dependency checks. The native build prepares arm64 and x86_64 macOS slices before making its universal
XCFramework, so CI test architecture does not dictate the release architecture.
Generated XCFramework and Swift bindings are disposable and excluded from Git. TypeBox owns wire types;
run `bun run schema:generate`, never edit `wire.generated.rs` manually.

The core's `store` persists the checkpoint, the update log and the theme overrides in
`state/document.sqlite`, and owns the writer lock; the Swift owner only schedules saves.
See [the architecture](../docs/architecture.md).
