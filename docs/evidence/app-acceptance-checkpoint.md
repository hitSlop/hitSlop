# Typed app acceptance foundation

2026-10-06. Step 3, staged without committing.

`app/package_format_1.rs` owns the new stored definition's decoder, fixed metadata
types and acceptance rules. `wire/build.rs` reuses those fixed types rather than
declaring another metadata model. The host `AppDefinition` and `WindowDefinition`
are separate from the serde decoder. Raising the package marker requires adding a
reader; no historical reader or migration was added for pre-launch files.

The new reader checks catalog metadata, URI syntax, standard/skin windows, existing
shape geometry, ordered theme defaults, unique commands with descriptor arguments,
and the document descriptor. Build declaration acceptance also checks initial values.
Theme validation and package/runtime marker checks are shared with the existing
production path. `fluent-uri` is now a direct storage dependency for URI syntax; it
was already present transitively, so this adds no new package to the lockfile.

The descriptor remains raw JSON inside the typed definition. Page boot can receive
the original representation, including descriptions, without serializing the parsed
Node and silently changing what the embedded SDK compares. Theme and command order
remain arrays. No JSON copy of document state is introduced.

`BuildInput` now carries `packageFormat` and `runtimeABI`. Its decoder reads these
before decoding the declaration; future payloads containing unknown fields or
`1e999` are refused with `requires_update`. The compiler supplies these markers, and
ts-rs regenerates the corresponding TypeScript.

Verification:

- Eight focused acceptance tests passed, including metadata diagnostics, unknown
  fields/null, window exclusivity, geometry/palette checks, descriptor-based command
  arguments, initial values, stored definition round trips and Loro save/reopen.
- `bun run schema:generate` passed.
- `bun run verify` passed in 362.9 seconds: 246 Rust tests (4 skipped), 152 Bun tests,
  four installed-package checks (1 skipped), types/contracts/hygiene.

This is structural definition acceptance. Resource bytes, media types, PNG decoding
and SQL publication remain the file boundary's responsibility. The new decoder is
not yet the production SQLite open path: that switch must land with the new layout,
packer, templates and native consumers. `file::summary` and removal of the current
manifest/command JSON Schema validators remain outstanding.
