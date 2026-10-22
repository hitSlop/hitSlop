---
name: hitslop-authoring
description: Create, preview, validate, build, and register hitSlop authoring projects with the TypeScript CLI. Use for slop.ts metadata, storage choices, package boundaries, capture, identity, and release workflow.
---

# Author local mini apps

**Do not create an automated test or test suite for each slop.** Creating, styling,
animating, or updating a slop does not require new tests. Use the existing check/build
commands and hands-on preview/export review. Shared SDK, storage, or host regressions
belong in existing tests at their owning boundary; do not duplicate that coverage in
example-specific tests or turn temporary review scripts into permanent tests.
Ordinary tests use minimal infrastructure fixtures, not live examples; generic
shipped-artifact smoke checks and frozen compatibility replay remain separate.

Do not display “Saved,” “Saving…,” or routine persistence indicators inside authored slops. The native host owns save-failure and retry UI. Use task-specific feedback for explicit operations, such as “Importing skin…” or “Skin applied.”

Read slop.ts first. It declares metadata with `defineSlop({ slug, title, description, author, categories, window, theme, artwork? })`. Required root `schema.ts` default-exports `defineDocument(fields, {initial})`; initial is optional when all fields may be omitted. Required `App.svelte` is the editor; required `Preview.svelte` supplies compact preview artwork, and required `Icon.svelte` supplies the icon. `Export.svelte`, `Widget.svelte` and agent-only `commands.ts` are optional. Role names are fixed and case-sensitive. Import CSS (including optional `hitslop/base.css`) from `App.svelte` or another component, never from slop.ts. schema.ts default-exports definition data and may also export helpers and shared edits. Vite builds disconnected UI and definition graphs; the restricted Rust runner evaluates the latter. The page cannot import slop.ts or commands.ts. Author schema.ts with defineDocument/s. Supported descriptors: s.text (merging typed text), s.boolean, s.string({minLength,maxLength}), s.number({min,max}), s.integer({min,max}), s.enum([...]) (last writer wins; string bounds count Unicode code points, while DOM/edit offsets count UTF-16 units; bounds are inclusive), s.optional(scalar, text or object) (absent until set; `clear()` removes it, except an optional object's handle, which also carries its fields and so uses `$set(value)` and `$clear()`; inserts and initial values may omit it; an absent optional object gets creation defaults; replacing an existing object with `$set` requires all required fields and reconciles surviving children; an unset optional text binds as "" and typing creates it), s.object, s.list(s.object) rows with $id, s.list(scalar) by index (`insert(v, i?)`, `set(i, v)`, `preview(i, v)`, `remove(i, n?)`, `replace(values)`; no move), s.record(scalar or object) by string key (`put(key, v)`, `delete(key)`, `entry(key)`; keys are 1–256 UTF-16 units, not `$id`/`__proto__`/`constructor`/`prototype`; putting a complete existing object reconciles text, lists and counters while preserving surviving identities; defaults apply only to absent entries; `doc.at(entry)` resolves an object entry) and integer s.counter. Trees and rich text are not implemented. Keep transient view state in $state.

Fields a write leaves out take their default: text starts "", lists and records empty, counters at 0, and a scalar takes the `default` it declares, `s.boolean({ default: false })` or `s.enum(["low", "high"], { default: "low" })`. Inserts, `put`, `$set` of an optional object and `initial` may omit those fields, so declare defaults instead of repeating `done: false` at every insert. A scalar without a default is still required. Defaults belong to object fields; optional fields, list elements and record values have none.

schema.ts default-exports plain `defineDocument(...)` data. Editor components import `schema` and call `const doc = getDocument(schema)` from `hitslop/svelte` while initializing, at the top of the script or inside a store factory called then. Never call it at module scope, in event handlers or timers. Each mount has its own document. `Value<typeof schema.descriptor>` types the saved data; `LiveDocument<typeof schema.descriptor>` types the acquired document. Read immutable `doc.current`; ordinary handles return promises: `await doc.at(row).done.set(true)` and `const {id} = await doc.fields.items.insert(...)`. A scalar `set` shows in `doc.current` at once and reverts if refused. `list.item(row)` addresses a row directly; `item`, `remove`, `move` and the `{ before }`/`{ after }` destinations of `insert` and `move` accept a row snapshot or its `$id`, so `doc.fields.tasks.move(task, { before: neighbor })` works. `await doc.change(tx => { ... })` collects synchronous tx writes once into one atomic batch; tx insert returns an ID immediately for later tx writes. Do not use async/nested collectors or ordinary document writes inside them. A write resolves after acceptance and local publication, before durability or necessarily a DOM update; use `await tick()` for the DOM and `await doc.flush()` for saving. Edit ▸ Undo (⌘Z) steps back through the person's changes and agent edits, one step per `doc.change`, write, typing run or run of agent edits; write a gesture once when it ends, and call `doc.undo()`/`doc.redo()` for authored buttons instead of keeping your own undo stack.

Edit text with `<EditableText class="…" field={row.text} label="…" placeholder="…" onenter={…} />` from `hitslop/svelte`, for titles and rows alike: it shows the field's text and mounts a bound textarea only while edited (WebKit form controls are too expensive to mount by the thousand), puts the caret where the person clicked, grows with pending typing and composition, returns focus to its display on Escape without discarding text, and calls `onenter` for Enter outside IME composition (Shift+Enter, or no `onenter`, adds a line). Give its typography to that one class; style `[data-placeholder]` inside it for the empty text. `use:bindText={handle}` binds your own `<input>` or `<textarea>` when you need one; both keep what the person types and send each change for the owner to merge, preserving Unicode, the caret and composition. A live text handle reads accepted text as `handle.value` (read-only); an active binding keeps pending typing and composition in its DOM control. Scalars (booleans, strings, numbers, integers, enums) have a `value` for Svelte `bind:` on native inputs and bits-ui components: `bind:checked={doc.fields.done.value}`. Assigning shows the value at once and commits once it settles (one write per drag or typing burst); null or undefined clears an optional field; a refused value reverts and is reported. Svelte binds only to a name or property path, so name a row's handle first: `{@const row = doc.at(task)}` then `bind:checked={row.done.value}`. For drags and drawing, call `handle.preview(value)` per frame and `set` the final value; previews stay local until set or flush. Text handles write whole fields with `set(value)`. Flush sends unsent text and waits for pending writes; close/export commit a composition in progress. Import attachments with `attachments.import<typeof schema.descriptor>(file, (tx, ref) => tx.fields.photo.$set({ id: ref.id, name: ref.name, mimeType: ref.mimeType }))`: the reference is written in the same step as the stored blob. `hitslop/svelte` also owns the behavior every slop repeats, while the markup stays yours. `const task = draft(text => doc.fields.tasks.insert({ text }))` is a composer: bind `task.value` to your input, disable the button unless `task.ready`, and use `onsubmit={task.submit}`; it trims, ignores a second submit while one runs, and clears only once accepted and only if the person typed nothing more. `const fill = motion(() => ratio)` eases a number (render `fill.current`), jumps under reduced motion and starts at its target. `notify("3 tasks filed.")` shows a message in the window's notice region, `[data-slop-notice]` (a `<p>` inside it); restyle that selector in your CSS. Failures need no code: do not add try/catch, error state or `.catch(() => {})`. An unhandled refusal from `refuse()` (see Named commands) appears in the notice region; any other failure is reported by the host. Catch only to react in your own way; caught failures are not reported again. Bindings report their internal failures once. The host owns save-failure and retry UI.

Writes are asynchronous, so avoid these patterns:
- Read-modify-write from a snapshot (`set(qty + 1)`): await dependent writes before reading, or use `increment` on a counter. A synchronous `change(tx)` collector provides atomic writes, not fresh transactional reads.
- Writes in `$effect` or on mount to "ensure" defaults: declare `default`s in the schema and put starting content in the `initial` option in schema.ts.
- Using an insert's id, or reading `doc.current`, right after an unawaited write: `await` it (inside `change`, insert ids are synchronous). To act on a write's outcome (a celebration, a sound, a notice), react to the state in `$effect` or `$derived` instead of reading it back in the handler.
- `try/catch` around an unawaited write: `await` it so the catch sees the rejection.
- Positions in a number field (`order: s.integer()`) that moves rewrite: two moves can produce the same number. Order rows by their place in the list and use `move`.
- Moving a row between lists while expecting identity to survive: use one list plus a status/group field. SDK inserts mint new ids; only within-list moves preserve the Loro row container.

Never replace an existing identity-bearing list through a containing object. Use insert/remove/move. Subtract from a counter with increment(-n); there is no concurrent reset API. History is bounded; undo is for the current session, not a durable audit log.

Rust owns Loro and storage in both the native app and browser development. WebViews receive snapshots and patches; `slop dev` connects Vite to a native Rust owner of a disposable file. WASM runs SDK tests and the local Chrome browser host (`slop open --browser`), using the same Rust owner and SQLite store. Browser copies persist in OPFS and download as complete `.slop` files; `slop dev` remains a native-owner preview. Build packs one SQLite template: scalar catalog metadata, one immutable definition, initial values, `ui.js`, optional `ui.css` and `commands.js`, content-addressed media and artwork. Svelte and the app SDK are bundled; the host supplies the page shell. Initial values become a seed Loro checkpoint. No source tree or dependency installation is needed to open it.

Start anywhere with `bunx hitslop init NAME`, then `cd NAME` and `bun install`. Use the generated `bun run check/dev/build/register` scripts. Bun is the only JavaScript runtime required. check/dev/build/register run on macOS or Linux; on both platforms, build and register also render the Preview and Icon artwork you do not supply, and PNG/PDF export renders a closed document, with no Mac app, Swift or Xcode. `slop open` needs the compatible installed hitSlop Mac app, which is also where the bundled starter templates come from. Data commands run through the native Rust engine on macOS or Linux. Preview state is disposable; rerun dev to rebuild source. Create a writable document before editing with `slop create --from TEMPLATE --output PATH`, where TEMPLATE is a registered template's slug (`slop templates` lists them) or a built template's path. Agents use describe/call for the app's named commands, and schema/get/apply/batch for direct field edits. Supply an explicit `id` to address an inserted row from later CLI commands. After an unknown outcome, inspect current state before another edit; never blindly replay the insertion.

For an agent already doing the work, use `init NAME --yes --brief 'What to build'`,
optionally with `--author`, `--title`, `--description`, and one or two `--category`
flags. The wizard initializes the explicit `slug` from NAME: 2–64 lowercase letters or digits, separated
by single hyphens. Metadata is validated before
any project files are created. `--yes`, CI, and non-TTY runs never prompt or launch
another agent. For humans, setup asks for a build brief and author, then offers
a detected agent CLI, Other CLI, or Finish without launching. Title, description,
and categories start as placeholders: set them in slop.ts to match what you
build; the user can edit them later. The chosen agent
runs in the project with normal permissions and reads `BRIEF.md` and `AGENTS.md`.
Launch failure keeps the project. Read the brief before adapting the starter.

`slop skills install` adds selected guides and agent links; `skills repair`
repairs existing links, and `skills uninstall` explicitly removes owned links.
Bare `skills` means install; use `skills repair` to repair installed links. The
guides linked into a new project's `.agents/skills` follow its pinned
`node_modules/hitslop` install and update when that dependency changes.

Projects are discovered under slops and bundled selection lives in bundled.json; Quick Checklist is the reference example. Use plain CSS, slop.ts theme colors (only colors a person may change; fonts and derived values in CSS) and each app's own visual identity. Read the bundled hitslop-design references for CSS, presentation, and capture. PNG/PDF export is supported; hosted publishing and catalog are deferred.

Use `App.svelte` for the editor, required `Preview.svelte` and `Icon.svelte`, plus optional `Export.svelte` for captures. All capture roles render saved state through synchronous Svelte SSR and Takumi in the restricted Rust child, including selectable vector PDF. Browser-dependent capture closures fail the build, as does your own code that uses `URL`, `URLSearchParams`, `TextEncoder`, `TextDecoder`, `AbortController`, `Blob` or `structuredClone`, which the capture runtime lacks. There is no public override or automatic fallback. Static views receive `snapshot` (deeply readonly saved data) and `attachmentURL(id)` (throws for a missing or non-image attachment); Preview, Export and Icon also receive `mode` (`"preview" | "export" | "icon"`), while Widget receives `family`, `width` and `height`. Type them with `CaptureProps<typeof schema>` plus `{ mode: CaptureMode }` or `WidgetProps`, imported as types from `hitslop/svelte`, and `import type schema from "./schema"`. The build refuses editor runtime imports (`getDocument`, `draft`, `motion`, `notify`, `EditableText`, `bindText`, `speech`, `resizeWindow`, etc.) and Svelte's `onMount`, following aliases, namespace imports and re-exports. `attachments` is refused too: read a stored image's URL from the `attachmentURL` prop. Everything `hitslop/embed` exports (the YouTube link parser, lookup and player) is refused as well; `hitslop/youtube` is static-safe and exports string helpers (`thumbnail`, `src`, `defaultRelay`, `idShape`), but capture cannot load a remote thumbnail: store it as an attachment in the editor and use `attachmentURL` in the static view. Use a thin dedicated export with natural height for growing content such as a checklist, and keep its markup, title and theme recognizable. Preview is a compact overview; Export includes all content. Without Export, PNG/PDF actions are hidden and CLI export is refused. Dedicated captures ignore the window silhouette. Quick Look reads saved preview artwork. Supply imported PNGs with `artwork: { preview, icon }`; build and register render whichever you leave out, from the initial values, on macOS and Linux. Register replaces a master only after a successful build. Optional `import "hitslop/base.css"` in App.svelte, before app styles, supplies low-specificity form, focus and reduced-motion defaults.

A capture's CSS is exactly what its own Preview/Export/Icon/Widget role imports, in import order, including component CSS. Captures no longer inherit the editor's stylesheets; import shared CSS in each role or a component it imports. `slop dev` still mounts static views inside the editor page. Build and register render every declared view during pack: Preview, Icon, Export PNG/PDF and all Widget families, even with supplied artwork. A failed view render publishes nothing; only unsupplied Preview/Icon artwork is stored.

Import images, fonts and skins through Vite. CSS `url()` and `@import` dependencies are included; unreferenced files and `public/` copies are not. Skin windows use `{ kind: "skin", width, height, image: importedPng }`; the RGBA PNG must be exactly 1× or 2× the logical window size and contain transparency. Standard windows use `kind: "standard"`.

Editor children also call `getDocument(schema)` during initialization; in long lists, pass rows and handles as props. Components shared with static views take data and an attachment URL function as props; editors pass `attachments.url`, static views pass `attachmentURL`.

Read [the workflow](references/workflow.md) and [package boundaries](references/storage-and-packages.md) for the complete source-to-document path.

Use `attachments` from `hitslop/svelte` for portable binary files.
`await attachments.import<typeof schema.descriptor>(file, (tx, ref) => { /* write reference fields with tx */ })`
stores the bytes, then writes the reference in the same step; it rejects if either fails. `read(id)` returns
a Blob fetched from `attachments.url(id)`; use that URL directly for images, audio and video. Rust determines the media type from bytes. Keep references in your own fields; they are ordinary schema scalars,
never base64 document values. Validate app formats first. Limits: 10 MiB/file,
100 MiB and 256 files/document. HTTPS data/media requests are allowed; CORS applies.

Keep high-frequency or transient values (drag positions, playback, timers) in local state, not saved fields; use attachments for binary data. Documents are capped at 32 MiB.
Counters are exact while increments summed as absolute values over their whole history stay below 2^53 (about 9×10^15). Beyond that envelope an off-by-one result is possible, including after replacement, undo or reopen; no exactness is guaranteed. The owner serializes increments and refuses an out-of-range result; render the saved number directly.

`slop dev` watches source with Vite. Component and CSS HMR retain accepted document state. A change to slop.ts, schema.ts, commands.ts or their dependencies (including skin and artwork) resets disposable state; refresh also resets it. Correcting a failed edit clears the diagnostic. Adding, deleting or renaming a role also rebuilds the owner; an invalid role name fences the preview.

The author SDK is `hitslop`; the private page shell runtime is host-owned.
Catch semantic refusals with `isRejected(error)`, a command's `refuse()` with
`isRefused(error)`, and other document outcomes with `isDocumentError(error)`, not
`instanceof`. Transaction handles collect writes only;
use live handles for previews and bound `.value` assignments. Explicit `flush()`
remains available.

## Devices and speech

For editor presentation, call `const environment = getEnvironment()` from `hitslop/svelte`
during component initialization. Read `environment.current.platform` (`macos`, `ios`,
`android`, `windows`, `linux`, `unknown`), `.device` (`phone`, `tablet`, `desktop`,
`unknown`) and `.layout` (`compact` below 640 CSS px of the document viewport, otherwise
`regular`). Native hosts report identity; browsers and older hosts use `unknown`.
A narrow Mac window remains desktop/macOS. Use layout for available space and ordinary
pointer/hover queries for input differences; identity does not grant capabilities.
Keep reads reactive rather than destructuring `current` once. Editor CSS can select
`html[data-slop-platform="macos"]`, `html[data-slop-device="phone"]` and
`html[data-slop-layout="compact"]` (`:global(html[...])` in Svelte component CSS).
For mobile support, reserve the native menu's header corner with
`--slop-host-actions-width` (64px) and `--slop-host-actions-height` (44px). These are zero
in other presentations. The host accounts for the notch, home indicator and keyboard;
never add those insets again. Header titles must wrap, and fixed controls and full-height
sheets must leave the menu reachable. Follow the Mobile editors section of hitslop-design
and review a native iPhone alongside the browser preset. `slop dev` has Browser/iPhone/iPad/Mac presentation presets that retain edits;
they do not simulate native services. Static views use their capture props, never this
editor helper or its CSS attributes.

Declare only the capabilities the app uses in `slop.ts`: `capabilities: ["camera", "microphone", "speech"]`.
Camera and microphone use `navigator.mediaDevices.getUserMedia`, `MediaRecorder` and canvas;
start from a button, show capture state, handle denied permission, and stop tracks on stop,
hide and teardown. Keep streams/chunks in view state and import finished named Files through
`attachments.import`. macOS consent belongs to hitSlop, not each document. Capture renderers
never request devices. Playback and ordinary file import need no capability.

`speech.transcribe(refOrId, { locale: "en-US" })` from `hitslop/svelte` transcribes a stored
recording on-device in the Mac app; omit locale for the person's default. Declare `speech`;
microphone is separate. Save returned `text` through document handles. Use `isSpeechError`
for `unavailable` (reason `notDeclared`, `host`, `system`, `locale`, `permission`, `format`), `cancelled`
or `failed`. Previews and browser copies return `unavailable`/`host`; native recognition never
falls back to uploading a clip. The Mac provider transcribes m4a, mp3, wav, flac, mp4 and mov;
other formats return `unavailable`/`format` before permission is requested. Let the host
report support instead of copying its format list. Locale separators normalize from `_` to `-`. Keep recording/playback useful when transcription is unavailable. A capability
change needs a rebuilt template and new document; existing documents keep their app.

## Named commands

Run `slop describe PATH --json` to inspect the schema, operations, command arguments,
current values, row IDs and version. Prefer a matching domain action with
`slop call PATH NAME --args JSON`. It runs the command stored in that document and saves
one atomic batch. A refusal applies no edits. After an unknown outcome, read the state
before deciding on another action.

Root `commands.ts` default-exports `defineCommands(schema, { name: { description, args, run } })` from `hitslop/commands`. Only this module may import that entry point, and the page cannot import it. Arguments are flat `s.string`, `s.number`, `s.integer`, `s.boolean` or `s.enum`, optionally wrapped in `s.optional`. Defaults are filled by Rust. A row ID is `s.string()`: use `tx.fields.tasks.item(id)` or `remove(id)`; missing rows refuse the entire edit. Objects, lists, records, text, counters and row descriptors are not arguments. JSON Schema is only a `describe` projection.

Call `refuse("Enter a task.")` (from `hitslop`) to stop a command with a message for the person: nothing changes, the window shows it in the notice region, `slop call` reports it with reason `refused`, and `isRefused(error)` recognizes it when caught. Anything else a command throws is a fault the host reports.

Commands are for agents; pages use handles and `doc.change()`. Write an edit both need once, as a plain function over `Tx<typeof schema>` (from `hitslop`): the page runs it with `doc.change(tx => edit(tx, …))`, the command's `run` calls `edit(tx, …)`. Both are one atomic edit and one undo step; pass time in as an argument (`Date.now()` in the page, `ctx.now` in a command).

```ts
// schema.ts
import { defineDocument, refuse, s, type Tx } from "hitslop";
const schema = defineDocument({ tasks: s.list(s.object({ text: s.text(), done: s.boolean({ default: false }) })) });
export default schema;
export function addTask(tx: Tx<typeof schema>, text: string) {
  if (!text.trim()) refuse("Enter a task.");
  return tx.fields.tasks.insert({ text: text.trim() }); // { id }, returned to the caller
}

// commands.ts
import { defineCommands } from "hitslop/commands";
import { s } from "hitslop";
import schema, { addTask } from "./schema";
export default defineCommands(schema, {
  addTask: {
    description: "Add a task and return its row ID.",
    args: { text: s.string() },
    run: ({ tx }, { text }) => addTask(tx, text),
  },
  completeTask: {
    description: "Mark a task done.",
    args: { task: s.string({ description: "The task's $id." }) },
    run: ({ tx }, { task }) => tx.fields.tasks.item(task).done.set(true),
  },
});

// App.svelte: inside <script lang="ts">
import { draft, getDocument } from "hitslop/svelte";
import schema, { addTask } from "./schema";
const doc = getDocument(schema);
const newTask = draft(text => doc.change(tx => addTask(tx, text)));
```

Describe every argument that names a row as its `$id` so agents know to pass one. The owner's restricted evaluator provides immutable `ctx.current`, `ctx.tx`, `ctx.now` and `ctx.random()`. `run` is synchronous and returns JSON; no promises, generators, DOM, network, ambient time or Svelte state. Use the supplied `ctx.current` and `ctx.tx`; the definition has no live document methods. Each evaluation starts fresh. Opening a document never upgrades its embedded app.


## Widgets

Optional root `Widget.svelte` renders a fixed canvas from its saved `snapshot` prop and the effective theme. Import `WidgetProps` from `hitslop/svelte`; it supplies `family` (`small`, `medium`, `large`), `width` and `height`. Use the `slop dev` Preview selector to switch between Editor, Preview, Icon and the three widget sizes without discarding edits; component/CSS HMR updates the selected composition. Overflow is clipped and the preview warns when the capture target overflows. Share authored typography, colors and components, and choose a useful amount of content for each size. There is no export/preview fallback. The Mac picker lists created documents only after all three images have been captured. Successful saves coalesce for one second before a native widget capture; opening with missing images and closing after edits also capture. Each complete generation requests a WidgetKit reload; the OS controls visible timing. Optional `widget: { refreshMinutes: 30 }` in `slop.ts` adds approximate scheduled refresh while the Mac app runs (15–1440 minutes). Save, file-watch and startup freshness checks also run without an interval. Clicking a placed widget opens its source document. iOS is deferred.

Widgets render through synchronous Svelte SSR and Takumi in a restricted Rust child.
Read `snapshot`; handles, writes, async/await, browser globals, `onMount`, DOM/canvas,
and browser capture hooks are unavailable in native widget output. Use flexbox, numeric
grid placement, explicit font properties, and SVG attributes/currentColor. Review native
output: the browser composition preview uses a different renderer. Bundle custom fonts
with `@font-face` and imported `url()`; aliases, variable weights and normal/italic faces
are registered per capture. Default sans/serif/mono and color emoji are provided; system
font discovery and remote font loading are not. Icons, preview artwork, export PNGs and
vector PDFs all use this renderer. The `attachmentURL(id)` prop resolves saved
PNG/JPEG/GIF/WebP images from a bounded inventory; blob reads/imports are unavailable. Quick Look displays saved artwork without running the renderer itself.
The running Mac app also watches cached documents for closed-owner edits and catches
up at startup/reopen. Renderer and bundled-font updates invalidate cached widget
images even without a document edit. A failed refresh retains the previous generation.
