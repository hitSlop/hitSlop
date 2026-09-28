# Author a mini app

A slop does one understandable job. Start with the user's primary action, the information they need at a glance, and what completion looks like. Each template owns its identity; shared infrastructure does not prescribe a collection-wide appearance.

## Start and iterate

In this checkout, use `bun slop dev examples/slops/quick-checklist`, `bun slop build SOURCE`, and `bun slop register SOURCE`. Repository contributors should follow [development](development.md) for runtime/helper preparation and adding templates.

For the distributed tools, install Bun and the matching Mac app, then:

```sh
bunx @hitslop/cli init my-slop
cd my-slop
bun install
bun run dev
```

Interactive setup collects a build brief and author, then asks which agent CLI
should implement the slop. The agent sets the manifest title, description, and
categories to match what it builds. Choose a detected agent, enter another
installed CLI, or finish without launching. `BRIEF.md` and `AGENTS.md` carry the
handoff. Use `--yes` and explicit metadata flags for unattended creation; CI and
non-TTY runs never launch an agent. See [CLI workflows](cli.md#create-preview-and-register-an-app)
for flag examples, defaults, and skill installation.

CLI 1.2.0 uses document/schema SDK 1.1.0 and works with Mac 1.0.7. Generated projects pin the CLI and its required SDK separately; these package versions need not be equal. Prefer their `bun run check/dev/build/register` scripts. Native capture requires the installed Mac app or an explicit `HITSLOP_NATIVE_CLI`; authoring never compiles Swift. Browser preview needs no native renderer.

Read `manifest.json` first. Set `runtime` to `hitslop-v1`, author, slug, title, description, one or two categories, and initial presentation. Define `schema.ts`, creation-only `initial.ts`, token defaults in `theme.ts`, and UI in `App.svelte`. No entry file is needed: the build mounts `App.svelte` with `styles.css` through `defineSlop`. A non-Svelte app supplies `main.ts` exporting `default { mount(ctx, target) }`; see the [runtime reference](../reference/runtime.md). Apps never import the runtime or call the host bridge.

Preview state is disposable: refresh resets it and source changes require restarting dev. Build emits `dist/<slug>.slop`. Registration builds a complete immutable master under `~/.hitslop/templates`, backing up a previous master outside the catalog before replacement. Create a writable copy in the Mac app to test persistence. Source, template, and writable document are distinct objects.

## Model state

```ts
import { defineDocument, s } from "@hitslop/document";
export default defineDocument({
  title: s.text(),
  tasks: s.list(s.object({ text: s.text(), done: s.boolean(), lane: s.enum(["todo", "doing", "done"]) })),
  checkins: s.list(s.string()),
  cells: s.record(s.object({ raw: s.string(), style: s.optional(s.object({ bold: s.boolean() })) })),
  cups: s.counter(),
  outline: s.tree(s.object({ label: s.text() })),
  bpm: s.integer({ min: 40, max: 240 }),
});
```

Choose each field by how concurrent edits should merge:

| Builder | Use for | Merge behavior |
| --- | --- | --- |
| `s.string({maxLength?})`, `s.number({min?,max?})`, `s.integer({min?,max?})`, `s.boolean()`, `s.enum([...])` | Settings, labels, amounts, states | Last writer wins |
| `s.text()` | Anything a person types | Character edits merge |
| `s.richtext({ bold: "after", link: "none" })` | Styled text; marks are declared with their expansion | Characters and marks merge |
| `s.object({...})` | Fixed groups of fields | Per field |
| `s.list(s.object({...}))` | Rows with identity: tasks, cards, entries | Insert, remove and move keep `$id` |
| `s.list(scalar)` | Plain sequences: dates, tags, a pixel grid | Positional insert, set, remove, move |
| `s.record(value)` | Values keyed by a string: spreadsheet cells, per-day entries | Per key; concurrent creation of one key merges |
| `s.counter()` | Tallies that several people may bump | Increments add up; reads `number \| null` (`null` only after merged increments overflow, flagged in issues). Show it as unavailable and disable increments, as Koi Pond does. |
| `s.tree(s.object({...}))` | Outlines and nested hierarchies | Moves never create cycles |
| `s.optional(node)` | A value that may be absent | Created lazily; concurrent creation merges |

Model moves between groups as a flat list with an enum or ID field (kanban lanes, Eisenhower quadrants), and real hierarchy with `s.tree`. Never store coordinates or fixed-size tuples as lists. Keep transient view state such as selection, hover, open panels and card flips in component `$state`; persist it only when a person expects it after reopening or through `slop get`. Supply explicit initial values without `$id`; tree initial values nest `children`. The descriptor is data, not JSON Schema, and schema changes require new documents.

Components call `useDocument(schema)`, which returns `{ current, status, error, fields, at, change, flush }`. Read only from the immutable `doc.current`; write only through handles:

```ts
const doc = useDocument(schema);
doc.at(task).done.set(!task.done);            // handle for any object from current
doc.fields.tasks.insert({ text: "", done: false, lane: "todo" }, { after: task.$id });
doc.fields.cells.put("A1", { raw: "1" });
doc.fields.cups.increment();
doc.change(tx => { for (const t of finished) tx.at(t).lane.set("done"); }, { message: "File finished" });
```

`doc.at(value)` accepts the root, a row, a nested object, a record or its object entry, or a tree node from `doc.current`, and returns that node's typed handle. Use the original snapshot object, not a clone. `fields.x.item(id)` and `entry(key)` remain for IDs held elsewhere. Key UI rows and selection by `$id`, never by index. Unchanged rows keep object identity across edits.

Initialize absent optional composites with `set`, and absent record entries with `put`. This includes row lists and trees. Once present, identity-bearing lists and trees must be edited with `insert`, `remove`, and `move`; assigning a containing object cannot replace them either. Clear an optional field or delete a record entry explicitly before recreating it with new identities. Scalar lists use an empty list instead of optionality.

`doc.change(tx => ...)` is one synchronous, all-or-nothing commit; its optional message is kept in document history. A rejected operation poisons the whole change even if caught. Inside the callback, `doc.current` still shows the state before the change; write through `tx.fields` or `tx.at`, never through `doc`. `await doc.flush()` is the durability barrier. Writes outside `change` commit individually.

Use `bindText` for plain text inputs. Typing becomes Unicode-safe text splices; composition drafts are rebased against intervening text changes and selection is adjusted when the input updates. This is a plain-text binding, not a collaborative rich-text editor. Use `bindValue` for range, number, select, checkbox and text inputs bound to scalars: range drags preview on `input` and commit once on `change`, starting normal autosave. For your own gestures, call `handle.preview(value)` (or `list.preview(index, value)`) while dragging and `set` when done; pending previews commit at flush, close and export and never enter history on their own. The host autosaves and flushes before close/export, presenting retry on failed saves. Never edit SQLite or add a JSON reconciliation writer.

## Attachments and HTTPS

In interactive native windows, standard `<input type="file">` controls open a
document-attached file picker. Cancellation leaves the document unchanged.
Background rendering and headless commands never present file pickers. Validate
the selected file in authored code before storing it.

Keep the document for state worth saving. High-frequency or transient values (drag positions, playback progress, timers, hover state) belong in local component state or `handle.preview()`, which commits only at flush; writing them on every frame grows history for no benefit. Store binary data as attachments, never inside fields. Documents are capped at 32 MiB; a refused edit leaves everything already saved intact.

Import `attachments` from `@hitslop/document/attachments`. Save a browser File with
`await attachments.import(file, { commit(ref) { doc.change(tx => { /* update typed fields */ }); } })`.
The synchronous commit runs after durable blob storage; the promise also waits for
the document write. Every failure rejects the promise. A commit that throws, or limit
refusals, reject with `OperationRejectedError` and are final (the stored blob stays
unreferenced); disk failures stay queued for native retry. References contain `id`, `name`, `mimeType`, and `byteLength`.
Model them with ordinary scalar/object schema fields. Never put binary/base64
payloads in Loro. Validate app-specific formats before importing.

`attachments.read(id, { type: ref.mimeType })` returns a typed Blob; revoke any object URLs you create.
`attachments.list()` returns stored IDs and sizes, including unused blobs.
The native owner manages `state/attachments/`, deduplicated by SHA-256, with limits
of 10 MiB per file, 100 MiB and 256 unique files per document. Removing a reference
does not delete bytes; garbage collection is deferred. Filenames and MIME types
are descriptive metadata in the document, never filesystem paths.

Close/export/reload/duplication flush accepted imports. Disk failures remain in
native retry. Preview uses disposable memory storage with identical limits.
Templates contain no imported attachments.

Slops may fetch HTTPS data and play HTTPS media without manifest declarations.
Images may use HTTPS, data or blob URLs. CORS remains enforced; remote scripts
and JavaScript eval stay blocked. Bundle executable code locally.

## Design and styling

Quick Checklist and Small Expenses are current examples, not a limit or a default visual style. Start new templates from their own purpose. Paper, Instrument, and Skin are optional directions. `_vibe/` is inspiration only; never ship its images. Record product context in PRODUCT.md and each object's actual palette, typography, layout, state language, and motion in its DESIGN.md.

Make the purpose visible in the first viewport. Keep controls familiar, state readable through words and structure, and essential text comfortable. Start with realistic content at manifest dimensions; remove competing elements before shrinking labels. Aim for at least 12px supporting text, 14px control labels, and 44px action targets. Provide keyboard access, visible focus, sufficient contrast, and reduced-motion behavior.

Use plain CSS, with app-prefixed classes and related states/descendants kept together. Style Bits UI primitives through their supported attributes; give portal content explicit classes because it lives outside the trigger's ancestor tree. Define public tokens with `defineTheme` from `@hitslop/document/theme` and use `var(--slop-TOKEN)`. The builder writes immutable defaults to `assets/theme.json` and compiled app styling to `assets/app.css`. The runtime applies defaults and overrides before mounting the app, so no separate theme stylesheet is generated. Owners use `slop theme get/set/reset`; the host writes overrides to `state/theme.json`. Never edit these built files directly. Layout changes require authoring source and a rebuild.

Motion should explain change and settle for capture. Persist target values immediately, stop transient work on unmount, and honor reduced motion. Export hooks must not mutate saved state to prepare a view.

## Window presentation

Standard windows use initial width/height, optional `resizable` (default true), and optional `shape` (`rounded`, `ellipse`, or `capsule`). Width is 240–4096px; height is 180–4096px. Use fluid outer layouts and test initial and narrower dimensions. In native windows, unskinned apps can request `globalThis.slop.window.resize({ width, height })`; the host returns the applied size. The manifest remains the initial size. This host global is not a removed runtime-package import and is unavailable in disposable browser preview.

The window is the stage in every mode. The host resets `html`/`body` margins and makes them, `<Slop>`'s root, and its mount ancestors fill the window; framework-neutral apps mark their root `data-hitslop-root`. Size your shell with `height: 100%`, grid, or flex, and scroll inside panes or the root. Prefer these defaults over repeated `html`/`body` sizing or `100vh`; override deliberately when the layout requires it. The sizing rules have zero specificity, so deliberate overrides still work. Native owns the silhouette: built-in shapes and skin PNGs mask the window, so keep controls inside the visible shape (`data-slop-shape` is set for padding). For transparency, set `background: "transparent"` with a built-in shape; transparent and skin windows override ordinary `html`/`body` backgrounds, so draw the visible surface in your app and avoid more-specific or `!important` page backgrounds that defeat that transparency. Capture disables the sizing rules, so export and icon views use normal flow. `slop dev` previews the same stage at the manifest size, with the shape or skin mask.

| Host value | Meaning |
| --- | --- |
| `data-slop-presentation` | `standard`, `transparent`, or `skin` |
| `data-slop-shape` | Built-in shape, absent for PNG skins |
| `data-slop-resizable` | Present when user resizing is enabled |
| `--slop-width`, `--slop-height` | Initial dimensions, not live viewport measurements |
| `data-slop-controls` | Optional `visible` / `hidden` signal matching the native hover toolbar |

These presentation values are supplied by the native host. Use actual viewport layout in preview rather than assuming native masks or host globals exist there. Move windows with the native toolbar handle; no guest drag API or drag attribute exists.

For strictly hover-only actions, follow `html[data-slop-controls="visible"]` in CSS.
The host initializes this attribute to `hidden` and updates it with the native toolbar,
including its pointer hit testing, toolbar interactions, and 0.8-second hide delay.
Focus, Tab, and a disabled control do not reveal or retain these actions. Hide the
controls with `visibility: hidden` and `pointer-events: none`, not opacity alone,
so they are also absent from keyboard navigation while hidden. The attribute is
transient presentation state, not document data or a JavaScript event API.

Browser previews and older hosts may omit the attribute. Use
`html:not([data-slop-controls]) .your-shell:hover .your-controls` as the CSS fallback;
do not let that fallback override an explicit native `hidden` value. Continue to
mark editing controls `data-slop-export="hide"`, or omit them from an export view.

Use a PNG skin only when built-in shapes cannot express the outline or hole. `presentation` then contains only width, height, and `skin: "assets/window-mask.png"`. The RGBA image must exactly match those dimensions; the window cannot resize. Alpha 0–25 is click-through and 26–255 receives input. Keep controls/focus rings away from feathered edges and verify clicks actually reach the desktop through holes. Browser transparency alone does not reproduce native hit testing.

## Export and icons

Keep editor, export, and icon markup together unless a separate component helps:

```svelte
<script lang="ts">
  import { Slop, useDocument, bindText } from "@hitslop/document/svelte";
  import schema from "./schema";
  const doc = useDocument(schema);
</script>
<Slop>
  <input aria-label="Title" use:bindText={doc.fields.title} />
  {#snippet exportView()}
    <article><h1>{doc.current.title}</h1></article>
  {/snippet}
  {#snippet icon()}<div class="icon">✓</div>{/snippet}
</Slop>
```

`<Slop>` reads the host document from context, reports rendering failures, flushes before capture, and mounts snippets lazily against the same document. It requires an app mounted by the host through `defineSlop`; no document prop is needed. Nested wrappers for multiple documents are unsupported. Preview/PNG/PDF use `exportView` when supplied, otherwise the editor. Without an icon snippet, the host uses its generic icon.

Rendering failures in export or icon snippets reject that capture without replacing the editor or reporting an application-render error. Restoration clears the snippet failure so a later attempt renders it again. Editor rendering failures still use native application-error recovery and prevent capture.

Child components can call `useDocument(schema)` during initialization for typed access to the same host document, including inside export and icon snippets. Each call creates its own subscription, not another document engine. For repeated list components, pass reactive rows and typed handles (or the existing `doc` facade) as props to avoid a subscription per row.

Dedicated exports should use normal flow, not fixed viewport heights or nested scrolling. With `exportView`, the editor is never captured. Without one, mark editing-only controls `data-slop-export="hide"`; fallback capture renders native text inputs as wrapping text. `<Slop>` centers icon art in a transparent 512px square with a strong silhouette and safe margins. Dedicated exports do not inherit native masks.

Framework-neutral apps may use `capture.registerTarget("export" | "icon", { element, prepare, restore })`; targets must be direct body children. `capture.onPrepare` supports asynchronous readiness. Dispose registrations and do not mutate durable state. Capture waits for fonts, visible images, and stable layout, blocks edits, and restores editor focus/selection/scroll on success or failure.

PNG is 2×, limited to 16,384 pixels per side and 24 megapixels. PDF preserves searchable text/vectors on one full-length page with a maximum 14,400-point dimension. Build/register render disposable copies and generate Quick Look artwork without leaving mutable state in masters. Close refreshes the preview and Finder icon metadata; immutable `QuickLook/Icon.png` remains unchanged.

Live exports capture the selected view at current width; closed exports use saved state and the app's initial view. See [CLI export](cli.md#export) and [runtime capture details](../reference/runtime.md#capture-and-finder-integration).

## Review a complete object

Review empty, typical, long-content, failed, and busy states; keyboard/IME interaction; reduced motion; initial/narrow widths; theme overrides; and open overlays. Use normal controls for temporary review data rather than changing creation defaults.

Run check/build, register, create a writable copy, type then immediately close, reopen, duplicate, and export PNG/PDF. Inspect editor, export, and icon from the same revision. Native tests are required for persistence, clipping, skins, and desktop click-through. See [presentation fixtures](development.md#focused-checks) and the packaged [design skill](../../packages/cli/skills/hitslop-design/SKILL.md).
