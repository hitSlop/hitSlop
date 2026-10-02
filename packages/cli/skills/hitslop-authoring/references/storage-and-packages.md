# Source, templates, and documents

Source contains the manifest, TypeScript descriptor definition, initial values,
theme colors, components, and plain CSS. Build compiles the immutable app module (assets/app.js, app.css) and assets,
state.schema.json, initial.json, runtime requirements, and document guidance.
Native capture adds QuickLook artwork. Templates contain no mutable state,
source, dependencies, caches, or stores.

Creating a writable copy adds state/document.sqlite and ownership files. The Rust Loro core
in the Swift host owns live data; Swift persists its bytes. WebViews apply publications. Initial values seed only
a new document. Never reconcile JSON files into state or edit SQLite directly.
Use typed handles or the native CLI; `flush()` acknowledges persistence.

assets/theme.json declares the theme's colors; the page shell applies defaults and the
document's changes before mounting the app. assets/app.css contains compiled app styling.
state/document.sqlite also holds the colors a document changed, saved with its edits.
Use the theme panel or slop theme get/set/reset/export/import, never direct edits to
these files. Fonts and layout changes require
authoring source and a rebuild. Close before moving documents; synced folders are unsupported.

Optional `state/attachments/<sha256>` files hold opaque imported bytes. Only the
host attachment API/CLI writes them, under existing ownership. References belong
to Loro; templates remain free of mutable state. Duplicate/export snapshots copy
attachments. Close/export flush accepted imports before proceeding.
