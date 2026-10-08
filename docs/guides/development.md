# Development

## Set up the checkout

Use the Bun version in root `package.json` (currently 1.4.2), Xcode, and XcodeGen on macOS. The shipped app supports Apple silicon on macOS 15.2 or newer.

```sh
bun install --frozen-lockfile
bun install --cwd apps/landing --frozen-lockfile
bun run build
bun run verify --all --native
```

`build` generates platform contracts, builds the Rust core bindings (arm64 XCFramework and WASM) and the page shell (bundled with Vite), installs the shell into the app resources and the CLI, builds agent skills, and compiles the native helper and Rust document engine. The engine stays in its Cargo output directory; `HITSLOP_ENGINE` and `HITSLOP_NATIVE_CLI` independently select development tools. Run `build` before native tests. Template artwork is a separate, cached `bun run build:templates` step. Rust toolchain setup is described in [crates/README.md](../../crates/README.md).

Before building the complete app, run `bun run build:templates` to prepare its bundled resources. To work on the app, generate `apps/apple/hitSlop.xcodeproj` with `xcodegen generate --spec apps/apple/project.yml` and open it in Xcode. `bun run apple:build` builds and verifies a disposable development app under `generated/app`.

Every Xcode app configuration builds, embeds and signs `hitslop-evaluator`, which runs
named commands such as Quick Checklist's add, file, restore and remove actions. Debug
Run does not require `HITSLOP_EVALUATOR`; that override is for isolated tests. The native
renderer is embedded for Release and the packaged development artifact separately.

## Workspace responsibilities

| Area | Responsibility |
| --- | --- |
| `apps/apple` | macOS entry point, Quick Look extensions, project configuration, signing, and Sparkle |
| `crates` | `hitslop-core` (Rust on Loro) document semantics and the `.slop` file, its UniFFI and WASM adapters, and the CLI's `slop-engine` |
| `apps/apple/Packages/HitSlopApple` | Core, the native document owner (HitSlopDocument), Host, the app and catalog models (HitSlopFeatures), Catalog, telemetry, and NativeCLI |
| `packages/hitslop/src/sdk` | Author SDK: `defineDocument`, descriptors, handle and `ctx` types, and the Svelte adapter |
| `packages/hitslop/src/shell` | Page shell (private): snapshot store, typed handles, bindings, themes, and capture (no CRDT) |
| `crates/hitslop-core/src/wire`, `src/app` | Serde wire types and package-format acceptance, exported with ts-rs and UniFFI |
| `packages/hitslop/src/cli` | Scaffolding, checks, disposable preview, builds, registration, skills, and native forwarding |
| `examples/slops` | Active authored templates and the bundled selection |
| `apps/landing` | Website and public author documentation; independently locked dependencies |
| `scripts` | Build, verification, packaging, and release tooling |

The `hitslop` npm package is publishable. The repository root, examples workspace, and landing application are private. The package exports TypeScript source, consumed by Bun and the supported Vite/Svelte
authoring build; it does not ship a precompiled Node.js entry point. The installed-package
tests check SDK types outside the workspace. Apple implementation belongs in the app-local Swift package.

`slop dev SOURCE` serves the browser preview on `127.0.0.1` only. The native helper has no `open-dev` command.

## Add a template

Add one authored project directly under `examples/slops/`, in a folder named by its slug, including `slop.ts`, `schema.ts`, `App.svelte`, `styles.css`, and `tsconfig.json` (no `main.ts`). Use the current examples and [authoring guide](authoring.md). The examples workspace supplies shared dependencies; add a project package manifest if it needs its own dependencies.

Discovery scans immediate project directories with a `slop.ts`, without running it: a folder's name is its slug. Hidden directories, `archive`, `dist`, and `node_modules` are excluded. A folder name that is not a slug fails; the build checks the rest of `slop.ts`. Each project must pass its own Svelte/TypeScript check. A typical `tsconfig.json` extends `../../../tsconfig.json`, includes local TypeScript/Svelte files, and excludes `dist` and `node_modules`.

`bun run build:templates` produces `generated/templates/<slug>.slop` for every discovered project. Add its slug to `examples/slops/bundled.json` only when it should ship with the Mac app. This list is the sole bundled selection; duplicate or unknown selections fail. The generated inventory connects the build to app embedding and release verification. Rebuild after changing sources or selection.

Embedding replaces the entire generated StarterTemplates directory, so deselected templates disappear from the next app build. It never edits a user's installed templates or documents. The native catalog already discovers any valid local template and reads its categories from its app metadata.

Quick Checklist is the reference example; the other examples wait in `examples/archive` until they move to `slop.ts`. New templates need no edits to build loops. App-specific tests can remain schema-specific; generic release checks cannot assume fields such as `title`.

## Focused checks

`bun run verify` runs the tiers a change touches; `bun run verify TIER [args]` runs one.
[Testing](../testing.md#running-tests) lists every tier.

- `hygiene`: repository skills, generated-source checks, and tracked-artifact rules.
- `contracts` and `types` (`bun run check`): generated contract drift (change Rust source and regenerate rather than editing generated files), skills, package types, and discovered template types.
- `bun`: SDK and shell tests over WASM, examples, release tooling, and verification-runner tests.
- `cli`: non-native CLI integration tests; `bun run test` runs both `bun` and `cli`.
- `rust` (`bun run core:test`): the Rust suite with cargo-nextest.
- `swift` (`bun run swift:test`): native tests with two cached black-box apps and four presentation fixtures.
- `app`: complete macOS app build and bundle acceptance; included in `release:check`.
- `native` (`bun run test:native`): native CLI owners, the relocated helper, the native render of the fixtures (`HITSLOP_RENDER=all` for every bundled template), the crash matrix (with host death when `HITSLOP_APP_BINARY` names an app) and the corpus replay.
- `packed`: exact npm artifact dependency/type/init/check/preview verification, without native rendering. `HITSLOP_PACKED_NATIVE=1` adds the complete build/register/theme/export workflow.
- `landing` (`bun run landing:check`, `bun run landing:build`): public documentation and site validation.

Test locations, including `tests/release`, `tests/presentation`, `tests/verification`,
`tests/native` and `apps/landing`, are listed in [testing](../testing.md).

`bun run release:check` is the complete macOS gate; see [releasing](releasing.md). Direct `swift test --package-path apps/apple/Packages/HitSlopApple` is useful for focused work but explicitly skips presentation fixtures when their environment is absent.

`bun run shape:lab fixtures` builds and prints the standard, ellipse, glass and washer controls plus all six Shape Lab variants with dedicated and fallback exports. `bun run shape:lab open hole` opens a fresh writable vector-hole lab; `locked`, `radii`, `concave`, `rounded` and `washer` select the other variants. Open writable copies in the development app to inspect layout, toolbar dragging, focus, native clipping, and desktop click-through. These are test fixtures, not catalog entries. Automated tests verify dedicated exports ignore native masks and icons preserve transparency.

`bun run bench:windows` runs the opt-in window matrix. Startup diagnostics are described in [testing](../testing.md#native-macos). Performance measurements are not CI latency thresholds.

## Change discipline

Read AGENTS and the template's `slop.ts` first. Preserve contributor changes already in the worktree. Keep generated artifacts separate from authored source and inspect generated changes after building. Never alter historical compatibility fixtures or release hashes to make a check pass.

`archive/` (`archive/docs` and `archive/spikes` are tracked; the rest is local), `examples/archive/`, `_docs/`, and `_vibe/` material is not part of active contracts. Restore an old project only after updating its source to the current document API; there is no legacy document migration.
