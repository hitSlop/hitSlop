# Authoring templates

The public guides are the how-to for every author, including us. This page adds the
rules and standards for templates in this repository.

| Topic | Guide |
| --- | --- |
| First slop, end to end | [Create your first slop](../../apps/landing/src/content/docs/docs/getting-started.mdx) |
| Fields, handles, changes and previews | [Data and schemas](../../apps/landing/src/content/docs/docs/guides/data-and-schemas.mdx) |
| Saved state, lifecycle, attachments, HTTPS and YouTube | [Files and the web](../../apps/landing/src/content/docs/docs/guides/files-and-web.mdx) |
| App definition, window shapes and hover controls | [App definition and windows](../../apps/landing/src/content/docs/docs/guides/manifest-and-windows.mdx), [PNG window skins](../../apps/landing/src/content/docs/docs/guides/png-window-skins.mdx) |
| Plain CSS and the theme palette | [Style a slop](../../apps/landing/src/content/docs/docs/guides/styling.mdx) |
| Export views, icons and capture | [Icons, previews, and exports](../../apps/landing/src/content/docs/docs/guides/icons-and-exports.mdx) |
| Build, register and share | [Build and share](../../apps/landing/src/content/docs/docs/guides/build-and-share.mdx) |
| Every document kind, error code and CLI path | [Document types](../reference/document-types.md) |
| The app interface (`ctx`) for other frameworks | [Runtime reference](../reference/runtime.md#page-shell-and-ctx) |

## Work in this checkout

Use `bun slop dev examples/slops/SLUG`, `bun slop build SOURCE` and
`bun slop register SOURCE`. [Development](development.md) covers preparing the helper,
adding a template and choosing which templates ship.

## Rules for authored code

- Read `slop.ts` first. One slop does one understandable job.
- Apps never import the runtime, call the host bridge, write SQLite or keep a second JSON
  copy of the document.
- Save only what is worth keeping. Selection, hover, drag positions and timers stay in
  Svelte `$state`; files are attachments, never base64 in fields.
- Key rows by `$id`. Don't write in `$effect` or on mount; defaults belong in `slop.ts`'s
  `initial`, which seeds new documents only. Changing the schema makes a new document type.
- Don't silence writes with `.catch(() => {})`: the host reports a refused write. Catch
  only to show the template's own message. Order rows with `move`, not a position field.
- Capture views and export hooks never change saved state to prepare a view.

## Design standards

The examples are references, not a limit or a default visual
style. Start each template from its own purpose. Paper, Instrument and Skin are optional
directions. `_vibe/` is inspiration only; never ship its images. Record product context in
[PRODUCT.md](../../examples/slops/PRODUCT.md) and each object's actual palette, typography,
layout, state language and motion in its `DESIGN.md`.

Make the purpose visible in the first viewport. Keep controls familiar, state readable
through words and structure, and essential text comfortable. Start with realistic content
at the declared window size; remove competing elements before shrinking labels. Aim for at least
12px supporting text, 14px control labels and 44px action targets. Provide keyboard access,
visible focus, sufficient contrast and reduced-motion behavior.

The host disables ordinary text selection in the editor by default; inputs, textareas
(including readonly fields), and editable content retain normal selection. Enable
`-webkit-user-select: text; user-select: text` on useful copyable content such as notes,
addresses, or code, or on `body` for the whole slop. The default also applies in
`slop dev` and leaves captures alone. See the
[styling guide](../../apps/landing/src/content/docs/docs/guides/styling.mdx#text-selection)
for CSS overrides.

Motion should explain change and settle before capture. Persist target values
immediately and honor reduced motion. Stop transient work on unmount and pause it while
the window is hidden, as
[Files and the web](../../apps/landing/src/content/docs/docs/guides/files-and-web.mdx#opening-hiding-and-closing)
describes.

## Review a complete object

Review empty, typical, long-content, failed and busy states; keyboard and IME interaction;
reduced motion; initial and narrow widths; theme overrides; and open overlays. Use normal
controls for temporary review data rather than changing creation defaults.

Run check and build, register, create a writable copy, type then immediately close,
reopen, duplicate, and export PNG and PDF. Inspect the editor, export and icon from the
same revision. Native tests are required for persistence, clipping, skins and desktop
click-through. See [presentation fixtures](development.md#focused-checks) and the
packaged [design skill](../../packages/hitslop/skills/hitslop-design/SKILL.md).

## Commands

Keep the stored shape in `schema.ts`, declare actions with `doc.command(...)` in any module,
and register them by name in `defineSlop({ commands })`. The same action can be called by a
page button or `slop call`; either way it runs in the owner's restricted evaluator. Start with the few verbs
that matter; ordinary handles, bindings and `doc.change()` remain available.

```ts
import { s } from "hitslop";
import doc from "./schema";

export const rename = doc.command({
  description: "Change the title shown in the window.",
  args: { title: s.string({ minLength: 1 }) },
  run({ tx }, { title }) {
    tx.fields.title.set(title);
  },
});
```

A button imports `rename` and awaits `rename({ title: "Weekend" })`. The CLI discovers it
with `slop describe My.slop` and calls `slop call My.slop rename --args '{"title":"Weekend"}'`.
Descriptions on fields use options such as `s.text({ description: "Window title." })`.

Commands synchronously read immutable `ctx.current`, collect writes with `ctx.tx`, and
return JSON or throw. Use `ctx.now` (epoch milliseconds) and `ctx.random()`; avoid ambient
time, random sources, browser APIs, I/O and promises. Register commands by key in
`defineSlop({ commands: { rename } })`. Page buttons and CLI calls both execute in the
restricted child; module globals do not survive a call and `run` cannot capture Svelte
state. Argument objects are strict automatically. Arguments support `s.string`, number,
integer, boolean, enum, optional, nested objects and lists of scalars. Text, counters,
records, object lists and unions are not argument types. `description` is allowed on any
node. String `minLength` and `maxLength` count Unicode code points: an emoji is one;
HTML `maxlength` and DOM caret positions count UTF-16 units instead.
A successful command is one undo step. CLI completion means the batch was saved; page
completion means its snapshot published. A definite stale conflict retries once with the
same clock and seed; an unknown outcome never retries.

Command metadata lives in `definition_json`; `commands.js` is private and immutable.
JSON Schema is computed only for `describe` tool clients, never stored or accepted as input.
Installing a newer hitSlop does not replace those assets or migrate a document's app.

## Entry and media

`slop.ts` exports `defineSlop({ slug, title, description, author, categories, window,
theme, document, initial, view, ... })`. Import components and CSS explicitly. Filenames
such as `App.svelte`, `Export.svelte` and `schema.ts` are conventions, not discovery rules.
The window is either `{ kind: "standard", width, height, ... }` or
`{ kind: "skin", width, height, image: importedPNG }`.

Import template images and fonts in components, CSS `url()`s or the declaration. Vite
collects the real bundle and rewrites them to `/assets/media/<sha256>.<ext>` URLs; duplicates
share one resource. A skin or export-only image is included even if the editor never
renders it. `public/` is not copied; import those files explicitly. Supplied artwork uses
`artwork: { preview: importedPNG, icon: importedPNG }`. A skin must be RGBA at 1× or 2×
the logical window size; Rust validates its pixels before packing.

User-imported images are attachments, separate from template assets. Import through
`app.attachments.import(file, (tx, ref) => { ... })`, store the reference, and render
`app.attachments.url(ref.id)` in an image or media element. `read(id)` fetches a Blob when
code needs bytes. The host supplies the media type and serves ranges without a base64
read bridge. Attachments stay with the document when it is copied or shared.
