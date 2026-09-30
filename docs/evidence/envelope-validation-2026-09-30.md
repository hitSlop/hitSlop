# Native envelope validation evidence — 2026-09-30

Measured on an Apple M1 Mac, macOS 26.6.2, arm64; Rust 1.96.1; `jsonschema` 0.58.3
(`macros` only, Draft 7, native-only). Dirty working tree based on
`6116a2e81e12802a610e5dacd6d7278c52d41570`.

The core now evaluates the TypeBox-generated schemas for the manifest (earlier
spike reports, since archived locally) and for socket
requests, replies, discovery files and page bridge requests
(`crates/hitslop-core/src/envelope.rs`). The Swift schema interpreter
(`PlatformContract`), its generator and the embedded schema blobs are deleted.

## Helper size

Release `hitslop-native`, arm64, both copies stripped with `strip -S -x`, same toolchain:

| Tree | Bytes |
| --- | ---: |
| Manifest validator only (Swift interpreter still validating envelopes) | 8,742,120 |
| Manifest and envelope validators (interpreter removed) | 8,793,480 |
| Change | +51,360 (+0.6%) |

The `jsonschema` runtime was already paid for by the manifest validator, so four more
compiled validators cost 50 KB, less the Swift interpreter and schema blobs. WASM is
unchanged: the dependency stays out of `wasm32`. This is not a signed app or universal
binary measurement.

## Call cost

Release build, one thread, `serde_json` parse included, warm:

| Envelope | Per call |
| --- | ---: |
| Socket request (apply) | 2.07 µs |
| Socket reply | 0.60 µs |
| Bridge `window.resize` | 0.35 µs |
| `attachments.put`, 10 MiB base64 | 2.35 ms |

The page's `open`, `apply`, `text` and `flush` messages never went through the Swift
interpreter (`DocumentSession.userContentController` hands them to `owner.admitPage`
first), so the typing path is unchanged. The interpreter guarded the CLI socket and the
low-frequency bridge methods only.

## Behavior changes

The Swift interpreter skipped some schema keywords; the core enforces them all:

- A pattern-keyed record (theme values) now rejects keys that do not match its pattern
  (`additionalProperties: false` added to `ThemeValuesSchema`). Before, the owner's theme
  rules caught them later.
- Every bridge method rejects unknown fields in the schema itself; `StorageRequest` no
  longer re-checks keys by hand.
- Skin paths reject `.`, `..` and empty components in the schema. Native `containedURL`
  still checks the filesystem.

## Checks

`cargo test --locked --workspace` (envelope and manifest corpus tests), `bun run check`,
`bun run test` (92), `bun run build`, `bun run swift:test`, `bun run test:native` (9),
`bun run test:restored` (16). The shared manifest corpus
(`packages/schema/tests/fixtures/manifest-cases.json`, 88 cases) is checked by both
TypeBox and the native validator; flipping one expectation fails the Rust test naming the
case.
