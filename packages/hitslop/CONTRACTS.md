# Internal contracts

`src/schema` owns TypeBox contracts for the manifest, core payloads, page bridge and
command protocol. It is private to `hitslop`; authors import the package root or its
`/svelte` and `/embed` exports. Run `bun run schema:generate` in the repository to emit
Rust, Swift and JSON contracts. Never edit generated files.

Released acceptance rules in `acceptance/` belong to format/storage markers. New
release numbers do not tighten those rules or upgrade embedded document apps.
