# Runtime reference (page shell, files, storage)

How an edit, a save and a close flow through the system is described in
[architecture](../architecture.md). This page is the reference for `.slop` files, storage
limits, security, capture and telemetry.

## Page shell and ctx

The page shell owns the page and the whole lifecycle: it opens the document, then
imports the app's `assets/app.js` and calls `default.mount(ctx, target)`
([abi.ts](../../packages/document/src/abi.ts)). The private `@hitslop/shell` package supplies the runtime; `@hitslop/document`
contains only the author SDK. `ctx` is the only thing an app may rely
on at run time: the document (snapshot, handles, `change`, `flush`, `subscribe`,
`issues`), `bind.text`, capture hooks, attachments, `window.resize` and
`reportError`; apps read the theme only as `--slop-*` CSS variables. The returned view supplies `rendered()` (wait for
pending UI updates) and `unmount()`. Reload replaces only the view; flush, close,
native readiness, themes, attachments and capture coordination stay in the page shell.

`@hitslop/document/svelte` is the Svelte adapter compiled into each app:
`svelteApp(App, { schema, export: Export, icon: Icon })` is the app's entry. The CLI
passes `schema.ts`'s default export and discovers optional `Export.svelte`,
`Icon.svelte`, and `styles.css` alongside `App.svelte`; authored `main.ts` takes
precedence. Once mounted, the schema's definition is the live document
(`ctx.document` with Svelte reactivity; a handle's `value` registers its read through
`ctx.document.observe`). `bindText`, `capture` and `attachments` forward to `ctx`. Svelte is an optional peer of the SDK. Other
frameworks supply `main.ts` exporting `default { mount(ctx, target) }` (type
`SlopApp` from `@hitslop/document/abi`), mark their root `data-hitslop-root`, and
use `ctx.capture` for custom export views.

An edit promise resolves after native acceptance and the corresponding local snapshot update, before durability or framework rendering. `flush()` and successful CLI mutations acknowledge local persistence. Renderer death retains accepted native edits; text not yet sent from a field can be lost. There is no network acknowledgement or second document engine.

The catalog combines immutable bundled starters, `~/.hitslop/templates`, and Recents. Users place `<slug>.slop` templates in that folder. Create makes a separate writable document. Bundled and installed templates have source-specific identities; categories come from local manifests. Account UI, OpenAPI/Registry, hosted discovery, and sharing are deferred.

## File layout

A `.slop` file is one SQLite database (application ID `0x534C4F50`, storage version in
`user_version`). TypeBox defines the manifest and platform envelopes. `slop.ts`
declares author, title, description, categories and presentation, the project folder's
name is the slug, and the stored manifest refuses any other field. `slop build` stages the app with `packageFormat`
from the builder and `runtimeABI` from the project's resolved SDK, and the file engine
stores them as columns beside the manifest. The host checks these independent
requirements before reading anything else; readers and app-facing context adapters
dispatch on their own requirement. Native and browser page configuration carry the
runtime ABI.

```text
app          one row: package_format, runtime_abi, manifest, descriptor (not JSON Schema),
             initial (creation-only values), theme (the palette's defaults)
assets       path → bytes: app.js (export default { mount(ctx, target) }), app.css, fonts…
artwork      preview and icon PNGs: built, then refreshed from documents' saved state
document     a document's identity (doc_id) and theme overrides; none in a template
checkpoint   the saved Loro snapshot and the schema key it was saved under
updates      saved Loro updates after the checkpoint
attachments  sha256 → bytes: imported files
```

A template has no `document`, `checkpoint`, `updates` or `attachments` rows; it never
opens as a document, and a command refuses it. Create copies a template into a new file
and gives it an identity. A template is immutable: it never opens as a document
(`is_template`), and bundled starters are also read-only on disk. Initial values seed only
a new document. Schema changes require new documents.

The page shell is served at `slop://app/__shell__/`, from the one shell bundled with the app. The document's assets are served at `slop://app/assets/`, whole or as byte ranges read from the file; nothing else in it is a resource, and the page receives the descriptor with its config. App bundles must not embed Loro or the document implementation. Preview serves the same shell plus the WASM core from the CLI, with disposable memory storage. No executable code is downloaded.

Every open checks the file before reading a value: the application ID, the storage version, the exact tables, one `app` row, the markers, the size of every value and asset, and asset paths. Opening a document to edit it, creating, packing and inspecting add SQLite's quick check; display-only opens (the catalog, Quick Look) leave it out. A template opened as a document is refused with `is_template`. A file a newer build wrote is refused with `requires_update` and left unchanged, as is a document whose layout is newer ([compatibility](../engineering-contract.md#compatibility)): connections never checkpoint when they close, and the writer is configured only after the checks pass, so a refused file keeps its bytes and its journal mode, including a newer build's WAL. The one write a read can cause is SQLite's own recovery: a crashed write's hot journal is rolled back first, restoring the last committed state. The CLI checks the helper's command protocol; the helper requires the live owner's exact core build.

## Persistence and ownership

The Rust store (`hitslop-core`'s `store`, on the platform SQLite) owns a document's saved state: the `document`, `checkpoint`, `updates` and `attachments` tables, DELETE journaling, synchronous EXTRA and macOS fullfsync, a 2-second busy timeout, incremental auto-vacuum, and opaque checkpoint/update bytes. Checkpoint replacement, covered-row deletion and freeing their pages are atomic. The exact canonical descriptor is the storage key. Nothing outside the core opens the file, so one SQLite library holds its locks. Every connection is defensive: no symbolic links, no trusted schema, cell-size checks, no memory mapping, and values no longer than the largest stored one. Readers beside the writer open read-write with `query_only`.

One OS flock, taken by the store, owns each document: on a lock file in the account's registry (`~/.hitslop/live/<device>-<inode>.lock`), never on the database. Lock files are never removed; never bypass a busy writer. The holder's discovery file beside it names its socket; an owner removes a crashed session's as soon as it takes the lock. A busy writer with unreachable discovery is an error, never permission for another writer. A document with a second hard link is refused, because SQLite names its journal after the path. A rename or replacement while open stops saving (`Moved`); moved back, the store reconnects and saves.

Save scheduling, the save job and the close sequence are described in [architecture](../architecture.md#saving). Checkpoint maintenance runs at 256 saved updates or 4 MiB. Native limits are 4,096 update rows and 32 MiB aggregate checkpoint/update bytes. Closing a document over 4 MiB that the session edited trims its history to that session at most; [architecture](../architecture.md#saving) has the rule. Oversized saves leave live edits pending, retain ownership and block close/export; explicit discard restores durable state under the same lock and remounts the renderer. Exact integer counter contributions replay through ordinary updates. `slop import` writes a JSON value as one `replace` operation ([CLI](../guides/cli.md#operations)).

Nothing resends a request: the CLI never replays a mutation, and after an uncertain result you run `get` before another edit. A live `get` returns owner-accepted state; text still in an open window's field is not included. `hello` supplies the owner's core build identity and epoch, which rotates when unsaved edits are discarded.

Failed saves retain ownership and native retry UI; cancel-close restores editing. Successful close removes discovery, destroys the WebView/bridge, drains storage, closes SQLite, and releases ownership; discovery is withdrawn before the writer lock is released, and restored if the close fails. Quit prepares every document before releasing any. Close before moving or renaming documents. iCloud and other synced folders are unsupported.

Opaque imported attachments live in the file's `attachments` table by SHA-256, outside Loro. `attachments.import(file, (tx, ref) => …)` commits the bytes, then submits the collector's reference edits; the close and capture barriers wait for it, so a blob is never saved without its reference. The store enforces the [attachment limits](../../apps/landing/src/content/docs/docs/guides/files-and-web.mdx#attachments) (10 MiB per file, 100 MiB and 256 files per document) and verifies hashes on read. Duplicate and Share a Copy carry attachments; templates contain none. Unreferenced blobs remain until a future explicit garbage-collection policy.

Theme overrides are bounded host presentation state, not a document projection: at most 256 declared colors in the file's `document` row, outside Loro, validated when changed and saved with the document's edits as [architecture](../architecture.md#themes-and-attachments) describes. The page shell applies the effective palette as CSS variables before mounting the app, and again whenever it changes. Loading never validates. Fonts, arbitrary CSS overrides and layout changes need authoring source and a rebuild.

## Opening and recovery

Document windows remain hidden until the page reports ready, which it does once the app has mounted and every web font it declares has loaded (each declared face is loaded directly from the app's assets, without forcing layout; bounded to 12 seconds, after which opening fails; a page with no `@font-face` skips the wait), then reveal directly and let WebKit paint normally. There is no extra animation-frame timeout or transparent-window staging. Opens taking more than one second from the original request display cancellable native progress. Checking the file, writer ownership and SQLite setup run on the preparation queue, and `slop://` assets are read off the main thread; cancellation disposes acquired storage before returning. Window skins reuse their validated image for that open. The hover toolbar and editor discovery are created only on first hover. Closing cancels presentation. Startup failures reveal native error UI. A document's artwork serves Quick Look, Finder and catalog display, not an opening placeholder.

Application-render errors and save failures have separate recovery paths. Renderer recovery retains the writer lease and the socket, and attaches a new WebView to the same owner, including its unsaved edits. Text not yet sent from the dead page cannot be recovered. Host and CLI exports flush before capture; expired captures cannot publish output.

## Security boundaries

Authored code can change or damage its own document. Runtime operation validation is not a separate security boundary from code sharing that page. Native code validates the file, bridge envelopes, and resource sizes. Credentials never belong in authored code.

The page shell synthesizes the page. The resource scheme exposes only the app's assets and the bundled page shell; saved state, attachments, artwork and the app's other columns are not resources. Decoded resource paths reject empty, dot and parent segments before anything else. An asset is read from the file through its own read-only connection, whole or as a range, never loading the rest; each is at most 25 MiB, and an app at most 256 assets and 50 MiB. Responses carry `Content-Length` and answer single byte ranges with 206, which WebKit's media loader requires for audio and video assets.

CSP permits local scripts and WebAssembly compiled at runtime (`'wasm-unsafe-eval'`; Soma Amp's MilkDrop compiles its presets this way), local and HTTPS connections/media, HTTPS frames, inline styles, and local/data/HTTPS/blob images. CORS remains enforced. Remote scripts and JavaScript eval remain blocked; fonts stay local/data. Native navigation cancels external navigation of the main frame; explicit HTTP(S) links in the app itself open in the system browser, while HTTPS sub-frames may load and navigate on their own and a click inside one never opens the browser. Camera/microphone grants are not part of this release.

Embedded frames are third-party web content inside the document's window. They cannot reach the bridge (main frame only), the file picker or downloads, and the web data store is non-persistent, so no cookies or logins reach them. Frames still expose the user to whatever page an author embeds, including hidden or phishing-styled frames. Revisit this policy (an allowlist, or frame origins the author declares and the host shows on open) before sharing or a hosted catalog ships.

A slop's page, `slop://app`, has no HTTP(S) referrer, and some providers refuse to embed without one (YouTube reports error 153; a `127.0.0.1` origin gets error 150). The SDK's `@hitslop/document/embed` embeds YouTube through a static relay page, `https://hitslop.com/embed/youtube.html`, served from `apps/landing/public/embed/`. It takes the video from the URL fragment, so the server never sees it, and relays only the player's allowlisted `postMessage` commands. YouTube playback therefore needs that page to be reachable; each additional referrer-gated provider needs its own relay and helper.

Bridge calls must originate in the main frame at `slop://app` and match generated TypeBox envelopes. Serialized requests are bounded to 48 MiB, with storage bounds checked before blob materialization. SQLite uses NOFOLLOW and `trusted_schema=OFF`. Socket request/reply bounds are 1 MiB/16 MiB, one request per connection, with bounded concurrency/timeouts. The server command deadline is 30 seconds; the client waits 35 seconds.

An export never replaces the document it renders. Capture stages output and publishes by atomic rename before its deadline. Failures do not replace existing output. A lost acknowledgement leaves an uncertain outcome; inspect the destination before retrying.

Slop-initiated blob/data downloads require a native save confirmation. Download bytes receive quarantine metadata before atomic installation, including replacement of existing files; a quarantine write failure leaves the destination unchanged.

Bridge resize requests respect the manifest’s resizable setting, including fixed-size skin windows. The validated `shape` becomes one immutable native silhouette that supplies layer clipping, hit testing and fallback PNG masking; authors' rules for shapes are in [manifest and windows](../../apps/landing/src/content/docs/docs/guides/manifest-and-windows.mdx).

## Capture and Finder integration

`svelteApp` mounts an internal editor boundary and optional lazy export/icon components against the existing document. The export component receives `mode: "preview" | "export"`. Framework-neutral targets must be direct body children. Native code consumes controller geometry and its `dedicated` flag rather than a separate DOM-marker protocol.

The host's window-filling sizing rules have zero specificity and are disabled during capture; the page reset remains. The Svelte icon target owns a transparent 512×512 canvas and centers authored artwork within it. Capture restoration uses the current window container if the user resized during capture.

Export and icon components have separate rendering-error boundaries. A component rendering failure rejects only that capture; restoration preserves the editor and clears the component failure for a later attempt. Editor rendering failures continue to report through the native application-error recovery path and prevent capture, including when authored code throws a falsy value.

Capture commits drafts, flushes persistence, waits for fonts, visible images, and stable layout, and blocks edits. Success and failure restore focus, selection, scroll, styles, and input rendering. Dedicated exports do not inherit native masks. Fallback capture can hide marked editing controls and replace native text inputs with wrapping text.

PDF recomposes WebKit's internal pages into one continuous page when needed. Output sizes are in [icons and exports](../../apps/landing/src/content/docs/docs/guides/icons-and-exports.mdx#export-from-the-host-or-cli).

Native PNG compression runs system zlib off the main actor. It preserves pixels, dimensions, transparency, and metadata, removes alpha only for fully opaque images, and retains the smallest successful candidate or the original. It covers template artwork, refreshed previews, Finder icon sources, and PNG exports; it does not rewrite existing artwork or PDFs.

Build and register render artwork the project does not supply from a draft of the template, before packing it. New documents copy their template's artwork. Close renders the preview and, when an icon target exists, the icon from a saved-state snapshot, then writes them into the file only while it still holds that state (a document opened or edited again meanwhile keeps its artwork). Nothing is written outside the file: no custom icon or extended attribute. The catalog reads the refreshed artwork and updates without reopening. Failed captures or writes retain the previous artwork. Quick Look extensions show the artwork in Finder, Mail and the share sheet, for documents that left this Mac too: sandboxed, read-only, through the core. Neither catalog, Finder nor Quick Look display loads the document engine.

Live exports use the current editor width and selected view. Closed exports render a saved-state snapshot with the app's initial view.

## Telemetry

Release enables Firebase Analytics and Crashlytics after configuration; Debug/tests do not initialize Firebase. Collection defaults off in the app plist. Auth/App Check remain deferred. Existing Analytics events record launch, creation source, opening, duplication, and export format. Cancelled operations produce only fixed breadcrumbs.

Non-fatals carry a fixed operation, classification (platform/authored/rejection), and reason. Native storage and WebKit callbacks supply diagnostic categories before error text is flattened. Fixed category-specific domains and stable numeric codes distinguish issue groups; per-event keys can include export format. No document IDs, paths, titles, contents, guest codes, authored error strings, or raw error userInfo enter reports. Document context never uses global Crashlytics keys.

Save and renderer incidents report once until recovery; propagated close/quit/export errors add breadcrumbs. Authored errors, expected rejections, and background catalog/artwork failures share a two-report limit per app launch, once per category, and stop after the first foreground platform failure. Optional artwork and stale Recents remain normal fallbacks. Standalone CLI processes have no Firebase sink.

Release validation requires actual Firebase delivery and symbolication; unit tests cannot establish those. Use a disposable validation build and retain its dSYM and dashboard evidence, without shipping a crash trigger.

## Schema identity

Saved state belongs to the descriptor it was saved under. The store records the
descriptor (the core's canonical serialization) with the checkpoint and compares it with
the app's by meaning when it opens (`same_schema`): key order and number spelling
never matter, any other difference refuses the saved state. Apps check a second key: the
SDK compiled into each `app.js` computes the descriptor's canonical JSON (keys sorted
recursively, array order kept, no whitespace, JavaScript number formatting) and refuses
to mount under a shell that computes a different one. Every released app.js carries that
algorithm, so it is frozen by golden vectors in `packages/document/tests/schema.test.ts`.
Schema evolution is deferred.

The native page protocol has one request/reply envelope for document edits and host
services. TypeBox owns it in `@hitslop/schema/page`; core payloads are in
`@hitslop/schema/core`. Requests carry no correlation ID or view token: WebKit
correlates promises and Swift supplies lifecycle identity after checking the sender.
The host enters the shell through `__slop` for publications, capture and lifecycle.
Apps use the restricted `ctx.document` facade and its explicit durability barrier,
`flush()`. Themes arrive as effective values (the app's defaults come from `slop.ts`'s `theme`), while
authored layout stays in CSS.

Swift encodes replies with the generated `PageResult`, and Rust validates requests
against the generated schemas. The shell checks only each reply's outcome envelope and
does not bundle TypeBox.
