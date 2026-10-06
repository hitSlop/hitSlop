# Runtime reference (page shell, files, storage)

How an edit, a save and a close flow through the system is described in
[architecture](../architecture.md). This page is the reference for `.slop` files, storage
limits, security, capture and telemetry.

## Page shell and ctx

The page shell owns the page and the whole lifecycle: it opens the document, then
imports the app's `assets/app.js` and calls `default.mount(ctx, target)`
([abi.ts](../../packages/document/src/abi.ts)). The private `@hitslop/shell` package supplies the runtime; `@hitslop/document`
contains only the author SDK. `ctx` is the only thing an app may rely
on at run time: the document (snapshot, handles, `change`, `flush`, `subscribe`),
`bind.text`, capture hooks, attachments, `window.resize` and
`reportError`; apps read the theme only as `--slop-*` CSS variables. The returned view may supply `rendered()` (wait for
pending UI updates) and `unmount()`. Reload replaces only the view; flush, close,
native readiness, themes, attachments and capture coordination stay in the page shell.

`@hitslop/document/svelte` is the Svelte adapter compiled into each app:
`svelteApp(App, { schema, export: Export, icon: Icon })` is the app's entry. The CLI
passes `schema.ts`'s default export and discovers optional `Export.svelte`,
`Icon.svelte`, and `styles.css` alongside `App.svelte`. Custom `main.ts` entries
are refused. Once mounted, the schema's definition is the live document
(`ctx.document` with Svelte reactivity; a handle's `value` registers its read through
`ctx.document.observe`). `bindText`, `capture` and `attachments` forward to `ctx`. The generated entry is the one authoring path. Capture components remain optional.

An edit promise resolves after native acceptance and the corresponding local snapshot update, before durability or framework rendering. `flush()` and successful CLI mutations acknowledge local persistence. Renderer death retains accepted native edits; text not yet sent from a field can be lost. There is no network acknowledgement or second document engine.

The catalog combines immutable bundled starters, `~/.hitslop/templates`, and Recents. Users place `<slug>.slop` templates in that folder. Create makes a separate writable document. Bundled and installed templates have source-specific identities; categories come from local manifests. Account UI, OpenAPI/Registry, hosted discovery, and sharing are deferred.

## File layout

A `.slop` file is one SQLite database (application ID `0x48534C50`, "HSLP"; storage version in
`user_version`). TypeBox defines the manifest and platform envelopes. `slop.ts`
declares author, title, description, categories and presentation, the project folder's
name is the slug, and the stored manifest refuses any other field. `slop build` stages the app with `packageFormat`
from the builder and `runtimeABI` from the project's resolved SDK, and the file engine
stores them as columns beside the manifest. The host checks these independent
requirements before reading anything else, so a page never opens an app that needs a
newer runtime ABI.

```text
app          one row: package_format, runtime_abi, manifest, descriptor (not JSON Schema),
             theme (the palette's defaults)
assets       path → bytes: app.js (export default { descriptor, mount(ctx, target) }), app.css, fonts…
artwork      preview and icon PNGs: built, then rewritten as an edited document's window closes
document     a document's identity; none in a template
checkpoint   the saved Loro snapshot, including data and theme overrides (a template's holds the initial values)
updates      saved Loro updates after the checkpoint
attachments  sha256 → bytes: imported files
```

A template has no `document`, `updates` or `attachments` rows; it never
opens as a document, and a command refuses it. Create copies a template into a new file
and atomically adds its identity. A template is immutable (`is_template` refuses it as a
document), and bundled starters are also read-only on disk. Initial values seed only
a new document. Schema changes require new documents.

The page shell is served at `slop://app/__shell__/`, from the one shell bundled with the app. The document's assets are served at `slop://app/assets/`, whole or as byte ranges read from the file; nothing else in it is a resource, and the page receives the descriptor with its config. App bundles must not embed Loro or the document implementation. Preview serves the same shell plus the WASM core from the CLI, with disposable memory storage. No executable code is downloaded.

Every open checks the file before reading a value: the application ID, the storage version, the `packageFormat` and `runtimeABI` markers, the exact tables, one `app` row, the size of every value and asset, and asset paths. Opening a document to edit it, creating, packing and inspecting add SQLite's quick check; display-only opens (the catalog, Quick Look) leave it out. A template opened as a document is refused with `is_template`. A file a newer build wrote is refused with `requires_update` and left unchanged, as is a document whose layout is newer ([compatibility](../engineering-contract.md#compatibility)): connections never checkpoint when they close, and the writer is configured only after the checks pass, so a refused file keeps its bytes and its journal mode, including a newer build's WAL. The one write a read can cause is SQLite's own recovery: a crashed write's hot journal is rolled back first, restoring the last committed state. The CLI checks the selected engine or helper's command protocol; the shared Rust router requires a live owner's exact core build.

## Persistence and ownership

The Rust store (`hitslop-core`'s `store`, on the platform SQLite) owns a document's saved state: the `document`, `checkpoint`, `updates` and `attachments` tables, DELETE journaling, synchronous EXTRA and macOS fullfsync, a 2-second busy timeout, full auto-vacuum, and opaque checkpoint/update bytes. Checkpoint replacement, covered-row deletion and freeing their pages are atomic. Nothing outside the core opens the file, so one SQLite library holds its locks. Every connection is defensive: no symbolic links, no trusted schema, cell-size checks, no memory mapping, and values no longer than the largest stored one. Readers beside the writer open read-write with `query_only`.

One OS flock, taken by the store, owns each document: on a lock file in the account's registry (`~/.hitslop/live/<device>-<inode>.lock`), never on the database. Lock files are never removed; never bypass a busy writer. The holder's discovery file beside it names its socket; an owner removes a crashed session's as soon as it takes the lock. A busy writer with unreachable discovery is an error, never permission for another writer. A document with a second hard link is refused, because SQLite names its journal after the path. A rename or replacement while open stops saving (`Moved`); moved back, the store reconnects and saves.

Save scheduling, the save job and the close sequence are described in [architecture](../architecture.md#saving). Checkpoint maintenance runs at 256 saved updates or 4 MiB. Native limits are 4,096 update rows and 32 MiB aggregate checkpoint/update bytes. Closing a document over 4 MiB that the session edited trims its history: it leaves none; [architecture](../architecture.md#saving) has the rule. Oversized saves leave live edits pending, retain ownership and block close/export; explicit discard restores durable state under the same lock and remounts the renderer. Exact integer counter contributions replay through ordinary updates. `slop import` writes a JSON value as one `replace` operation ([CLI](../guides/cli.md#operations)).

Nothing resends a request: the CLI never replays a mutation, and after an uncertain result you run `get` before another edit. A live `get` returns owner-accepted state; text still in an open window's field is not included. Every request names its command protocol, which the owner checks before reading anything else, so an engine of another build is refused with which side to update. A reply carries its state whole, as large as the document; the CLI validates each reply's method and required result fields (`SocketReply`). A live and a closed `get` of one saved state print the same value.

Failed saves retain ownership and native retry UI; cancel-close restores editing. Successful close removes discovery, destroys the WebView/bridge, drains storage, closes SQLite, and releases ownership; discovery is withdrawn before the writer lock is released, and restored if the close fails. Quit prepares every document before releasing any. Close before moving or renaming documents. iCloud and other synced folders are unsupported.

Opaque imported attachments live in the file's `attachments` table by SHA-256, outside Loro. `attachments.import(file, (tx, ref) => …)` stores the bytes, then submits the collector's reference edits; the close and capture barriers wait for both. An agent's `batch` carries its files (`slop apply --attach`), stored before its operations in the same request, so no close falls between a blob and its reference. A collector that throws, or an edit the core refuses, leaves the stored blob unreferenced until the document closes: after the final save, the owner deletes every blob whose ID appears in no string, text or map key of the saved state (`Store::reclaim_attachments`). Undo covers the open session only, so retained history is not a reason to keep a blob. The store enforces the [attachment limits](../../apps/landing/src/content/docs/docs/guides/files-and-web.mdx#attachments) (10 MiB per file, 100 MiB and 256 files per document) and verifies hashes on read. Duplicate and Share a Copy carry only the attachments their state references, with no history and artwork rendered for the copy (`Store::copy_clean`); templates contain none.

Theme overrides live in the Loro root `theme` map: at most 256 declared colors, checked on write and on open. They share the document's sequence, undo history and save jobs. The initial state carries the effective palette; ordered publications carry it when it changes. The shell applies it as CSS variables before mounting the app and after each theme publication. JSON document replacement changes data and preserves the palette. Fonts, arbitrary CSS overrides and layout changes need authoring source and a rebuild.

## Opening and recovery

Document windows remain hidden until the page reports ready, which it does once the app has mounted and every web font it declares has loaded (each declared face is loaded directly from the app's assets, without forcing layout; bounded to 12 seconds, after which opening fails; a page with no `@font-face` skips the wait), then reveal directly and let WebKit paint normally. There is no extra animation-frame timeout or transparent-window staging. Opens taking more than one second from the original request display cancellable native progress. Checking the file, writer ownership and SQLite setup run on the preparation queue, and `slop://` assets are read off the main thread; cancellation disposes acquired storage before returning. Window skins reuse their validated image for that open. The hover toolbar and editor discovery are created only on first hover. Closing cancels presentation. Startup failures reveal native error UI. A document's artwork serves Quick Look, Finder and catalog display, not an opening placeholder.

Application-render errors and save failures have separate recovery paths. Renderer recovery retains the writer lease and the socket, and attaches a new WebView to the same owner, including its unsaved edits. Text not yet sent from the dead page cannot be recovered. Host and CLI exports flush before capture; expired captures cannot publish output.

## Security boundaries

Authored code can change or damage its own document. Runtime operation validation is not a separate security boundary from code sharing that page. Native code validates the file, bridge envelopes, and resource sizes. Credentials never belong in authored code.

The page shell synthesizes the page. The resource scheme exposes only the app's assets and the bundled page shell; saved state, attachments, artwork and the app's other columns are not resources. Decoded resource paths reject empty, dot and parent segments before anything else. An asset is read from the file through its own connection, opened read-write with `query_only` as every reader beside the writer is, whole or as a range. Packing stores text (HTML, JavaScript, CSS, JSON, SVG) and WebAssembly Brotli-compressed when that is smaller, and is decoded whole to serve; everything else is stored as it is and read by range without loading the rest. Limits count decoded bytes: each asset is at most 25 MiB, and an app at most 256 assets and 50 MiB. Responses carry `Content-Length` and answer single byte ranges with 206, which WebKit's media loader requires for audio and video assets.

The native and browser CSPs come from `packages/schema/src/policy.ts`; only local origins and the development HMR connection differ. CSP permits local scripts and WebAssembly compiled at runtime (`'wasm-unsafe-eval'`; Soma Amp's MilkDrop compiles its presets this way), local and HTTPS connections/media, HTTPS frames, inline styles, and local/data/HTTPS/blob images. CORS remains enforced. Remote scripts and JavaScript eval remain blocked; fonts stay local/data. Native navigation cancels external navigation of the main frame; explicit HTTP(S) links in the app itself open in the system browser, while HTTPS sub-frames may load and navigate on their own and a click inside one never opens the browser. Camera/microphone grants are not part of this release.

Embedded frames are third-party web content inside the document's window. They cannot reach the bridge (main frame only), the file picker or downloads, and the web data store is non-persistent, so no cookies or logins reach them. Frames still expose the user to whatever page an author embeds, including hidden or phishing-styled frames. Revisit this policy (an allowlist, or frame origins the author declares and the host shows on open) before sharing or a hosted catalog ships.

A slop's page, `slop://app`, has no HTTP(S) referrer, and some providers refuse to embed without one (YouTube reports error 153; a `127.0.0.1` origin gets error 150). The SDK's `@hitslop/document/embed` embeds YouTube through a static relay page, `https://hitslop.com/embed/youtube.html`, served from `apps/landing/public/embed/`. It takes the video from the URL fragment, so the server never sees it, and relays only the player's allowlisted `postMessage` commands. YouTube playback therefore needs that page to be reachable; each additional referrer-gated provider needs its own relay and helper.

Bridge calls must originate in the main frame at `slop://app` and match generated TypeBox envelopes. Serialized requests are bounded to 48 MiB, with storage bounds checked before blob materialization. SQLite uses NOFOLLOW and `trusted_schema=OFF`. Socket requests are bounded to 1 MiB (16 MiB for a batch carrying attachments), one request per connection, with bounded concurrency/timeouts. The server command deadline is 30 seconds; the client waits 35 seconds.

An export never replaces the document it renders. Capture stages output and publishes by atomic rename before its deadline. Failures do not replace existing output. A lost acknowledgement leaves an uncertain outcome; inspect the destination before retrying.

Slop-initiated blob/data downloads require a native save confirmation. Download bytes receive quarantine metadata before atomic installation, including replacement of existing files; a quarantine write failure leaves the destination unchanged.

Bridge resize requests respect the manifest’s resizable setting, including fixed-size skin windows. The validated `shape` becomes one immutable native silhouette that supplies layer clipping, hit testing and fallback PNG masking; authors' rules for shapes are in [manifest and windows](../../apps/landing/src/content/docs/docs/guides/manifest-and-windows.mdx).

## Capture and Finder integration

Every host capture uses a disposable read-only page. An open document first drains its
page writes, saves and backs up SQLite to a temporary `.slop`; the editor capture barrier
ends after the backup. Rendering then uses that independent copy, including its app,
attachments, data and theme. The editor's focus, selection, scroll and local selected
view are not used or changed. No read transaction remains open during rendering.

The generated Svelte entry discovers optional `Export.svelte` and `Icon.svelte`.
`Export.svelte` receives `mode: "preview" | "export"` and supplies the capture layout.
Without it, a fresh `App.svelte` renders saved data using its default local UI state.
State that should determine an export, such as a selected report, must be saved in the
document. Capture components read the same document facade; they cannot change saved
state to prepare their view.

The capture page waits for fonts, visible images and stable layout. A dedicated export
hides the editor before layout and does not inherit the native window mask. The App
fallback can hide `data-slop-export="hide"` controls and replace native text inputs with
wrapping text. The icon target owns a transparent 512×512 canvas and centers authored
artwork. The host's window-filling sizing rules are disabled during capture; the page
reset remains. Rendering errors reject the capture, and the disposable page is closed.

PDF recomposes WebKit's internal pages into one continuous page when needed. Output sizes are in [icons and exports](../../apps/landing/src/content/docs/docs/guides/icons-and-exports.mdx#export-from-the-host-or-cli).

The core stores artwork losslessly optimized with oxipng: `pack` at level 2, and a closing window's artwork at level 0, because the writer lock is released only after it is written ([measurements](../evidence/artwork-optimization-2026-10-05.md)). Pixels, dimensions and transparency are preserved, fully transparent pixels keep their colour, alpha is removed only from fully opaque images, and metadata that does not affect display is stripped. A result is kept only when it is smaller; artwork oxipng cannot read, or would decode past 64 MiB, is stored as it is. PNG and PDF exports are written as rendered.

Build and register render artwork the project does not supply from a draft of the template, before packing it. New documents copy their template's artwork. A window closing a document its session changed, or one without a preview, renders the preview and, when an icon target exists, the icon from a saved backup in a fresh read-only page; the owner writes them through its writer connection before the file closes. The file's Finder custom icon is a local copy of that artwork (the icon, else the preview), written after a close that wrote artwork, at create and at copy; Finder shows it without Quick Look's icon-mode tile. It is file metadata, never a source: tools that drop metadata lose only the copy, and Finder falls back to the Quick Look thumbnail. The catalog reads the new artwork and updates without reopening. A failed capture or write keeps the previous artwork and never stops the close; a close that fails shows the window again. Quick Look extensions show the artwork in Finder, Mail and the share sheet, for documents that left this Mac too: sandboxed, read-only, through the core. Catalog, Finder and Quick Look display read stored artwork without running authored code or creating a live document owner.

Open and closed exports use fresh saved-state pages with default local UI state. An open export uses the flushed backup described above; a closed export takes no writer lock and can render while another process holds the document.

## Telemetry

Release enables Firebase Analytics and Crashlytics after configuration; Debug/tests do not initialize Firebase. Collection defaults off in the app plist. Auth/App Check remain deferred. Existing Analytics events record launch, creation source, opening, duplication, and export format. Cancelled operations produce only fixed breadcrumbs.

Non-fatals carry a fixed operation, classification (platform/authored/rejection), and reason. Native storage and WebKit callbacks supply diagnostic categories before error text is flattened. Fixed category-specific domains and stable numeric codes distinguish issue groups; per-event keys can include export format. No document IDs, paths, titles, contents, guest codes, authored error strings, or raw error userInfo enter reports. Document context never uses global Crashlytics keys.

Save and renderer incidents report once until recovery; propagated close/quit/export errors add breadcrumbs. Authored errors, expected rejections, and background catalog/artwork failures share a two-report limit per app launch, once per category, and stop after the first foreground platform failure. Optional artwork and stale Recents remain normal fallbacks. Standalone CLI processes have no Firebase sink.

Release validation requires actual Firebase delivery and symbolication; unit tests cannot establish those. Use a disposable validation build and retain its dSYM and dashboard evidence, without shipping a crash trigger.

## Schema identity

Saved state belongs to the descriptor in the `app` row it is stored with. Packing
writes that row once and a new document copies it from its template, so a document's
state and its descriptor never part. The app declares the descriptor it was built for (`SlopApp.descriptor`; `svelteApp` sets it from its schema), and the
shell refuses to mount an app on a document of another: key order never matters. Schema evolution is deferred.

The native page protocol has one request/reply envelope for document edits and host
services. Text edits are batches: a binding's `apply` names its `base` and a `set` with
`from` and `selection`, and the reply adds `authored` and the merged selection
([architecture](../architecture.md#text)). TypeBox owns it in `@hitslop/schema/page`; core payloads are in
`@hitslop/schema/core`. Requests carry no correlation ID or view token: WebKit
correlates promises and Swift supplies lifecycle identity after checking the sender.
The host enters the shell through `__slop` for publications, capture and lifecycle.
Apps use the restricted `ctx.document` facade and its explicit durability barrier,
`flush()`. The initial owner state includes effective theme values, and ordered publications include
them when they change (defaults come from `slop.ts`'s `theme`). Authored layout stays in CSS.

Swift encodes replies with the generated `PageResult`, and Rust validates requests
against the generated schemas. The shell checks only each reply's outcome envelope and
does not bundle TypeBox.
