# Rust-owned build input

2026-10-06 · `ready-ship` · macOS arm64

Packaging step 2's type foundation is complete. `wire/build.rs` defines the compiler's
untrusted `BuildInput`, metadata, window variants, ordered theme/command entries,
resource inventory and artwork paths. ts-rs exports the complete dependency graph to
`src/wire/app.generated.ts`. The existing Rust shape parser supplies the shape types;
there is no separate TypeScript definition of the path/radius grammar.

The compiler imports `BuildInput` instead of declaring its own transport type. The
explicit `defineSlop` overload uses Rust's metadata and window types while the SDK keeps
document/initial inference and category tuple ergonomics. A skin's imported URL is
`window.image`, as in the plan; its resolved resource key remains `roles.skin` in build
input. The package includes `src/wire`, so these types remain available after installation.

This defines input structure, not app acceptance. Descriptors, initial values and command
arguments remain untrusted data for the core's checks. The default legacy builder and
templates have not switched. Step 3 supplies typed acceptance, followed by the storage
and default-builder switch; these stages must not treat a TypeScript assertion as validation.

## Verification

- `bun run schema:generate` and contract drift checks pass without hand edits.
- `bun run verify` passed in 288.1 seconds: 150 Bun tests, 229 Rust tests (four skipped),
  TypeScript/templates and four installed-package checks (one skipped).
- The declaration type cases preserve initial-value inference and refuse unknown
  categories, missing skin images and standard-window flags on a skin, including flags
  supplied through a variable.
- `bun run verify native definition-build` passed in 13.9 seconds. Its real WebKit page
  renders emitted CSS/fonts and invokes a stripped UI stub through the restricted runner.
- The missing-image fixture was updated to mutate `window.image`; it again reaches the
  intended missing-emitted-asset refusal instead of failing on malformed TypeScript.

All changes remain uncommitted; completed checkpoints are staged at the user's request.
