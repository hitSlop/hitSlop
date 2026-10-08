# Source, templates, and documents

Source has one explicit `slop.ts` declaration: identity, window, theme, document and initial values, imported view/capture components, commands and optional artwork. Filenames besides the entry have no role. Vite resolves imports, component CSS, CSS fonts/images and exported assets. Build packs one SQLite template with scalar catalog columns, one `definition_json`, `ui.js`, optional `ui.css` and `commands.js`, and content-addressed `media/<sha256>.<ext>`. Artwork is stored separately. The build writes `packageFormat` and `runtimeABI`; authors never specify them.

Rust validates the definition and every listed resource. Assets are sealed by the app row. Templates contain a seed Loro checkpoint but no document identity, edits, source, dependencies, caches or runtime engine. The host supplies the shell; the UI bundles Svelte and the author SDK.

Creating a writable document copies the template and gives it its own identity; its
saved state, theme changes and attachments then live in the same file. The Rust Loro
core in the Swift host owns live data and saves it. WebViews apply publications. Initial
values seed only a new document. Never reconcile JSON files into state or open a .slop
file with SQLite. Use typed handles or the native CLI; `flush()` acknowledges
persistence. `slop inspect FILE` shows what a file holds.

slop.ts's `theme` declares the theme's colors; the page shell applies defaults and the
document's changes before mounting the app. ui.css contains compiled app styling.
A document also holds the colors it changed, saved with its edits. Use the theme panel
or slop theme get/set/reset/export/import. Fonts and layout changes require authoring
source and a rebuild. Close before moving documents; synced folders are unsupported.

Attachments hold opaque imported bytes, by SHA-256, in the document. Only the host
attachment API/CLI writes them, under existing ownership. References belong to Loro;
templates hold none. Duplicate, Share a Copy and export snapshots carry attachments.
Close/export flush accepted imports before proceeding.
