# Packaging integration

Verified checkpoint for the app-definition and packaging plan (executed and archived 2026-10-07; open items moved to [the roadmap](../../../docs/roadmap.md)).
Changes are staged and remain uncommitted on `ready-ship`; HEAD remains `6557240e`.
Linux qualification is deferred by request.

## Result

- Rust owns internal serde wires, the format-1 definition decoder, catalog summaries,
  descriptor validation, resource acceptance and durable storage. ts-rs 12.0.1 exports
  TypeScript; UniFFI 0.32.2 carries native types and opaque page replies.
- One explicit `defineSlop` entry selects views, CSS imports, images, skins, artwork,
  document shape, initial values and named commands. Both compiler modes use Vite.
  The compiler resolves its injected SDK helpers from the project's SDK with Vite;
  global and project installations cannot create separate page contexts. Canonical
  source paths keep symlink aliases out of component tokens and embedded output.
- One immutable definition JSON value holds window, ordered theme, original descriptor,
  views and command descriptors. Initial values become a Loro checkpoint. Catalog fields
  remain scalar columns. Assets, attachments and artwork retain separate lifecycles.
- Pack inserts assets first and the app row last. The six triggers refuse updates,
  deletes and replacement inserts, including explicit-rowid bypasses. Summary's read
  set is tested with a SQLite authorizer; it is not full acceptance.
- Page and CLI commands share the native owner's restricted child runner. Arguments are
  checked before spawning; returned intents take one version-guarded batch. One definite
  stale conflict retries with the same clock/seed; lifecycle changes fence the result.
  Every evaluation starts with a fresh process and realm. The UI contains stubs only.
- Native WebKit and browser preview serve attachments by URL, with ranges, first-touch
  whole-blob hash verification, sniffed types, `nosniff` and CSP `sandbox`. Commands are
  never page-visible resources. PNG acceptance streams the full image including IEND.
- TypeBox, quicktype, handwritten wire generators, manifest/schema artifacts and runtime
  JSON Schema validators are removed. `jsonschema` remains only a development dependency
  used as an independent oracle for the output-only `describe` projection.
- Swift no longer parses app, theme, page or host JSON. Native theme controls also use a
  typed Rust adapter. The evaluator is embedded and signed with the native helper.

## Evidence collected

- Rust focused suites: file acceptance/seal/ranges, owner/storage, argument projection,
  marker-first refusals and delayed command lifecycle/stale retry tests.
- 143 Bun tests passed; TypeScript and every active template passed Svelte checking.
- All five installed-package tests passed with native checks enabled: SDK imports,
  build/check/create/call, register, global skills/upgrades and real WebKit preview.
  The tests isolate template registration from the account's pre-launch catalog.
- Native WebKit policy probe passed: app workers/worklets and WASM run; blob/data code
  and attachment workers/modules are refused. A misnamed imported PNG remains drawable
  to an untainted canvas; byte ranges and passive attachment response headers hold.
  WebKit matches CSP paths on `slop:`; no separate attachment host was needed.
- Native preview HMR, definition-error recovery and reset, imported attachment URLs and
  abrupt owner death passed. Death fences edits and invalidates resource URLs; reload
  starts a new disposable document.
- M1 measurements, 40 fresh child evaluations: native WebKit request-to-reply p50 11 ms,
  p95 13 ms; browser SDK command-promise p50 11 ms, p95 15 ms. Both satisfy the 50 ms
  gate without a warm process. The native command test also confirms a CLI call reaches
  the open page and survives close/reopen.
- Existing skin geometry, artwork replacement, catalog, capture, theme and startup
  tests were ported to explicit BuildInput resource/artwork registration and passed
  their focused rerun. Landing checks report no errors or warnings.

## Final verification

`HITSLOP_CARGO_PROFILE=dist bun run verify --native` passed on the M1:

| Boundary | Result |
| --- | --- |
| Repository, generated contracts, TypeScript and template types | Passed |
| Bun SDK/compiler/CLI | 143 passed |
| Rust workspace, formatting and native/WASM clippy | 277 tests passed; 4 opt-in cases skipped |
| Landing documentation | No errors or warnings |
| Installed npm package | Passed; the native-enabled rerun covers all 5 workflows |
| Swift | 184 tests passed across 3 shards |
| Native CLI, browser preview, rendering and corpus replay | 57 passed; the real-app crash case ran separately |

The fresh, unfrozen `tests/compat/dev` corpus contains 16 saved-document cases, including
page-written documents. Rust, Swift and native replay passed with the original embedded
programs. All five markers remain 1. No frozen corpus exists or was changed; pre-launch
files are not supported by a legacy reader.

The macOS app was built with its embedded evaluator and helper signatures checked.
The artifact check passed matching-core/shell checks, packaged-template acceptance,
create/reopen/edit, and PNG/PDF export without Bun or Node on the helper's path. A live
app command succeeded with `HITSLOP_EVALUATOR` unset, proving it used the bundled helper.
The real-app crash test passed with an isolated `HITSLOP_TEST_REGISTRY`: an acknowledged
edit survived killing the host and reopening the document. The first attempt using the
shared test registry timed out during startup; it did not report lost data.

The full run took 266 seconds. Bun (44 seconds) and Swift (97 seconds) exceeded their
advisory tier budgets; command latency stayed within the separate product gate above.
This is macOS verification, not a Linux or signed/notarized release qualification.
