---
name: hitslop-authoring
description: Create, preview, validate, build, and register hitSlop authoring projects with the TypeScript CLI. Use for slop.ts metadata, storage choices, package boundaries, capture, identity, and release workflow.
---

# Author local mini apps

Do not display “Saved,” “Saving…,” or routine persistence indicators inside authored slops. The native host owns save-failure and retry UI. Use task-specific feedback for explicit operations, such as “Importing skin…” or “Skin applied.”

Read slop.ts first. It explicitly declares `defineSlop({ slug, title, description, author, categories, window, theme, document, initial, view, commands })` from `hitslop`. Import components for `view`, optional `export` and `icon`; import CSS explicitly. `document` is the `defineDocument` export. Vite builds the UI and a separate declaration/command program; the restricted Rust runner evaluates the declaration. The page must not import slop.ts: put shared values in their own module. Author schema.ts with defineDocument/s. Supported descriptors: s.text (merging typed text), s.boolean, s.string({minLength,maxLength}), s.number({min,max}), s.integer({min,max}), s.enum([...]) (last writer wins; string bounds count Unicode code points, while DOM/edit offsets count UTF-16 units; bounds are inclusive), s.optional(scalar, text or object) (absent until set; `clear()` removes it; inserts and initial values may omit it; an optional object is created or replaced by `set` unless it holds lists, counters or text; an unset optional text binds as "" and typing creates it), s.object, s.list(s.object) rows with $id, s.list(scalar) by index (`insert(v, i?)`, `set(i, v)`, `preview(i, v)`, `remove(i, n?)`, `replace(values)`; no move), s.record(scalar or object) by string key (`put(key, v)`, `delete(key)`, `entry(key)`; keys are 1–256 UTF-16 units, not `$id`/`__proto__`/`constructor`/`prototype`; an object entry holding text or lists is edited, never replaced; `doc.at(entry)` resolves an object entry) and integer s.counter. Trees and rich text are not implemented. Keep transient view state in $state.

schema.ts default-exports `defineDocument(...)`, and that export is also the live document: components `import doc from "./schema"`. Read immutable `doc.current`; ordinary handles return promises: `await doc.at(row).done.set(true)` and `const {id} = await doc.fields.items.insert(...)`. A scalar `set` shows in `doc.current` at once and reverts if refused. `list.item(id)` addresses a row directly. `await doc.change(tx => { ... })` collects synchronous tx writes once into one atomic batch; tx insert returns an ID immediately for later tx writes. Do not use async/nested collectors or ordinary document writes inside them. A write resolves after acceptance and local publication, before durability or necessarily a DOM update; use `await tick()` for the DOM and `await doc.flush()` for saving. Edit ▸ Undo (⌘Z) steps back through the person's changes and agent edits, one step per `doc.change`, write, typing run or run of agent edits; write a gesture once when it ends, and call `doc.undo()`/`doc.redo()` for authored buttons instead of keeping your own undo stack.

`use:bindText={handle}` keeps the user's text in the field and sends each change for the owner to merge, preserving Unicode, the caret and composition. For text in a list's rows, or wherever a field may repeat, use `<EditableText class="…" handle={row.text} text={task.text} label="…" placeholder="…" onenter={…} />` from `hitslop/svelte`: WebKit form controls are too expensive to mount by the thousand, so it shows text and mounts a bound textarea only while edited, puts the caret where the person clicked, grows with its text, and calls `onenter` for Enter outside IME composition (Shift+Enter, or no `onenter`, adds a line). Give its typography to that one class; style `[data-placeholder]` inside it for the empty text. Scalars (booleans, strings, numbers, integers, enums) have a `value` for Svelte `bind:` on native inputs and bits-ui components: `bind:checked={doc.fields.done.value}`. Assigning shows the value at once and commits once it settles (one write per drag or typing burst); null or undefined clears an optional field; a refused value reverts and is reported. Svelte binds only to a name or property path, so name a row's handle first: `{@const row = doc.at(task)}` then `bind:checked={row.done.value}`. For drags and drawing, call `handle.preview(value)` per frame and `set` the final value; previews stay local until set or flush. Text handles write whole fields with `set(value)`. Flush sends unsent text and waits for pending writes; close/export commit a composition in progress. Import attachments with `attachments.import<typeof doc.descriptor>(file, (tx, ref) => tx.fields.photo.set({ id: ref.id, name: ref.name, mimeType: ref.mimeType }))`: the reference is written in the same step as the stored blob. Keep composer input until insert succeeds, preserve newer input, and display success notices only after acceptance. Do not add `.catch(() => {})` to writes: an unhandled refusal is reported to the person by the host. Catch only to show your own message; caught failures are not reported again. Bindings report their internal failures once. The host owns save-failure and retry UI.

Writes are asynchronous, so avoid these patterns:
- Read-modify-write from a snapshot (`set(qty + 1)`): await dependent writes before reading, or use `increment` on a counter. A synchronous `change(tx)` collector provides atomic writes, not fresh transactional reads.
- Writes in `$effect` or on mount to "ensure" defaults: put defaults in slop.ts's `initial`.
- Using an insert's id, or reading `doc.current`, right after an unawaited write: `await` it (inside `change`, insert ids are synchronous).
- `try/catch` around an unawaited write: `await` it so the catch sees the rejection.
- Positions in a number field (`order: s.integer()`) that moves rewrite: two moves can produce the same number. Order rows by their place in the list and use `move`.
- Moving a row between lists while expecting identity to survive: use one list plus a status/group field. SDK inserts mint new ids; only within-list moves preserve the Loro row container.

Never replace an existing identity-bearing list through a containing object. Use insert/remove/move. Subtract from a counter with increment(-n); there is no concurrent reset API. Checkpoints retain history.

Rust owns Loro and storage in both the native app and browser development. WebViews receive snapshots and patches; `slop dev` connects Vite to a native Rust owner of a disposable file. WASM is for SDK tests only. Build packs one SQLite template: scalar catalog metadata, one immutable definition, initial values, `ui.js`, optional `ui.css` and `commands.js`, content-addressed media and artwork. Svelte and the app SDK are bundled; the host supplies the page shell. Initial values become a seed Loro checkpoint. No source tree or dependency installation is needed to open it.

Start anywhere with `bunx hitslop init NAME`, then `cd NAME` and `bun install`. Use the generated `bun run check/dev/build/register` scripts. Bun is the only JavaScript runtime required. check/dev/build run on macOS or Linux; register, native artwork and PNG/PDF exports need the compatible installed hitSlop Mac app, not Swift or Xcode. Data commands run through the native Rust engine on macOS or Linux. Preview state is disposable; rerun dev to rebuild source. Create a writable document before editing with `slop create --from TEMPLATE --output PATH`, where TEMPLATE is a registered template's slug (`slop templates` lists them) or a built template's path. Agents use schema/get/apply/batch. Supply an explicit `id` to address an inserted row from later CLI commands. After an unknown outcome, inspect current state before another edit; never blindly replay the insertion.

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

Projects are discovered under examples/slops and bundled selection lives in bundled.json; Quick Checklist is the reference example. Use plain CSS, slop.ts theme colors (only colors a person may change; fonts and derived values in CSS) and each app's own visual identity. Read the bundled hitslop-design references for CSS, presentation, and capture. PNG/PDF export is supported; hosted publishing and catalog are deferred.

Import the editor as `view`, and optional capture components as `export` and `icon` in `defineSlop`. Filenames carry no meaning. Captures render saved state in a fresh hidden page; without an export component, the fresh editor uses default transient view state. An export or icon component that imports a module with top-level `$state` (state the editor sets) fails check and build: read `doc.current` there. The export component receives `mode: "preview" | "export"`. Supply imported PNGs with `artwork: { preview, icon }` or capture them with register / `build --artwork native`. Register replaces a master only after a successful build.

Import images, fonts and skins through Vite. CSS `url()` and `@import` dependencies are included; unreferenced files and `public/` copies are not. Skin windows use `{ kind: "skin", width, height, image: importedPng }`; the RGBA PNG must be exactly 1× or 2× the logical window size and contain transparency. Standard windows use `kind: "standard"`.

Child components `import doc from "./schema"` for the same document; in long lists, pass rows and handles as props instead.

Read [the workflow](references/workflow.md) and [package boundaries](references/storage-and-packages.md) for the complete source-to-document path.

Use `attachments` from `hitslop/svelte` for portable binary files.
`await attachments.import<typeof doc.descriptor>(file, (tx, ref) => { /* write reference fields with tx */ })`
stores the bytes, then writes the reference in the same step; it rejects if either fails. `read(id)` returns
a Blob fetched from `attachments.url(id)`; use that URL directly for images, audio and video. Rust determines the media type from bytes. Keep references in your own fields; they are ordinary schema scalars,
never base64 document values. Validate app formats first. Limits: 10 MiB/file,
100 MiB and 256 files/document. HTTPS data/media requests are allowed; CORS applies.

Keep high-frequency or transient values (drag positions, playback, timers) in local state, not saved fields; use attachments for binary data. Documents are capped at 32 MiB.
Counter values read `number | null`: `null` flags invalid stored contributions or merged overflow; render it as unavailable and disable increments.

`slop dev` watches source with Vite. Component and CSS HMR retain accepted document state. A change to slop.ts or any non-component module it imports (the document definition, commands, skin or artwork) resets disposable state; refresh also resets it. Correcting a failed edit clears the diagnostic. The entry's dependency graph, not filenames, decides what resets.

The author SDK is `hitslop`; the private page shell runtime is host-owned.
Catch semantic refusals with `isRejected(error)` and other document outcomes with
`isDocumentError(error)`, not `instanceof`. Transaction handles collect writes only;
use live handles for previews and bound `.value` assignments. Explicit `flush()`
remains available.

## Named commands

Run `slop describe PATH --json` to inspect the schema, operations, command arguments,
current values, row IDs and version. Prefer a matching domain action with
`slop call PATH NAME --args JSON`. It runs the command stored in that document and saves
one atomic batch. A refusal applies no edits. After an unknown outcome, read the state
before deciding on another action.

Authors export `doc.command({ description, args, run })` values and register them by name in `defineSlop({ commands: { addTask } })`. Arguments use `s.*`: `args: { text: s.string({ minLength: 1 }) }`, or `{}` for none. Scalars, optional values, nested objects and scalar lists are supported; document text, counters, records, object lists and unions are not argument kinds. `description` may annotate any argument node. Rust checks arguments before execution; JSON Schema is only a `describe` projection for tools.

Both page buttons and CLI calls go through the owner and restricted child runner. `run` synchronously uses `current`, `tx`, host `now` and `random()` and returns JSON or throws. It cannot use DOM, network, ambient time/randomness or Svelte state. Each evaluation starts fresh. Page callables are awaitable stubs; no command body ships in the UI, so write each declaration out as `doc.command({ description, args, run })`: another shape holding `args` and `run` (a spread, an extra field) fails the build. Keep bindings and `doc.change()` for incidental UI edits. Opening a document never upgrades its embedded app.
