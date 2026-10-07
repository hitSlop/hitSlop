# Rust wire generator removed

2026-10-06. Changes staged without commits.

Deleted `scripts/build/rust-contracts.ts` and `wire.generated.rs`. Their remaining
definitions now live as ordinary serde types in `wire/core.rs`, `socket.rs`,
`page.rs` and the transitional compiler input `app_row.rs`. There is no generated
Rust wire source to hand-edit or schema-identity matching code to maintain.

ts-rs now exports the core intents, batches, paths, text changes, publications,
snapshots and theme-file types. SDK/shell type aliases consume those definitions.
Unused TypeBox input schemas for these payloads were removed. The output schemas
still used by the remaining page generator stay until that boundary moves.

Rust owns shared limits and ordered code/category lists. The small value exporter
writes `constants.generated.ts`; the TypeScript module only re-exports values and
keeps two JavaScript convenience helpers. A before/after comparison confirmed every
exported constant retains its value. The pre-launch acceptance JSON changed object
key order only. No released corpus or marker changed.

Verification:

- `bun run schema:generate` and drift checks pass.
- TypeScript and every template pass; no generated output was hand-edited.
- Workspace Clippy with all targets and the ts-rs feature passes.
- `bun run verify` passed in 436.1 seconds: 266 Rust tests (4 skipped), 152 Bun tests,
  landing checks and four installed-package checks (one skipped).

TypeBox is still needed for the current manifest/command schemas and the remaining
page/socket validators. The Swift contract generator and quicktype are not yet
removed. The next boundary checkpoint removes the socket validator/generator path.
