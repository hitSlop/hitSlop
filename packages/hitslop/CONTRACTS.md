# Internal contracts

Rust owns the wire types, limits and app definition (`crates/hitslop-core/src/wire`,
`src/app`). `bun run schema:generate` exports them to `src/wire/*.generated.ts` with ts-rs,
plus a few constants; `src/schema` re-exports the ones the SDK, shell and CLI share. Both
are private to `hitslop`: authors import the package root or its `/svelte` and `/embed`
exports. Never edit generated files.

Each released package format keeps its own acceptance rules in Rust
(`app/package_format_N.rs`), selected by a file's marker. New releases never tighten them
or upgrade the app embedded in a document.
