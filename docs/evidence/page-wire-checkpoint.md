# Rust page routing and typed native actions

2026-10-06. Changes staged without commits.

The page posts JSON text and parses the JSON reply. Swift checks the sending WebView,
main frame and origin, then carries the string to Rust without inspecting its fields.
Rust decodes and checks the request once. Document requests run through the owner;
native actions cross UniFFI as `HostAction`. Both use the serial owner's view/lifecycle
fence, and Swift checks the current view again before performing a delayed host action.

Config is built in Rust from the accepted app and retains its original descriptor.
The current packer's presentation is projected into the Rust-owned window input type;
the planned storage switch will take that projection directly from `AppDefinition`.
Swift's file/window manifest decoder remains until that switch.

Rust also encodes native action replies and host-to-shell requests, and decodes capture
geometry. Success replies carry a method discriminator; the same-build shell checks it
and strips the transport fields before returning the result. Stored apps still call
the same host context, so no document marker or app rebuild is needed for this change.

Deleted:

- `scripts/build/swift-contracts.ts` and `Contracts.generated.swift`;
- `envelope.rs`, `Envelope`, `envelope_is_valid` and Swift `Envelope.swift`;
- the page TypeBox schema, page JSON Schema artifact and unused core output schemas;
- Swift page routing/serialization through dictionaries and generated JSON models.

Both handwritten wire generators are now gone. TypeBox and `jsonschema` still serve the
current manifest and command-authoring path; quicktype still emits its manifest model.
Those leave with their packaging/authoring switch. Constants generation also remains.

Verification:

- 18 focused Rust command/page tests pass: strict fields, bounds, diagnostic UTF-16
  limits, opaque payloads, capture results, config and replaced/closed view refusals.
- Bridge tests preserve definite refusals versus unknown outcomes and reject malformed
  JSON or a mismatched success method; TypeScript and every template pass.
- Full nonnative verification passes: 273 Rust tests (4 skipped), 149 Bun tests,
  schema drift, Clippy, landing checks and installed-package checks.
- Native compilation first caught a private-state access from the bridge extension;
  the closed-session check now lives in the session's host-action handler.
- `bun run verify --native` then passed: Swift and 56 native integration tests (one
  skipped), including real WebKit malformed-message/frame tests, resizing, capture,
  relocated helpers and replay of the original prelaunch corpus bytes.

No commit, migration, legacy reader or marker change was added.
