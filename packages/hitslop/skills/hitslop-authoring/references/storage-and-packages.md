# Source, templates, and documents

Source contains slop.ts (the app's metadata, theme colors and initial values),
schema.ts (the TypeScript descriptor definition), components, and plain CSS. Build packs
one template file (`.slop`, a SQLite database): the manifest, the descriptor, initial
values, theme defaults, the `packageFormat` and `runtimeABI` the file requires (written
only by build; never add them to source), the compiled app module (assets/app.js, app.css) and assets, and
preview and icon artwork when supplied (`artwork/preview.png`, `artwork/icon.png`) or
captured natively. Build checks the template the way the app opens it. Templates contain
no document state, source, dependencies, caches, or stores.

Creating a writable document copies the template and gives it its own identity; its
saved state, theme changes and attachments then live in the same file. The Rust Loro
core in the Swift host owns live data and saves it. WebViews apply publications. Initial
values seed only a new document. Never reconcile JSON files into state or open a .slop
file with SQLite. Use typed handles or the native CLI; `flush()` acknowledges
persistence. `slop inspect FILE` shows what a file holds.

slop.ts's `theme` declares the theme's colors; the page shell applies defaults and the
document's changes before mounting the app. assets/app.css contains compiled app styling.
A document also holds the colors it changed, saved with its edits. Use the theme panel
or slop theme get/set/reset/export/import. Fonts and layout changes require authoring
source and a rebuild. Close before moving documents; synced folders are unsupported.

Attachments hold opaque imported bytes, by SHA-256, in the document. Only the host
attachment API/CLI writes them, under existing ownership. References belong to Loro;
templates hold none. Duplicate, Share a Copy and export snapshots carry attachments.
Close/export flush accepted imports before proceeding.
