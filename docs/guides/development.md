# Development

## Set up the checkout

Use the Bun version in root `package.json` (currently 1.4.2), Xcode, and XcodeGen on macOS. The shipped app supports Apple silicon on macOS 15.2 or newer.

```sh
bun install --frozen-lockfile
bun install --cwd apps/landing --frozen-lockfile
bun run build
bun run check
bun run test
bun run swift:test
```

`build` generates platform contracts, builds the Rust core bindings (native XCFramework and WASM) and the page shell, installs the shell into the app resources and the CLI, builds agent skills, and compiles the native helper. Run `build` before native tests. Template artwork is a separate, cached `bun run build:templates` step. Rust toolchain setup is described in [crates/README.md](../../crates/README.md).

Before building the complete app, run `bun run build:templates` to prepare its bundled resources. To work on the app, generate `apps/apple/hitSlop.xcodeproj` with `xcodegen generate --spec apps/apple/project.yml` and open it in Xcode. `bun run apple:build` builds and verifies a disposable development app under `generated/v1/app`.

## Workspace responsibilities

| Area | Responsibility |
| --- | --- |
| `apps/apple` | macOS entry point, project configuration, signing, and Sparkle |
| `crates` | `hitslop-core` (Rust on Loro) document semantics, plus its UniFFI and WASM adapters |
| `apps/apple/Packages/HitSlopApple` | Core, the native document owner (HitSlopDocument), Runtime, Host, TCA Features, Catalog, telemetry, and NativeCLI |
| `packages/document` | Page shell and SDK: snapshot store, typed handles, bindings, Svelte adapter, themes, and capture (no CRDT) |
| `packages/schema` | TypeBox manifest, bridge, owner and socket contracts |
| `packages/cli` | Scaffolding, checks, disposable preview, builds, registration, skills, and native forwarding |
| `examples/slops` | Active authored templates and the bundled selection |
| `apps/landing` | Website and public author documentation; independently locked dependencies |
| `scripts/v1` | Current build, verification, packaging, and release tooling |
| `deferred` | Historical code and future-feature scaffolding, excluded from active builds |

The three npm packages are publishable. The repository root, examples workspace, and landing application are private. TypeScript belongs in packages; Apple implementation belongs in the app-local Swift package.

`slop dev SOURCE` serves the browser preview on `127.0.0.1` only. The native helper has no `open-dev` command. Its `storage-probe` command and storage phase hooks exist only in debug builds for the crash matrix.

## Add a template

Add one authored project directly under `examples/slops/`, including `manifest.json`, `schema.ts`, `initial.ts`, `theme.ts`, `App.svelte`, `styles.css`, and `tsconfig.json` (no `main.ts`). Use the current examples and [authoring guide](authoring.md). The examples workspace supplies shared dependencies; add a project package manifest if it needs its own dependencies.

Discovery scans immediate project directories with manifests. Hidden directories, `archive`, `dist`, and `node_modules` are excluded. Invalid manifests and duplicate slugs fail. Each project must pass its own Svelte/TypeScript check. A typical `tsconfig.json` extends `../../../tsconfig.v1.json`, includes local TypeScript/Svelte files, and excludes `dist` and `node_modules`.

`bun run build:templates` produces `generated/v1/templates/<slug>.slop` for every discovered project. Add its slug to `examples/slops/bundled.json` only when it should ship with the Mac app. This list is the sole bundled selection; duplicate or unknown selections fail. The generated inventory connects the build to app embedding and release verification. Rebuild after changing sources or selection.

Embedding replaces the entire generated StarterTemplates directory, so deselected templates disappear from the next app build. It never edits a user's installed templates or documents. The native catalog already discovers any valid local template and derives categories from its manifest.

Quick Checklist and Small Expenses are current examples and deliberate fixtures for mutation/storage tests, not a limit on collection size. New templates need no edits to build loops. App-specific tests can remain schema-specific; generic release checks cannot assume fields such as `title`.

## Focused checks

- `bun run hygiene`: repository skills, generated-source checks, and tracked-artifact rules.
- `bun run schema:check`: generated contract drift; change TypeBox source and regenerate rather than editing generated files.
- `bun run check`: generated contracts, skills, package types, and discovered template types.
- `bun run test`: SDK, schema, and CLI tests over the WASM core, including the shared fixture replay.
- `bun run swift:test`: native tests with two cached black-box apps and three presentation fixtures.
- `bun run test:native`: native CLI owners. `bun run test:render` checks the full built template/preserved-package corpus; `--fixtures` limits it to the native fixtures and contract specimens.
- `bun run test:storage` and `bun run test:native-crash`: commit-phase and native-process crash probes.
- `bun run packages:pack` and `bun run test:packed`: exact npm artifact dependency/type/init/check/preview verification, without native rendering. Add `--native` to the packed check for the complete build/register/theme/export workflow.
- `bun run landing:check` and `bun run landing:build`: public documentation and site validation.

`bun run test:local` is the complete macOS gate; see [releasing](releasing.md). Direct `swift test --package-path apps/apple/Packages/HitSlopApple` is useful for focused work but explicitly skips presentation fixtures when their environment is absent.

`bun run presentation:fixtures` prints generated standard, ellipse, and washer package paths. Open writable copies in the development app to inspect layout, toolbar dragging, focus, native clipping, and desktop click-through. These are test fixtures, not catalog entries. Automated tests verify dedicated exports ignore native masks and icons preserve transparency.

`bun run bench:windows` runs the opt-in window matrix. Startup diagnostics and their opt-in test are described in the [runtime reference](../reference/runtime.md#opening-and-recovery). Performance measurements are not CI latency thresholds.

## Change discipline

Read AGENTS and the template manifest first. Preserve contributor changes already in the worktree. Keep generated artifacts separate from authored source and inspect generated changes after building. Never alter historical compatibility fixtures or release hashes to make a check pass.

The Bun SQLite and flock adapters in `packages/document/test-support` are crash/format fixtures, excluded from the published npm package; production ownership and persistence stay in Swift. They are not fallback document engines. Local ignored `archive/`, `examples/archive/`, `_docs/`, and `_vibe/` material is not part of active contracts. Restore an old project only after updating its source to v1; there is no legacy document migration.
