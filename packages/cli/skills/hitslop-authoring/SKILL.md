---
name: hitslop-authoring
description: Create, preview, validate, build, and register hitSlop authoring projects with the TypeScript CLI. Use for manifests, storage choices, package boundaries, capture, identity, and release workflow.
---

# Author local v1 mini apps

Do not display “Saved,” “Saving…,” or routine persistence indicators inside authored slops. The native host owns save-failure and retry UI. Use task-specific feedback for explicit operations, such as “Importing skin…” or “Skin applied.”

Read manifest.json first. Author schema.ts with defineDocument/s, initial.ts and theme.ts. Supported descriptors: s.text, s.boolean, s.object, s.list(s.object) and integer s.counter. Other descriptors reject explicitly until their core and SDK coverage lands. Keep transient view state in $state.

Components call `const doc = useDocument(schema)`. Read immutable `doc.current`; ordinary handles return promises: `await doc.at(row).done.set(true)` and `const {id} = await doc.fields.items.insert(...)`. `list.item(id)` addresses a row directly. `await doc.change(tx => { ... })` collects synchronous tx writes once into one atomic batch; tx insert returns an ID immediately for later tx writes. Do not use async/nested collectors or ordinary document writes inside them. A write resolves after acceptance and local publication, before durability or necessarily a DOM update; use `await tick()` for the DOM and `await doc.flush()` for saving.

`bindText` keeps the user's text in the field and sends each change for the owner to merge, preserving Unicode, the caret and composition; `bindValue` submits booleans asynchronously. Text handles write whole fields with `set(value)`. Flush sends unsent text and waits for pending writes; close/export commit a composition in progress. Import attachments with `attachments.import(file, (tx, ref) => tx.fields.photo.set(ref.id))`: the reference is written in the same step as the stored blob. Keep composer input until insert succeeds, preserve newer input, and display success notices only after acceptance. Handle rejected promises; the runtime reports command failures centrally. The host owns save-failure and retry UI.

Writes are asynchronous, so avoid these patterns:
- Read-modify-write from a snapshot (`set(qty + 1)`): use a `change(tx)` collector or `increment` on a counter.
- Writes in `$effect` or on mount to "ensure" defaults: put defaults in initial.ts.
- Using an insert's id, or reading `doc.current`, right after an unawaited write: `await` it (inside `change`, insert ids are synchronous).
- `try/catch` around an unawaited write: `await` it so the catch sees the rejection.
- Moving a row between lists as separate remove and insert calls: do both in one `change(tx)` with an explicit id.

Never replace an existing identity-bearing list through a containing object. Use insert/remove/move. Counter decrement is increment(-n); there is no concurrent reset API. Checkpoints retain history.

Swift owns the native Rust Loro core; WebViews receive snapshots and patches. Browser development uses the same Rust core compiled to WASM. Do not embed the engine into app bundles or expose a second JSON writer. Build emits state.schema.json (a descriptor), initial.json, assets (including the app module assets/app.js, generated from App.svelte and styles.css) and document guidance. The host owns the page; apps reach it only through the document SDK. Never include state/, stores/, source, dependencies or caches in templates.

Start anywhere with `bunx @hitslop/cli init NAME`, then `cd NAME` and `bun install`. Use the generated `bun run check/dev/build/register` scripts. Bun is the only JavaScript runtime required; build/register need the compatible installed hitSlop Mac app, not Swift or Xcode. Preview state is disposable; rerun dev to rebuild source. Create a writable copy of a built/registered template before editing. Agents use schema/get/apply/batch/compact. Supply an explicit `id` for inserts you may need to retry: an insert whose outcome is unknown is refused as a duplicate on rerun only when you supplied its id.

For an agent already doing the work, use `init NAME --yes --brief 'What to build'`,
optionally with `--author`, `--title`, `--description`, and one or two `--category`
flags. `--slug` overrides the directory-derived slug. Metadata is validated before
any project files are created. `--yes`, CI, and non-TTY runs never prompt or launch
another agent. For humans, setup asks for a build brief and author, then offers
a detected agent CLI, Other CLI, or Finish without launching. Title, description,
and categories start as placeholders: set them in manifest.json to match what you
build; the user can edit them later. The chosen agent
runs in the project with normal permissions and reads `BRIEF.md` and `AGENTS.md`.
Launch failure keeps the project. Read the brief before adapting the starter.

`slop skills install` adds selected guides and agent links; `skills repair`
repairs existing links, and `skills uninstall` explicitly removes owned links.
Bare `skills` means install; `skills update` remains an alias for repair. The
guides copied into a new project's `.agents/skills` are portable files, not
managed links, and must be reviewed manually when upgrading the project.

Quick Checklist is the active trial example; other slops are archived; additional projects are discovered under examples/slops and bundled selection lives in bundled.json. Use plain CSS and defineTheme tokens and each app's own visual identity. Read the bundled hitslop-design references for CSS, presentation, and capture. PNG/PDF export is supported; hosted publishing and catalog are deferred.

Use `<Slop>` from `@hitslop/document/svelte`; optional inline exportView and icon snippets mount only during capture. Keep markup together in App.svelte unless a separate component helps. Build/register generate Quick Look artwork through the native helper, without bundling Loro. Register backs up and replaces an existing stateless master only after a successful complete build.

`<Slop>` takes no document prop. Child components call `useDocument(schema)` for the same document; in long lists, pass rows and handles as props instead.

Read [the workflow](references/workflow.md) and [package boundaries](references/storage-and-packages.md) for the complete source-to-document path.

Use `attachments` from `@hitslop/document/attachments` for portable binary files.
`await attachments.import(file, { commit: ref => doc.change(tx => { /* typed reference fields */ }) })`
waits for durable bytes, accepted reference edits and persistence. `read(id)` returns
a Blob; `list()` returns IDs and sizes. References are ordinary schema scalars,
never base64 document values. Validate app formats first. Limits: 10 MiB/file,
100 MiB and 256 files/document. HTTPS data/media requests are allowed; CORS applies.

Keep high-frequency or transient values (drag positions, playback, timers) in local state, not saved fields; use attachments for binary data. Documents are capped at 32 MiB.
Counter values read `number | null`: `null` flags invalid stored contributions or merged overflow; render it as unavailable and disable increments.
