# @hitslop/schema

TypeBox definitions and validation for hitSlop manifests, bridge and socket envelopes, and the owner wire. They generate the Rust wire types, Swift contracts and JSON schemas.

Socket types are exported from `@hitslop/schema/socket`; bridge requests and method-specific
successful replies come from `@hitslop/schema/bridge`. Shared limits, categories and codes
come from `@hitslop/schema/constants`, which needs no schema library. `bun run schema:generate` emits the
Swift socket models and bridge method enum alongside the existing native validators.
Validate untrusted Foundation dictionaries before mapping them into generated types.
Native code checks envelopes; the Rust core parses operations and document state.

```sh
bun add @hitslop/schema@4.0.0
```

See the [author guides](https://hitslop.com/docs/getting-started/) and [release guide](https://github.com/hitSlop/hitslop/blob/master/docs/guides/releasing.md).

Part of the hitSlop SDK 4.0.0. MIT licensed.
