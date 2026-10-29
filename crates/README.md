# Shared document core

The document semantics every host uses: the Swift app and the CLI's engine (`tiny dev`
included) use the native Rust core. Bun SDK tests use the WASM adapter.

- `tinys-core`: descriptor interpretation, Loro operations, publications and bytes; with
  `storage`, the `.tiny` file, its writer lock and the save policy.
- `tinys-core-ffi`: UniFFI adapter used by the Apple host and its Quick Look extensions.
- `tinys-core-wasm`: wasm-bindgen adapters for SDK tests and durable browser copies; browser authoring development uses the native owner.
- `tiny-engine`: the document engine the CLI runs on any platform (templates, create,
  pack, inspect, schema, export and document operations); `pack` also renders the artwork
  a build does not supply when asked. On macOS and Linux the engine exports closed documents in its own
  restricted evaluator; on macOS it also opens documents in tinys.app.
- `tinys-sharing-protocol`: the relay's wire types, shared limits and failure table,
  free of Loro, used by the owner, the native and web clients and the relay.
- `tinys-sharing-client`: the native relay client over the existing owner; Swift
  persists the opaque credentials.
- `tinys-runner`: the restricted QuickJS child and its bounded process launcher, with
  no storage dependency. The engine re-executes itself with `--evaluate-command`; the
  `tinys-evaluator` binary provides the same child for native-host integration.

The root toolchain/lockfile pin Rust 1.96.1 (edition 2024, with clippy and rustfmt),
Loro 1.17.1 (the exact crates.io release), UniFFI 0.32.2 and wasm-bindgen 0.2.129.
`bun run verify rust` runs the workspace's lints and tests.
Use `cargo fmt --all` to apply `rustfmt.toml` explicitly. Install the matching bindings generators and the test runner once
(the uniffi library leaves out its generator, so every host build shares one Cargo graph):

```sh
cargo install wasm-bindgen-cli --version 0.2.129 --locked --root generated/core-tools
cargo install uniffi --version 0.32.2 --features cli --locked --root generated/core-tools --bin uniffi-bindgen
brew install cargo-nextest
bun run schema:generate
bun run core:build:wasm
bun run build # macOS: the native binding and the engine
bun run core:test
```

`bun run build` prepares both bindings and the engine before Swift; `bun run verify bun`
refreshes the WASM binding and the engine before SDK tests. A step whose output would not
change rewrites nothing, so a repeated build recompiles nothing. On a Mac the engine and
the app's core library build together, for Apple silicon only (the app ships arm64).
Development builds use Cargo's `release` profile, in `target/release`: optimized, because
the core's timeouts and save scheduling assume it. What ships uses `dist`, which adds fat
LTO (about a fifth smaller, five times slower to rebuild): `TINYS_CARGO_PROFILE=dist`
selects it, and the release gate, compatibility capture, the release workflow and the
Engines workflow set it. CI caches artifacts by toolchain, lockfile and source; a cache hit
never substitutes for Cargo's dependency checks.
Generated XCFramework and Swift bindings are disposable and excluded from Git. Rust owns
wire types and shared limits in `tinys-core/src/wire`; ts-rs exports TypeScript and
UniFFI carries native types to Swift; the app definition and its format readers live in
`tinys-core/src/app`. Run `bun run schema:generate`; never edit generated files manually.
The Cargo configuration pins macOS 15.0 for Rust and C dependencies, matching the lowest
native consumer (the Swift package), rather than the locally installed SDK.

The core's `file` owns the `.tiny` file (one SQLite database holding the app and its
saved state), every statement on its tables (every write in `file::rows`) and the checks
every open runs; its `store` decides what a save writes (a snapshot or the updates since the last save, in `history`,
with theme overrides and attachments), its `owner` schedules saves, and its `registry` owns the
writer lock. Swift's `DocumentOwner` is the owner's façade.
See [the architecture](../docs/architecture.md).

The semantic implementation lives under `document/`: session history, frontier tokens,
replication and history maintenance stay beside `Document`. The crate root re-exports its
public types. `store/` separates save planning, attachments, native copies and browser
storage; `file/` separates connections, acceptance and app reads. `owner/` keeps the
actor's admission and save ordering together, with failures and serial persistence in
child modules.
