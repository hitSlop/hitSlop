# Authoring templates

The public guides are the how-to for every author, including us. This page adds the
rules and standards for templates in this repository.

| Topic | Guide |
| --- | --- |
| First slop, end to end | [Create your first slop](../../apps/landing/src/content/docs/docs/getting-started.mdx) |
| Fields, handles, changes and previews | [Data and schemas](../../apps/landing/src/content/docs/docs/guides/data-and-schemas.mdx) |
| Attachments, HTTPS and YouTube | [Files and the web](../../apps/landing/src/content/docs/docs/guides/files-and-web.mdx) |
| Manifest, window shapes and hover controls | [Manifest and windows](../../apps/landing/src/content/docs/docs/guides/manifest-and-windows.mdx), [PNG window skins](../../apps/landing/src/content/docs/docs/guides/png-window-skins.mdx) |
| Plain CSS and theme tokens | [Style a slop](../../apps/landing/src/content/docs/docs/guides/styling.mdx) |
| Export views, icons and capture | [Icons, previews, and exports](../../apps/landing/src/content/docs/docs/guides/icons-and-exports.mdx) |
| Build, register and share | [Build and share](../../apps/landing/src/content/docs/docs/guides/build-and-share.mdx) |
| Every document kind, error code and CLI path | [Document types](../reference/document-types.md) |
| The app interface (`ctx`) for other frameworks | [Runtime reference](../reference/runtime.md#page-shell-and-ctx) |

## Work in this checkout

Use `bun slop dev examples/slops/SLUG`, `bun slop build SOURCE` and
`bun slop register SOURCE`. [Development](development.md) covers preparing the helper,
adding a template and choosing which templates ship.

## Rules for authored code

- Read `manifest.json` first. One slop does one understandable job.
- Apps never import the runtime, call the host bridge, write SQLite or keep a second JSON
  copy of the document.
- Save only what is worth keeping. Selection, hover, drag positions and timers stay in
  Svelte `$state`; files are attachments, never base64 in fields.
- Key rows by `$id`. Don't write in `$effect` or on mount; defaults belong in `initial.ts`,
  which seeds new documents only. Changing the schema makes a new document type.
- Capture views and export hooks never change saved state to prepare a view.

## Design standards

Quick Checklist and Small Expenses are current examples, not a limit or a default visual
style. Start each template from its own purpose. Paper, Instrument and Skin are optional
directions. `_vibe/` is inspiration only; never ship its images. Record product context in
[PRODUCT.md](../../examples/slops/PRODUCT.md) and each object's actual palette, typography,
layout, state language and motion in its `DESIGN.md`.

Make the purpose visible in the first viewport. Keep controls familiar, state readable
through words and structure, and essential text comfortable. Start with realistic content
at the manifest size; remove competing elements before shrinking labels. Aim for at least
12px supporting text, 14px control labels and 44px action targets. Provide keyboard access,
visible focus, sufficient contrast and reduced-motion behavior.

Motion should explain change and settle before capture. Persist target values
immediately, stop transient work on unmount, and honor reduced motion.

## Review a complete object

Review empty, typical, long-content, failed and busy states; keyboard and IME interaction;
reduced motion; initial and narrow widths; theme overrides; and open overlays. Use normal
controls for temporary review data rather than changing creation defaults.

Run check and build, register, create a writable copy, type then immediately close,
reopen, duplicate, and export PNG and PDF. Inspect the editor, export and icon from the
same revision. Native tests are required for persistence, clipping, skins and desktop
click-through. See [presentation fixtures](development.md#focused-checks) and the
packaged [design skill](../../packages/cli/skills/hitslop-design/SKILL.md).
