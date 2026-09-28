# Source, templates, and documents

Source contains the manifest, TypeScript descriptor definition, initial values,
theme tokens, components, and plain CSS. Build compiles the immutable app module (assets/app.js, app.css) and assets,
state.schema.json, initial.json, runtime requirements, and document guidance.
Native capture adds QuickLook artwork. Templates contain no mutable state,
source, dependencies, caches, or stores.

Creating a writable copy adds state/document.sqlite and ownership files. Loro
in WebKit owns live data; Swift persists opaque bytes. Initial values seed only
a new document. Never reconcile JSON files into state or edit SQLite directly.
Use typed handles or the native CLI; `flush()` acknowledges persistence.

assets/theme.json declares token defaults; the runtime applies defaults and overrides
before mounting the app. assets/app.css contains compiled app styling.
state/theme.json contains bounded declared-token overrides written by the host.
Use slop theme get/set/reset, never direct edits to these files. Layout changes require
authoring source and a rebuild. Close before moving documents; synced folders are unsupported.
Shipped v1 runtime contracts remain supported; pre-v1 formats are not migrated.

Optional `state/attachments/<sha256>` files hold opaque imported bytes. Only the
host attachment API/CLI writes them, under existing ownership. References belong
to Loro; templates remain free of mutable state. Duplicate/export snapshots copy
attachments. Close/export flush accepted imports before proceeding.
