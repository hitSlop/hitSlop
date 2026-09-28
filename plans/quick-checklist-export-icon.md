# Quick Checklist: export/icon views and the Svelte SDK

Status: proposed, 2026-09-29. Nothing implemented.

This plan comes from a review of `examples/slops/quick-checklist` and `@hitslop/document/svelte`. It covers how export and icon views are defined, and how the slop uses the SDK.

## Sequencing constraint

`App.svelte`, `styles.css`, `packages/document/src/owner/document.ts`, `packages/document/src/owner/text-binding.ts` and `BenchmarkTests.swift` carry uncommitted perf work. `examples/slops/_bench5k/` is an untracked copy of an earlier `App.svelte`. `App.svelte` changed while this review was being written.

- This plan cites symbols, not line numbers, for those files.
- Steps 2, 3 and 4 wait for the perf work to land.
- Step 1 touches only `packages/cli` and can go first.

## Findings

All were checked in code.

### Capture and export

- **Snippets can't see the capture mode.**
  - Swift calls `window.__hitslopCapture.begin(token, "preview" | "export")` (`SlopRenderer.capture`).
  - `capture.begin` forwards the mode to `onPrepare` handlers, but calls `target.prepare()` with no argument.
  - `CaptureTarget.svelte` renders `content()` with no argument.
- **Previews are cropped after a full render.**
  - Native limits a preview to at most 3:1 portrait, or the window height (`SlopRenderer.capture`, `previewHeight`).
  - The export snippet still renders every row first.
  - `content-visibility` is switched off during capture (`html[data-slop-capture] .checklist-row`).
  - The cost at 1k or 5k rows is unmeasured.
- **PNG export has hard limits.**
  - PNG export rejects over 16384px per side or 24 megapixels, with "export as PDF for longer documents" (`SlopRenderer.validateSize`).
  - At 2× that is roughly 130 checklist rows.
  - Export is meant to be full-length, so this is not a defect.

### Slop structure

- **Export re-implements the editor and reuses its layout classes.**
  - `.checklist-shell` and `.checklist-paper` (a 5-row grid) are reused with 2 children.
  - About 26 lines under `html[data-slop-capture="static"]` then undo them.
  - The checkbox is drawn twice: `.checklist-box` in the editor, and leftover Bits-style `[data-checkbox-root]` and `[data-state="checked"]` rules used only by export.
  - The progress bar is drawn twice (`transform` in the editor, `width` in export).
  - Inactive editor rows now render as plain text, near-identical to export rows.
- **One 300-line file holds editor, export and icon.** `exported`, `exportFinished` and `marks` sit in the editor script and depend on `activeView`.
- **Icon art is hand-sized.**
  - 456px inside the 512px stage here.
  - The sealed meeting-notes and resume specimens use 464px.
  - Only three specimens were read.
- **Dead bits.**
  - `data-slop-selection="none"` is read by nothing; `docs/HostOwnedReset.md` §5.4 lists it as set-but-ignored.
  - `.checklist-error` is unused.
  - `{ message }` on `change` is ignored by the runtime (`_options`).

### Authoring model

- **A missing icon is silent.**
  - `packages/cli/src/template.ts` runs the native `screenshot --target icon --if-present`.
  - With no icon view the native CLI returns exit 0, writes nothing and prints nothing (`NativeCLI.swift`, `Screenshot.run`).
  - `template.ts`'s `run()` discards stdout, so `Icon.png` is just absent.
- **`<Slop>` is required boilerplate.**
  - It supplies the render-error boundary, the `data-hitslop-root` element the host sizes to fill the window, and capture-target registration.
  - `useDocument` works without it, so forgetting it degrades quietly.
- **Snippet placement is positional.**
  - `exportView` and `icon` only work when declared inside `<Slop>` with exact names.
  - A correctly named snippet declared outside `<Slop>` is an ordinary local snippet and is never passed to it.

## Steps

### 1. `slop register`: warn when there is no icon view

**Change**
- After the icon `screenshot` call in `buildTemplate` (`packages/cli/src/template.ts`), check whether `QuickLook/Icon.png` exists.
- If not, print one line: no icon view, Finder will show the generic icon.
- Find the CLI's existing warning channel first (`packages/cli/src/authoring.ts`, `app.ts`). Do not invent one.

**Evaluate**
- Extend `packages/cli/tests/build.native.test.ts`, which already iterates `["Preview", "Icon"]` for a fixture that defines an icon.
- Add a case with no icon view. Assert the notice is emitted and `Icon.png` is absent.
- Keep the existing icon case and assert no notice.

### 2. Measure preview capture

**Change**
- Add an env-gated cell to `apps/apple/Packages/HitSlopApple/Tests/HitSlopHostTests/BenchmarkTests.swift` (`HITSLOP_BENCH=1`), inside the existing `callAsyncJavaScript` block, after the edit timings and before the final flush.
- Sketch:

```js
const t = performance.now();
const m = await window.__hitslopCapture.begin("bench-preview", "preview");
const beginMS = performance.now() - t;
await window.__hitslopCapture.restore("bench-preview");
return { beginMS, height: m.height, dedicated: m.dedicated };
```

- Record `preview_capture_ms` and `export_height_px` per `HITSLOP_BENCH_ROWS` (1000 and 5000) in the existing evidence JSON.
- The number is a lower bound. It covers mounting the export snippet, fonts and the stable-layout polls. It excludes `SlopRenderer`'s resize loop (up to four iterations) and PNG encoding.

**Decision rule**
- Proposed bar: ≤ 500 ms at 1k rows and ≤ 1.5 s at 5k, because the preview refresh runs at close. Set the numbers before running.
- Pass: do nothing further.
- Fail: add a `{ mode }` parameter to the export snippet.
  - `Slop.svelte` already receives `mode` in its `onPrepare` handler.
  - It stores the mode in `$state`, and `CaptureTarget` renders `content({ mode })`.
  - `exportView` is typed `Snippet<[{ mode: "preview" | "export" }]>`.
  - No runtime change is needed, because `onPrepare` runs before `target.prepare()`.
  - The slop renders about 30 rows when `mode === "preview"`, enough to fill the 3:1 crop.
  - Confirm zero-argument snippets still type-check with `bun run check`.

### 3. Split the slop

Behavior-preserving.

**Change**
- Extract `Export.svelte` and `Icon.svelte`. Use these names now so step 4 needs no rename.
  - `Export.svelte` takes title, tasks and view.
  - `Icon.svelte` takes marks.
  - For now the `<Slop>` snippets just render them.
- Give export its own root class, such as `.checklist-export`.
- Share one static `TaskRow` between the editor's inactive rows and export, plus `Mark`, `Brand` and `Progress`.
- Delete the `html[data-slop-capture="static"]` overrides, the `[data-checkbox-root]` and `[data-state="checked"]` rules, `data-slop-selection`, `.checklist-error` and `{ message }`.

**Evaluate**
- No change is the pass.
  - Build before and after, then compare `QuickLook/Preview.png` and `Icon.png` pixel for pixel.
  - Compare PNG and PDF export of the same fixture document.
  - Native presentation tests stay green.
- Track:
  - the static-override CSS lines (about 26 → 0);
  - the `App.svelte` line count.

### 4. `defineSlop(App, { export, icon })` replaces the snippet form

Replace what we teach and generate. `<Slop>` stays exported as a low-level primitive.

**SDK** (`packages/document/src/app/store.svelte.ts`, new internal `Root.svelte`)

```ts
export function defineSlop(
  App: Component,
  options: { export?: Component; icon?: Component } = {},
): SlopApp
```

- With options, `mount` renders `Root.svelte`. Without options it mounts `App` bare, as today.
- Sketch of `Root.svelte`:

```svelte
<script lang="ts">
  import type { Component } from "svelte";
  import Slop from "./Slop.svelte";
  let { App, Export, Icon }: { App: Component; Export?: Component; Icon?: Component } = $props();
</script>
{#snippet exportView()}<Export />{/snippet}
{#snippet iconView()}<Icon />{/snippet}
<Slop exportView={Export ? exportView : undefined} icon={Icon ? iconView : undefined}>
  <App />
</Slop>
```

- `Slop.svelte` and `CaptureTarget.svelte` are unchanged.
- If step 2 required `{ mode }`, `Export` receives it as a prop. Otherwise there is no `mode` prop.
- If `App` also renders `<Slop>` and options are given, both register an export target and the runtime throws "Expected one export capture target". Prefer an explicit error at mount.

**Build** (`packages/cli/src/build.ts`)
- The entry is currently synthesized in stdin as `import "./styles.css"; import App from "./App.svelte"; import { defineSlop } ...; export default defineSlop(App);`.
- Detect `Export.svelte` and `Icon.svelte` the way `styles.css` is detected.
- Use an exact-case directory listing. macOS's case-insensitive filesystem would otherwise make detection platform-dependent.
- Emit:

```ts
import "./styles.css";
import App from "./App.svelte";
import Export from "./Export.svelte";  // only when present
import Icon from "./Icon.svelte";      // only when present
import { defineSlop } from "@hitslop/document/svelte";
export default defineSlop(App, { export: Export, icon: Icon });
```

- `main.ts` authors call `defineSlop(App, { ... })` explicitly.
- `slop dev` builds through `packages/cli/src/authoring.ts`. Confirm it shares this entry synthesis.

**Shared UI state**
- Editor state the export needs moves to a small module:

```ts
// ui.svelte.ts
export const ui = $state<{ view: "tasks" | "filed" }>({ view: "tasks" });
```

- The Tabs bind to `ui.view`, and `Export.svelte` reads it.
- This is safe because `packages/document/src/app/context.ts` already assumes one slop per page.
- Closed exports still start in "To do", as `docs/reference/runtime.md` documents.

**Docs, skills and templates** (same change as the API)
- 6 landing pages under `apps/landing/src/content/docs/docs/` (`getting-started`, `guides/data-and-schemas`, `guides/icons-and-exports`, `guides/manifest-and-windows`, `guides/png-window-skins`, `guides/styling`).
- `docs/guides/authoring.md`, `docs/reference/runtime.md`, `README.md`.
- `packages/cli/skills/hitslop-authoring/SKILL.md`, `hitslop-design/SKILL.md`, `hitslop-design/references/bits-ui-styling.md`, `hitslop-design/references/presentation-and-export.md`.
- `packages/cli/templates/checklist/App.svelte`. Decide whether the starter gets an `Icon.svelte` stub, which pairs with the step 1 warning.
- Do not edit the historical plans (`HostOwnedReset`, `LoroHostPlan`, `LoroRustCutover`, `NextPhasePlan`).

**Fixtures**
- `tests/abi/svelte`, `tests/abi/owner-svelte`, `packages/cli/tests/fixtures/presentation` and the inline sources in `build.native.test.ts` use the snippet form. They keep building unchanged.
- The export and icon failure-injection tests stay valid, because `Root.svelte` uses the same `<Slop>` boundaries.

**Why it is better**
1. **One owner for the page contract.** `defineSlop` already owns `mount`. It becomes the single place that applies the boundary, root marker and capture registration, so no slop can forget or double-wrap `<Slop>`.
2. **Structural, not positional.** With files, presence is a directory check, and step 1's warning reports it. A misplaced snippet is no longer a silent failure.
3. **Explicit lifecycle.** `App.svelte` is interactive. `Export.svelte` and `Icon.svelte` are capture-only. Agents follow a three-file rule more reliably than "declare snippets inside `<Slop>`".
4. **Export is a function of document, shared UI state and mode.** No closure over editor locals.
5. **Enables a cleaner internal design later.** Export and icon could mount as separate Svelte roots on demand, with no DOM reparenting to `document.body` and no `hidden` toggling in `CaptureTarget`.

**What it does not buy, and the costs**
- No runtime, perf or dev-preview gain by itself.
- The missing-icon guardrail comes from step 1, not from this step.
- Export state needs a shared module instead of a closure.
- About 15 files of prose change.
- Detection by file name is convention, not configuration.

**Evaluate**
- Extend `build.native.test.ts` with a fixture project that has `Export.svelte` and `Icon.svelte`.
  - Assert Preview and Icon renders come from the dedicated views.
  - Assert failure injection still rejects only that capture.
- Assert a project without those files behaves as today. `Icon.png` is absent, and the step 1 notice fires.
- After step 3, record the `<Slop>` ceremony left in `App.svelte`. Confirm this step removes it without the new files growing.

## Not committed

- **Handled write promises.**
  - `OwnerDocument.enqueue` (`packages/document/src/owner/document.ts`) reports a failure and rethrows.
  - The page forwards `unhandledrejection` to native as `runtimeError` with kind `application` (`DocumentSession.swift`), and native treats `application` and `operation` differently.
  - So an unawaited rejected write becomes an authored-exception report, for example "Document barrier is active" during close or capture.
  - `App.svelte` and `packages/cli/templates/checklist/App.svelte` each carry 5 pure-swallow catches.
  - Fix: mark the promises returned from `enqueue`, `submit` and `change` as handled, with a Bun owner-boundary test in `packages/document/tests`.
  - Left out by choice. The finding stands.
- **Dropping the dead `message` option** from the Svelte-facing `change` type. Step 3 removes the one call.
- **Icon stage constants** as CSS custom properties. Skip unless more slops need icons.
- **Textarea auto-grow in the SDK.** The mirror trick is right on macOS 15.2, which has no `field-sizing` (`docs/HostOwnedReset.md` §7).
- **Live vs closed export differing by selected tab.** Documented in `docs/reference/runtime.md`.
- **Per-binding O(N) subscriptions.** Already planned in `docs/HostOwnedReset.md` §4.3.

## Open decisions

- The preview capture thresholds in step 2.
- The warning channel for step 1, once the CLI's existing one is found.
- Whether the starter template gets an `Icon.svelte` stub.
- Whether `Export` needs a `mode` prop (depends on step 2).

## Verification

- **Everyday:** `bun run check && bun run test` after each step.
- **Native:** `bun run build && bun run swift:test && bun run test:native` for anything touching capture, export or icon.
- **Manual:** `bun slop dev examples/slops/quick-checklist`, then register, create a writable copy, and export PNG and PDF. Inspect editor, export and icon from the same revision.
- **Coverage rule (`AGENTS.md`):** before adding a test, name the observable failure, the independent expected result and the gap. For any test that replaces coverage, break the protected behavior first and confirm it fails.
