# Shared document core

The document semantics every host uses: the Swift app and helper natively, and
`slop dev` and Bun tests through WASM. The original spike is archived in
`archive/spikes/hitslop-core`.

- `hitslop-core`: descriptor interpretation, Loro operations, publications and bytes; with
  `storage`, the `.slop` file, its writer lock and the save policy.
- `hitslop-core-ffi`: UniFFI adapter used by the Apple host, its Quick Look extensions and
  the native helper.
- `hitslop-core-wasm`: wasm-bindgen adapter for browser development and Bun tests.
- `slop-engine`: the CLI's file tool (`pack`, `inspect`, `schema`) on any platform.

The root toolchain/lockfile pin Rust 1.96.1, Loro 1.16.2, UniFFI 0.31.1 and
wasm-bindgen 0.2.127. Install the matching generator once:

```sh
cargo install wasm-bindgen-cli --version 0.2.127 --locked --root generated/core-tools
bun run schema:generate
bun run core:build:wasm
bun run build # macOS: the native binding and the engine
bun run core:test
```

`bun run build` prepares both bindings and the engine before Swift. `bun run test`
refreshes the WASM binding and the engine before SDK tests. CI caches artifacts by toolchain, lockfile and source;
a cache hit never substitutes for Cargo's dependency checks. The native build prepares arm64 and x86_64 macOS slices before making its universal
XCFramework, so CI test architecture does not dictate the release architecture.
Generated XCFramework and Swift bindings are disposable and excluded from Git. TypeBox owns wire types;
run `bun run schema:generate`, never edit `wire.generated.rs` manually.

The core's `file` owns the `.slop` file (one SQLite database holding the app and its
saved state) and the checks every open runs; its `store` persists the checkpoint, the
update log, theme overrides and attachments, and its `registry` owns the writer lock. The
Swift owner only schedules saves.
See [the architecture](../docs/architecture.md).
