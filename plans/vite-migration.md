# Vite migration plan

Status: proposed implementation, following the completed evaluation on September 29, 2026. **The shipping compiler remains esbuild. This document does not perform the migration.**

## Goal and decision

Move authored-app development and production compilation to one Vite pipeline using the official Svelte plugin. Preserve hitSlop's CLI, package writer, native ownership, page lifecycle and app-module boundary. The main benefits are state-preserving hot updates, smaller app assets and upstream maintenance of Svelte integration.

Use the evaluated versions initially: Vite **8.3.1**, `@sveltejs/vite-plugin-svelte` **7.3.1**, Svelte **5.57.1** and Bun **1.4.2**. Treat subsequent upgrades as separate changes. Do not adopt `svelte-bundle`, SvelteKit, SSR or single-file HTML packaging. Do not introduce a permanent compiler-selection flag.

Evidence and reusable implementation:

- [Evaluation report](../spikes/vite-evaluation/README.md)
- [Recorded measurements](../spikes/vite-evaluation/measurements.json)
- [Vite compiler adapter](../spikes/vite-evaluation/vite.ts)
- [Development server adapter](../spikes/vite-evaluation/dev.ts)
- [Browser evaluation](../spikes/vite-evaluation/evaluate.ts) and [native export evaluation](../spikes/vite-evaluation/native.ts)

## What we discovered

### Measured behavior

Five samples per measurement, on an Apple M1 running Darwin 25.6.0. Times below are medians. The machine was shared with other workspace work; production and native timings were noisy and are not controlled speedup estimates.

| Measurement | esbuild | Vite |
| --- | --- | --- |
| Quick Checklist fresh-output build | 16.90 s | 6.80 s |
| Quick Checklist repeat-output build | 16.17 s | 11.44 s |
| Fixture first visible preview | 4.03 s | 5.74 s |
| Fixture edit to visible update | 5.71 s, including restart | 94 ms, using HMR |
| Quick Checklist JavaScript | 233,864 bytes | 173,909 bytes |
| Quick Checklist CSS | 10,355 bytes | 10,417 bytes |
| Package without Quick Look | 248,866 bytes | 188,973 bytes |
| Native PNG export | 4.35 s | 1.32 s |
| Native PDF export | 3.18 s | 1.11 s |

Combined JS/CSS was **24.5% smaller** with Vite. Initial preview startup was slower. The most useful improvement was the editing loop, not a universal startup/build performance win.

The existing CLI builds once and serves that package; reloading alone does not rebuild changed source. Its measured edit loop therefore includes stopping the server, rebuilding and navigating again. Vite's edit measurement ends when the changed heading is visible. Fresh-output builds used fresh processes/output directories, not cold OS disk caches. Later Vite development starts reused its optimizer cache.

### Proven in the spike

- All **five existing build tests** and **three native build/capture tests** passed with Vite substituted through the internal compiler seam.
- Five component updates, scoped CSS updates and syntax-error recovery retained accepted text, row IDs and the same public document ID. One UI remained mounted, and a later insert added exactly one row.
- Parent/child document context, rune modules, an external Svelte component and custom `main.ts` worked in the browser fixture. Quick Checklist mounted as a black-box trial.
- Initial-data changes reset the disposable preview to a new document with the new seed.
- Both candidates exported PNG/PDF five times per format and reopened with unchanged document state. Their final PNG exports were byte-identical at 88,579 bytes and visually inspected. This is evidence, not a new pixel-output contract.
- Build/preview execution succeeded with Node absent from child processes' PATH. This establishes compatibility for the pinned Bun/toolchain combination, not a general upstream Bun-support guarantee.

### Integration details that mattered

1. **Keep module output.** The host owns the page and calls the app's default `mount(ctx, target)`. A standalone HTML bundle adds the wrong packaging boundary.
2. **Use normal package resolution with Svelte deduplication.** A naive alias to the Svelte package directory broke exported subpaths. `resolve.dedupe: ["svelte"]` worked and kept the SDK and components on one runtime.
3. **Force production environment in the build worker.** `bun test` supplies `NODE_ENV=test`; without an explicit production setting, Vite emitted Svelte development instrumentation and checkout paths. Content-based CSS hashing also preserves portable output.
4. **Keep asset emission explicit.** The spike uses a programmatic module build rather than library mode, avoiding library mode's automatic asset inlining. Copied font URLs, nonduplicated font files and license files passed the existing tests.
5. **Relocated test projects need valid configuration.** Quick Checklist's relative tsconfig inheritance broke after copying it to a temporary directory. Vite rejected the nonexistent config; esbuild tolerated it. The shared fixture-copy helper now resolves that inheritance against the original source. Assertions were retained.
6. **Library import shape affects the comparison.** An initial fixture imported the entire icon-library barrel and exceeded the baseline startup deadline. The final fixture uses direct icon subpaths, matching Quick Checklist. Broad barrel imports are not proven inexpensive.
7. **Development still needs a host adapter.** Vite does not replace package metadata generation, the page shell, the Rust WASM owner or native capture. Its maintenance benefit is upstream Svelte/HMR integration, not eliminating all custom code.
8. **Dependencies increase.** The isolated Vite/plugin/Svelte install added 40 installed packages and occupied about 37 MiB, including shared Svelte dependencies. This is not an incremental CLI-size estimate. These dependencies must never enter built slops.

## Migration steps

### 1. Establish the current baseline

- [ ] Read the current engineering, versioning and testing contracts before changing code. The workspace underwent a separate platform refactor during evaluation; the spike follows `shellDirectory` and `/__shell__/`. Do not copy unrelated deletions or contract changes from the working-tree diff into this migration.
- [ ] Run the existing checks serially and classify failures before switching the default. Preserve failures and timings as evidence; do not increase deadlines or remove assertions to obtain a green comparison.
- [ ] Retain the current package writer and its staging/publication behavior. The evaluation introduced an internal `AppCompiler(source, stage)` callback; use this boundary to replace compilation without duplicating packaging.

### 2. Promote production compilation

- [ ] Add the pinned Vite and Svelte-plugin dependencies to `@hitslop/cli` and the repository lockfile. Keep the established Svelte version shared across compiler, SDK and authored components.
- [ ] Move the compiler adapter into the CLI and make it the sole authored-app compiler after the acceptance gates pass. Keep fresh Bun worker execution for schema, initial-data and theme evaluation.
- [ ] Set production `NODE_ENV` before constructing Vite/plugin configuration, including when launched by tests. Retain `HITSLOP_DEBUG_BUILD` as the existing minification control.
- [ ] Preserve the generated `App.svelte`/`defineSlop` entry and custom `main.ts` support. Emit one ES module at `assets/app.js` with its default export preserved, one `assets/app.css` file (empty when needed), Safari 17 output and local assets. Disable JS and CSS splitting for this migration.
- [ ] Retain content-based CSS hashes, Svelte deduplication, copied asset paths, font URL encoding and license preservation. Keep project Vite/Svelte config loading and automatic `.env` loading disabled, as in the evaluated centrally controlled configuration.
- [ ] Run the existing input-graph and emitted-output audits. Preserve rejection of embedded engines, private bridge access and remote resources required to boot. Keep manifest/schema/initial/theme writing and native capture outside the compiler.
- [ ] Remove the custom esbuild Svelte plugin and authored-app esbuild configuration once the replacement is proven. Keep esbuild where repository tooling or page-shell builds still use it; this is not a repository-wide bundler replacement.

### 3. Promote the development server

- [ ] Keep the public `slop dev SOURCE --port` interface and preview frame. Serve the host-owned page shell and Rust WASM core as host resources, while Vite handles authored modules and HMR.
- [ ] Use the same entry resolution, Svelte deduplication, asset policy and app-boundary checks as production. Select the dependency optimizer entry from the actual app entry; the spike's unconditional `App.svelte` entry must be corrected for plain-JavaScript projects.
- [ ] Apply the loopback WebSocket allowance only to the development CSP. Preserve native/production CSP and bind development serving to loopback. Replace the spike's repository-root filesystem allowance with the authored project and the resolved SDK/dependency locations needed by installed CLI projects.
- [ ] Preserve the current owner and accepted document state for component/CSS HMR. Do not boot a second owner or modify the runtime ABI for hot updates. Do not promise transient component state or unaccepted draft preservation beyond verified behavior.
- [ ] Reevaluate schema, initial data, theme and manifest in fresh workers and deliberately reset disposable preview state when those inputs change. Serialize metadata rebuilds; failed rebuilds must remain recoverable after the next edit.
- [ ] Observe imported dependencies of metadata entrypoints as well as their top-level files. Handle addition/removal of `main.ts` and `styles.css` by rebuilding the entry configuration and performing a full preview reload.
- [ ] Preserve visible compilation diagnostics and recovery. Clean up the Vite server, pending rebuild workers and temporary metadata on startup failure, SIGINT and SIGTERM. The spike's cleanup is not complete enough to promote verbatim.
- [ ] Keep optimizer caches outside `.slop` packages and immutable templates. Test cache behavior and path resolution from an installed CLI outside the monorepo.

### 4. Integrate tests and documentation

- [ ] Run the existing build/native suites directly against the new default; remove reliance on the spike preload adapter for acceptance. Keep the corrected source-fixture copy helper.
- [ ] Promote the dedicated browser fixture and behavioral HMR scenarios into the appropriate authoring integration suite. Keep Quick Checklist black-box; do not make its business logic a platform oracle.
- [ ] Update template-cache inputs so compiler implementation, Vite/plugin versions, relevant options and SDK changes invalidate compiled artifacts. Retain package-content and failed-publication protections.
- [ ] Update CLI/authoring documentation and generated skills: `slop dev` watches changes, component/CSS updates preserve accepted state, and metadata changes reset disposable state. Rebuild generated guides through the existing skill tooling.
- [ ] Preserve the evaluation report and measurements as historical evidence. Retire its production-path preload/worker duplication after the real CLI owns those paths; do not ship evaluation dependencies or scripts in CLI packages.

## Acceptance and remaining gaps

Before adding cases, name the observable failure, independent expected result and existing coverage gap. Extend the owning suites; record changed contracts in the five-column test ledger. Before replacing consequential coverage, demonstrate that the retained owner catches the protected failure.

| Area | Required proof |
| --- | --- |
| Production package | Existing build cases pass: forbidden imports/bridge/remote boot, fresh source evaluation, output paths, fonts/licenses and portable bytes across checkout locations. Add only missing cases for dynamic imports and emitted non-font assets. |
| Native integration | Existing native build/capture cases pass, including shared document context and failure-safe master replacement. PNG/PDF export and reopen preserve document state. No browser engine enters native app bundles. |
| HMR | Accepted title, row IDs and document ID survive repeated component/CSS edits; one UI remains; one subsequent insert adds one row; syntax-error overlay recovers without losing accepted data. |
| Reset and configuration | Schema/initial/theme/manifest changes, including imported metadata dependencies, reset disposable state correctly. Invalid metadata recovers after correction. Adding/removing entry/style files updates the preview. |
| Lifecycle | Missing shell, invalid source, occupied port and termination clean up temporary state/workers. No duplicate owners or accumulating event handlers across repeated updates/restarts. |
| Installed CLI | Packed install outside the checkout builds and previews with Bun and no Node on PATH. SDK/dependency resolution, filesystem allowances, plain-JS custom entry and assets work without repository-relative assumptions. |
| Cache and distribution | Relevant toolchain changes invalidate the template cache; packages contain no source, dependencies, optimizer cache or mutable state. Public CLI commands and authoring source layout remain compatible. |

Run the affected owner tests first, then the required repository tiers: `bun run check`, `bun run test`, `bun run build`, `bun run swift:test`, `bun run test:native`, and the packed-consumer checks. Run `bun run release:check` before distribution. Use the current release/versioning policy for publication; a bundler change does not itself authorize native ABI/storage changes, resealing historical artifacts or publishing packages.

Known evaluation limits: IME/unaccepted drafts during HMR, exhaustive third-party component-library compatibility, production source maps, imported metadata dependency watching and packaged CLI integration were not fully proven. Preserve current production source-map behavior; no new source-map distribution policy is part of this migration. Handle these gaps through the acceptance cases above rather than assuming the spike is production-ready.

The evaluation's repository type/Svelte check passed. Its full test run reported **46 passed, 8 failed and 4 errors** during concurrent platform work: six timeouts and two cache/manifest assertions. The focused esbuild suites also recorded timeout failures. The Vite candidate's eight passing focused cases and browser/native-export checks do not establish a clean release gate or prove the cause of the baseline failures. Reestablish a green, attributable baseline before switching the shipping compiler.

## Completion and rollback

Migration is complete when Vite is the sole authored-app compiler and development server, the existing CLI/package/owner contracts remain intact, the required gates pass, and installed consumers work without Node or checkout assumptions. Report repeatable performance measurements, but do not make hardware-dependent timings or byte-identical rendered images mandatory test thresholds.

Make the compiler switch reversible as a coherent code change. If acceptance fails, retain or restore the previous default and investigate the failing boundary. Do not maintain two shipping compiler modes, rewrite documents, or migrate stored state to work around a bundling regression.
