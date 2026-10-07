# App-definition compiler gate

2026-10-06 · macOS arm64 · integrated in `ready-ship` as `27e5d6cc`

This is implementation step 1 of [app definition and packaging](../../plans/app-definition-and-packaging.md).
The checkpoint was verified in `app-definition-refactor`, then brought into the main
checkout and verified again against its own Rust build outputs. It is an internal
compiler entry point, not the default `slop build` yet. The source worktree is retired;
its branch remains available (see [consolidation](worktree-consolidation-2026-10-06.md)).

## What the gate exercises

`buildDefinition` runs two modes of the same Vite configuration. The UI mode compiles
real Svelte components. The definition mode substitutes frozen component tokens and
empty stylesheet modules, then evaluates its bundle in the existing restricted Rust
child. That child has a definition mode alongside its existing command mode, under
the same memory, input, output and execution limits.

The fixture includes component CSS, a font reachable only through `@import`, a CSS
image resolved through `$lib`, an unrendered PNG skin, an export component with an
image, an artwork-only PNG, a shared document module and a command. Its component
module throws if evaluated without a browser, proving that definition evaluation
does not execute the component. Static `new URL(..., import.meta.url).href` and an
eager `import.meta.glob` are exercised in a second build.

The built UI is **127,531 bytes** before compression with the current SDK and Svelte
5.57.1, using Vite 8.3.1. The fixture produces seven packaged resources plus its
separate initial artwork. This is not a final size budget: the fixture still uses
the existing TypeBox command-argument API until the descriptor migration.

The UI AST transform removes the command's executable body. A body-only sentinel
is absent from `ui.js` and present in `commands.js`. A WebKit test mounts the real
bundle, verifies its component styling and loaded font, clicks the command callable,
and forwards its `{name, args}` request to the restricted Rust child. The returned
result and set intent are checked. This proves the stub/runner boundary; the test
does not substitute for the later owner persistence and stale-retry tests.

Failure cases cover an unresolved skin, a command importing Svelte, an entry cycle,
DOM access during initialization, ambient time, infinite initialization, an async
command, and nonexistent resource references in both the declaration and CSS.
Rebuilding produces identical declarations, role keys and resource inventories.

The consolidation adds two regression cases. A declaration without commands is still
evaluated in the restricted child, but has no command role or packaged `commands.js`.
A nonempty `public/` directory fails with explicit-import guidance; an empty directory
is allowed. Both cases failed for their intended reason before the compiler fix.

## Findings incorporated into the plan

- Read the completed Vite build output. A pre-plugin `generateBundle` hook misses
  CSS emitted by later plugins.
- Vite can emit an asset whose last JavaScript use was removed by tree shaking.
  Project the UI declaration to its UI fields and use Vite's `importedAssets`
  dependency metadata to identify artwork-only leftovers. The converse is tested:
  artwork reused by CSS is also retained as an app resource.
- Empty stylesheets must resolve to virtual JavaScript modules; returning JavaScript
  source under a `.css` module ID makes Vite parse it as CSS.
- Headless asset URLs need no URL polyfill or alternate resolver. After Vite resolves
  a static URL, an AST transform retains the emitted package path.
- Collect module IDs and the emitted assets' original source names, including fonts
  and static URL assets, for definition invalidation. Vite's dev server will keep
  ownership of CSS dependency watching and UI HMR; Rolldown's build plugin context
  does not expose Rollup's `getWatchFiles` API.
- The entry uses a named `defineSlop` import and a default call with explicit object
  fields. Values can be computed or imported; root spreads, computed field names and
  indirect calls are refused. Commands use an explicit inline synchronous `run`.

## Remaining migration

`BuildInput` is provisionally typed in TypeScript. Rust will own it at the next
checkpoint. The legacy default builder, TypeBox arguments, generated contracts,
storage layout, callback ABI and WASM dev path still exist. The new compiler must
not become the default before Rust acceptance, descriptor arguments, packing and
the owner-routed context land. No markers have changed; no frozen corpus was edited.

Linux remains deferred. The separate native-dev feasibility evidence is in
[native-dev-owner.md](native-dev-owner.md).

## Verification

- `bun run verify` on `ready-ship`: passed; 219 Rust tests (4 skipped), 152 Bun tests, 4 packed-package
  tests (1 skipped), hygiene, contract drift, TypeScript/template checks and landing
  checks. The full consolidation run passed in 183.5 seconds, including a cold Rust
  test build. The release engine was rebuilt from the main checkout's sources.
- Focused compiler and WebKit compiler/stub/runner gates: 5 tests, 46 assertions,
  passed together. WebKit mounts the actual bundle and evaluates a stub request in
  the native restricted runner.
- No Swift or FFI surface changed in this checkpoint. The full native migration gate
  remains required when those surfaces switch; Linux remains deferred.
