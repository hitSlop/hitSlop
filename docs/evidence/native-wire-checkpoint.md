# Rust-owned native helper wire

2026-10-06 · `ready-ship` · macOS arm64

The native helper portion of packaging step 2 replaces JSON Schema validation followed
by Swift mapping with one checked Rust decode. The helper receives `NativeWireRequest`
through UniFFI and returns `NativeWireReply`; Rust supplies the JSON discriminants and
serializes the response. Export still forwards the original bytes through the existing
Rust command router and supplies only the closed-document renderer.

## Removed

- `packages/hitslop/src/schema/engine.ts`, its last TypeBox definitions and its obsolete
  native-request test. Engine types moved at the preceding checkpoint; native types now
  live in `crates/hitslop-core/src/wire/native.rs`.
- The native request/reply JSON Schema artifacts and their compiled validators.
- The native branch of `swift-contracts.ts`, including 183 generated Swift lines.
- Engine/native variants from the transitional `Envelope` API. The engine now decodes
  helper replies as typed Rust outcomes, checks the matching method, and preserves an
  unknown outcome when the response is malformed.
- `Envelope.valid`, `JSONSerialization`, and generated native-request mapping from the
  helper's request path.

Rust checks request size before decoding, rejects missing/unknown/duplicate fields,
and enforces the existing path bounds before host work. The Swift stdin loop remains
bounded, and protocol negotiation still precedes stdin and AppKit. The reply adapter
uses the existing engine wire serializer, including required nullable screenshot output
and optional classified-error fields. UniFFI uses the existing compile-checked remote
type pattern; no new Swift dependency or separate schema framework is introduced.

## Verification

`bun run verify --native` passed in 538.2 seconds: hygiene, generated-contract drift,
TypeScript/templates, 150 Bun tests, 229 Rust tests (four skipped), four installed-package
tests (one skipped), all 182 Swift tests in three shards, and 56 native integration tests (one skipped).
The Rust suite took 16.1 seconds after discovery; newly linked test discovery accounted
for most of its 224.4-second tier time.

Three new Rust tests cover malformed/missing/duplicate native fields, path and total
input bounds, the native-only reply subset, and required nullable outputs. A Swift test
crosses the actual UniFFI boundary for a screenshot request, an absent screenshot and
an invalid path. Existing host tests exercise protocol refusal, renderer-only routing,
exports of live and closed documents, and newer-marker refusals. The native tier also
replays the original development-corpus bundles and their attachments through rendering,
editing and reopening.

The native build emits deployment-target warnings for existing libdeflate objects built
for macOS 27 while Swift links for macOS 15. This run verifies the current Mac; it does
not establish minimum-OS compatibility. Resolve the native dependency build target before release.

## Remaining migration

`rust-contracts.ts` still emits core operations, socket/page contracts, the old app row
and constants. `swift-contracts.ts` still serves socket/page/host consumers. Quicktype
still emits the old manifest model. These are live migration dependencies, not the
target architecture; each emitter leaves when its remaining consumers move to Rust
types and UniFFI. The default builder and stored app format have not switched yet.

The remaining `BuildInput` work subsequently completed the wire-foundation step; see
[that checkpoint](build-input-checkpoint.md). Typed app acceptance, storage and descriptor
arguments follow it. Linux remains deferred.
