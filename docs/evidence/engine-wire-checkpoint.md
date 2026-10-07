# Rust-owned engine wire checkpoint

2026-10-06 · `ready-ship` · macOS arm64

The engine portion of packaging step 2 is implemented. Its request and reply types
live in [wire/engine.rs](../../crates/hitslop-core/src/wire/engine.rs). Core and owner
outcome enums live in `wire/codes.rs`. The remaining generated Rust contracts are
temporarily re-exported through `wire/mod.rs` while their boundaries migrate.

## What changed

- `ts-rs` is pinned to `=12.0.1`, behind a code-generation-only feature. The exporter
  starts from `EngineRequest` and `EngineReply` and follows their dependencies with
  `TS::export_all`. No handwritten dependency list or TypeScript edits are required.
- `bun run schema:generate` exports to a fresh temporary directory. `--check` compares
  the resulting bytes without editing the checkout. Verification watches the Rust
  sources, exporter and generated TypeScript for drift.
- The CLI imports the generated union types and a small method-selection helper.
  It no longer runs TypeBox over engine requests or replies. A transport guard still
  requires the matching method, expected envelope fields and string-array IDs for
  mutation acknowledgements. Unknown classifications and incomplete replies are
  reported as unknown outcomes; they are never retried. Same-build read payloads
  are no longer checked against a duplicate nested JSON Schema.
- Engine request/reply JSON Schema artifacts and their compiled Rust validators are
  deleted, along with the corresponding handwritten Rust-generator paths. The
  remaining native-helper TypeBox schemas stay until that checkpoint.
- Paths, command names, attachment IDs and payload bounds have explicit Rust checks.
  Absent optional fields remain distinct from explicit null. Duplicate and unknown
  request fields are refused.

## Raw payloads and compatibility

An internally tagged serde enum buffers its contents and cannot preserve `RawValue`.
The decoder therefore uses a small raw envelope for `validateApp` and serde
deserialization for every other request. It never materializes a future app payload
before the app's own marker check. The existing `packageFormat: 999` payload containing `1e999` still
returns `requires_update`, rather than a number-decoding error. Command arguments are JSON values, as in their existing evaluation path; incidental
number formatting and whitespace are not a contract.

The regression also exposed an existing ordering bug in app acceptance: it required
today's fields and rejected unknown fields before reading the markers. The test
failed with `unknown field futureField` before the fix. Acceptance now reads the
two permanent markers first, then decodes the current format only when supported.
Future formats can omit today's fields or introduce new ones without masking the
required update refusal.

This is forward refusal, not support for older pre-launch formats. All markers remain
1; no migration or legacy reader was added. The development corpus remains replaceable.
After the first public release, released formats and their original corpus entries stay supported.

Structured reply fields replace the engine's opaque reply payloads. Manifest and
command metadata remain the existing acceptance model until packaging step 3;
the generated TypeScript temporarily references that manifest type. `schema` and
`wire` share the package's bottom dependency layer during this transition, and
neither may import SDK, shell or CLI implementation.

The protocol preflight, discovery shape, markers and development corpus are unchanged.
A test exercises the production refusal serializer and checks its exact bytes.
Rust's `ts` feature is not enabled by the native or WASM production builds.

## Verification

`bun run verify` passed: hygiene, contract drift, TypeScript/template checks, 151 Bun
tests, 226 Rust tests (four skipped), landing checks and four installed-package tests
(one skipped). The seven new Rust tests cover closed shapes, null/absence, bounds,
opaque payloads, marker ordering, result fields and permanent refusal bytes. Existing end-to-end
engine tests cover packing, creation, inspection, helper routing and protocol refusal.
The CLI regression cases include missing command results, invalid IDs and unknown
outcome classifications.

The final full run passed in 291.2 seconds, including a slow test-discovery phase.
Once discovery completed, the 226 Rust tests ran in 16.9 seconds. A prior incremental
verification passed in 75.2 seconds; its Rust tier took 20.3 seconds.
No test failure remained. No Swift or FFI surface changed, and Linux remains deferred.

## Next checkpoint

Native-helper serde/UniFFI types and Rust-owned `BuildInput` remain in step 2. The
default builder, app acceptance, storage switch, `s.*` arguments, owner-routed
commands and native development are subsequent work. This checkpoint does not
claim that TypeBox or `jsonschema` has left the whole product yet.
