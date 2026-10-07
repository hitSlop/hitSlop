# Rust socket wire and native verification

2026-10-06. Changes staged without commits.

Rust decodes socket requests and discovery records directly. It checks the protocol
before interpreting request fields, including payloads the current build cannot
represent. A regression with a future protocol and `1e999` failed with
`invalid_request` before the fix, then passed with `requires_update`; the document
bytes remain unchanged. Discovery keeps its existing serialized fields and tolerates
unknown fields. Explicit request checks preserve the former bounds and null rules.

ts-rs exports the request types; replies reuse the engine's structured reply types.
The socket TypeBox module, five JSON Schema artifacts, duplicate validators and 380
generated Swift lines are deleted. Production Swift uses the UniFFI export-format
enum; a small reply header remains in native test support only.

Native verification also exposed an unset C deployment target: `libdeflate-sys`
compiled for the installed macOS 27 SDK's minimum instead of the supported host.
`.cargo/config.toml` now fixes `MACOSX_DEPLOYMENT_TARGET` to 15.0 for Cargo and its C
builds, matching the Swift package floor. `otool` reports `minos 15.0` on the rebuilt
object, and the native retry has no newer-macOS link warnings. This verifies build
targeting, not execution on an actual macOS 15 machine.

Verification:

- The regression fails before the fix and passes after it.
- Focused socket, command and envelope suites: 20 passing tests.
- Schema drift, TypeScript, templates and workspace Clippy with ts-rs pass.
- Full nonnative verification: 271 Rust tests (4 skipped), 150 Bun tests, landing
  checks and installed-package checks pass.
- `bun run verify --native`: Swift and 56 native integration tests pass (one native
  test skipped), including relocated helpers, real exports and the prelaunch corpus.

The page validator and Swift page/host generator remain. Manifest and stored command
validators remain until their packaging boundaries switch. No legacy reader,
migration, compatibility marker change or commit was added.
