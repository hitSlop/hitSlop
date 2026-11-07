# Shared document core

The document semantics every host uses: the Swift app and the CLI's engine (`pop dev`
included) use the native Rust core. Bun SDK tests use the WASM adapter.

- `pop-contracts`: passive shared wire, error, limit, app-presentation, media-registry
  and evaluator types; no Loro, SQLite or renderer.
- `pop-core`: pure Loro document semantics, app acceptance and sync. It has no storage or
  runtime features by default; optional `acceptance` codecs and `ts` generation are tooling.
- `pop-storage`: SQLite `.pop` files, file acceptance, registry, attachments and atomic
  publication. It depends on core and contracts.
- `pop-capture`: saved-source capture and widgets; it uses storage, core and the restricted
  runner.
- `pop-runtime`: owner, command, socket and persistence orchestration; it composes core,
  storage, capture and runner.
- `pop-renderer`: Takumi raster/PDF rendering and fonts; no JavaScript or owner.
- `pop-library`: device-local cloud index, pending catalog writes, downloaded copies and
  recovery copies over runtime and storage; Firestore IO stays in the Apple SDK adapter.
- `pop-core-ffi`: UniFFI adapter used by the Apple host and its Quick Look extensions.
- `pop-core-wasm`: wasm-bindgen adapters for SDK tests and durable browser copies; browser authoring development uses the native owner.
- `pop-engine`: the document engine the CLI runs on any platform (templates, create,
  pack, inspect, schema, export and document operations); `pack` also renders the artwork
  a build does not supply when asked. On macOS and Linux the engine exports closed documents in its own
  restricted evaluator; on macOS it also opens documents in dotpop.app.
- `pop-sharing-protocol`: the cloud service's wire types, route table (`ROUTES`), limits
  and failure table, free of Loro, used by the native and web clients and the Worker.
- `pop-sharing-client`: native identity-authenticated, event-driven transport, cloud
  HTTP calls and durable confirmed-version receipts over the existing owner.
- `pop-sync`: the pure supervisor shared by native and browser drivers (reconnect
  backoff, fold/compaction decisions, push budgets); no I/O, clocks or Loro, and document
  replication remains in core.
- `pop-runner`: the restricted QuickJS child and its bounded process launcher, with
  no storage dependency. The engine re-executes itself with `--evaluate-command`; the
  `pop-evaluator` binary provides the same child for native-host integration.

`apps/sharing` is the Cloudflare Worker built on `pop-sharing-protocol`;
`tools/pop-bindgen` also generates its `openapi.json`.

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
LTO (about a fifth smaller, five times slower to rebuild): `POP_CARGO_PROFILE=dist`
selects it, and the release gate, compatibility capture, the release workflow and the
Engines workflow set it. CI caches artifacts by toolchain, lockfile and source; a cache hit
never substitutes for Cargo's dependency checks.
Generated XCFramework and Swift bindings are disposable and excluded from Git. Rust owns
shared wire types and limits in `pop-contracts` and the sharing protocol; `tools/pop-bindgen` uses ts-rs to export
TypeScript and UniFFI carries native types to Swift. App acceptance belongs to `pop-core`;
file acceptance belongs to `pop-storage`. Run `bun run schema:generate`; never edit
generated files manually.
The Cargo configuration pins macOS 15.0 for Rust and C dependencies, matching the lowest
native consumer (the Swift package), rather than the locally installed SDK.

`pop-storage` owns the `.pop` file (one SQLite database holding the app and its saved
state), its schema and file acceptance, save planning and attachments, and the registry
writer lock. `pop-runtime` owns the live owner and persistence scheduling. Swift's
`DocumentOwner` is the owner's façade.
See [the architecture](../docs/architecture.md).

The semantic implementation lives under `pop-core`'s `document/`: session history,
frontier tokens, replication and history maintenance stay beside `Document`. Storage,
capture and runtime responsibilities live in their respective crates rather than behind
features on `pop-core`.
