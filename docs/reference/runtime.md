# Runtime reference (page shell, packages, storage)

How an edit, a save and a close flow through the system is described in
[architecture](../architecture.md). This page is the reference for packages, storage
limits, security, capture and telemetry.

## Page shell and ctx

The page shell owns the page and the whole lifecycle: it opens the document, then
imports the package's `assets/app.js` and calls `default.mount(ctx, target)`
([abi.ts](../../packages/document/src/abi.ts)). `ctx` is the only thing an app may rely
on at run time: the document (snapshot, handles, `change`, `flush`, `subscribe`,
`issues`), `bind.text`/`bind.value`, capture hooks, attachments, theme,
`window.resize` and `reportError`. The returned view supplies `rendered()` (wait for
pending UI updates) and `unmount()`. Reload replaces only the view; flush, close,
native readiness, themes, attachments and capture coordination stay in the page shell.

`@hitslop/document/svelte` is the Svelte adapter compiled into each app:
`defineSlop(App, { schema, export: Export, icon: Icon })` is the package entry. The CLI
passes `schema.ts`'s default export and discovers optional `Export.svelte`,
`Icon.svelte`, and `styles.css` alongside `App.svelte`; authored `main.ts` takes
precedence. Once mounted, the schema's definition is the live document
(`ctx.document` with Svelte reactivity; a handle's `value` registers its read through
`ctx.document.observe`). `bindText`, `capture` and `attachments` forward to `ctx`. Svelte is an optional peer of the SDK. Other
frameworks supply `main.ts` exporting `default { mount(ctx, target) }` (type
`SlopApp` from `@hitslop/document/abi`), mark their root `data-hitslop-root`, and
use `ctx.capture` for custom export views.

An edit promise resolves after native acceptance and the corresponding local snapshot update, before durability or framework rendering. `flush()` and successful CLI mutations acknowledge local persistence. Renderer death retains accepted native edits; text not yet sent from a field can be lost. There is no network acknowledgement or second document engine.

The catalog combines immutable bundled starters, `~/.hitslop/templates`, and Recents. Users unpack external downloads before placing valid `<slug>.slop` masters in that folder. Create makes a separate writable document. Bundled and installed templates have source-specific identities; categories come from local manifests. Account UI, OpenAPI/Registry, hosted discovery, and sharing are deferred.

## Package layout

TypeBox defines the manifest and platform envelopes. The manifest requires author, slug, title, description, categories, and presentation; `$schema` is an optional editor hint.
Readers tolerate unknown metadata and named enum values without rewriting manifest bytes; writers and known-field validation remain strict.

```text
Example.slop/
  manifest.json
  assets/                       immutable compiled code, CSS, and resources
    app.js                      app module: export default { mount(ctx, target) }
    app.css                     compiled app styling
    theme.json                  declared token defaults
  state.schema.json             root object descriptor, not JSON Schema
  initial.json                  immutable creation-only values
  .agents/skills/hitslop-document/SKILL.md
  QuickLook/
    Preview.png                 refreshed in writable documents
    Icon.png                    optional icon, refreshed in writable documents
  state/                        writable documents only
    document.sqlite             opaque checkpoint/update bytes, doc_id, theme overrides
    writer.lock                 permanent ownership inode
    host.lock                   live socket discovery
    attachments/<sha256>        optional immutable imported blobs
```

Templates contain no `state`, `stores`, source, dependencies, caches, or editable structural stylesheets. Builds add document guidance; missing or changed guidance does not prevent opening an existing document. A built or registered master is immutable and must be copied before editing. Initial values seed only a new database. Schema changes require new documents.

The page shell is served at `slop://app/__shell__/`, from the one shell bundled with the app. App bundles must not embed Loro or the document implementation. Preview serves the same shell plus the WASM core from the CLI, with disposable memory storage. No executable code is downloaded.

Packages carry no runtime metadata: a package is valid when its manifest, descriptor, initial data and assets validate. Earlier 1.x builds are unsupported. Populated SQLite storage must carry the hitSlop application ID and supported storage version. Native commands require matching CLI/helper/live-owner core build identities; there is no negotiation or migration.

## Persistence and ownership

The Rust store (`hitslop-core`'s `store`, on the platform SQLite) owns `state/document.sqlite`: three tables (`document` for the identity and theme overrides, `checkpoint`, `updates`), DELETE journaling, synchronous EXTRA and macOS fullfsync, a 2-second busy timeout, incremental auto-vacuum for new files, and opaque checkpoint/update bytes. Checkpoint replacement, covered-row deletion and freeing their pages are atomic. The exact canonical descriptor is the storage key. Nothing else opens the database, so one SQLite library holds its locks.

One OS flock on permanent `state/writer.lock`, taken by the store, owns each local package. Never unlink it or bypass a busy writer. `state/host.lock` is discovery only; an owner removes a leftover one from a crashed session as soon as it takes the lock. A busy writer with unreachable discovery is an error, never permission for another writer.

Save scheduling, the save job and the close sequence are described in [architecture](../architecture.md#saving). Checkpoint maintenance runs at 256 saved updates or 4 MiB. Native limits are 4,096 update rows and 32 MiB aggregate checkpoint/update bytes. Closing a document over 4 MiB that the session edited trims its history to that session at most; [architecture](../architecture.md#saving) has the rule. Oversized saves leave live edits pending, retain ownership and block close/export; explicit discard restores durable state under the same lock and remounts the renderer. Exact integer counter contributions replay through ordinary updates. `slop import` writes a JSON value as one `replace` operation ([CLI](../guides/cli.md#operations)).

Nothing resends a request: the CLI never replays a mutation, and after an uncertain result you run `get` before another edit. A live `get` returns owner-accepted state; text still in an open window's field is not included. `hello` supplies the owner's core build identity and epoch, which rotates when unsaved edits are discarded.

Failed saves retain ownership and native retry UI; cancel-close restores editing. Successful close removes discovery, destroys the WebView/bridge, drains storage, closes SQLite, and releases ownership; discovery is withdrawn before the writer lock is released, and restored if the close fails. Quit prepares every document before releasing any. Close before moving or renaming packages. iCloud and other synced folders are unsupported.

Opaque imported attachments live at `state/attachments/<sha256>`, outside Loro. `attachments.import(file, (tx, ref) => …)` stores and fsyncs the bytes, then submits the collector's reference edits; the close and capture barriers wait for it, so a blob is never saved without its reference. The owner enforces the [attachment limits](../../apps/landing/src/content/docs/docs/guides/files-and-web.mdx#attachments) (10 MiB per file, 100 MiB and 256 files per document), rejects links, and verifies hashes on read. Duplication copies attachments; runtime templates contain none. Unreferenced blobs remain until a future explicit garbage-collection policy.

Theme overrides are bounded host presentation state, not a document projection: at most 64 KiB of declared tokens in `state/document.sqlite`, outside Loro, validated on write as [architecture](../architecture.md#themes-and-attachments) describes. The page shell applies defaults and overrides as CSS variables before mounting the app. Loading never validates; the browser ignores CSS it cannot parse. Arbitrary CSS override files are unsupported, and layout changes need authoring source and a rebuild.

## Opening and recovery

Document windows remain hidden until the page reports ready, which it does once the app has mounted and every web font it declares has loaded (each declared face is loaded directly from the package, without forcing layout; bounded to 12 seconds, after which opening fails; a page with no `@font-face` skips the wait), then reveal directly and let WebKit paint normally. There is no extra animation-frame timeout or transparent-window staging. Opens taking more than one second from the original request display cancellable native progress. Package validation, writer ownership and SQLite setup run on the preparation queue, and `slop://` package files are read off the main thread; cancellation disposes acquired storage before returning. Window skins reuse their validated image for that open. The hover toolbar and editor discovery are created only on first hover. Closing cancels presentation. Startup failures reveal native error UI. Quick Look artwork serves Finder and catalog display, not an opening placeholder.

Application-render errors and save failures have separate recovery paths. Renderer recovery retains the writer lease and the socket, and attaches a new WebView to the same owner, including its unsaved edits. Text not yet sent from the dead page cannot be recovered. Host and CLI exports flush before capture; expired captures cannot publish output.

## Security boundaries

Authored code can change or damage its own document. Runtime operation validation is not a separate security boundary from code sharing that page. Native code validates package isolation, symlinks, bridge envelopes, and resource sizes. Credentials never belong in authored code.

The page shell synthesizes the page. The resource scheme exposes only the descriptor, initial values, immutable assets, and the bundled page shell. Databases and discovery files are not served. Decoded resource paths reject empty, dot and parent segments before normalization; the allowlist applies to the resolved path. Descriptor-relative no-follow reads reject nonregular files and enforce 25 MiB per resource. Immutable packages are limited to 256 entries and 50 MiB. Symlinks are rejected during opening and resource reads.

CSP permits local scripts/WASM, local and HTTPS connections/media, HTTPS frames, inline styles, and local/data/HTTPS/blob images. CORS remains enforced. Remote scripts and JavaScript eval remain blocked; fonts stay local/data. Native navigation cancels external navigation of the main frame; explicit HTTP(S) links in the app itself open in the system browser, while HTTPS sub-frames may load and navigate on their own and a click inside one never opens the browser. Camera/microphone grants are not part of this release.

Embedded frames are third-party web content inside the document's window. They cannot reach the bridge (main frame only), the file picker or downloads, and the web data store is non-persistent, so no cookies or logins reach them. Frames still expose the user to whatever page an author embeds, including hidden or phishing-styled frames. Revisit this policy (an allowlist, or frame origins the author declares and the host shows on open) before sharing or a hosted catalog ships.

A slop's page, `slop://app`, has no HTTP(S) referrer, and some providers refuse to embed without one (YouTube reports error 153; a `127.0.0.1` origin gets error 150). The SDK's `@hitslop/document/embed` embeds YouTube through a static relay page, `https://hitslop.com/embed/youtube.html`, served from `apps/landing/public/embed/`. It takes the video from the URL fragment, so the server never sees it, and relays only the player's allowlisted `postMessage` commands. YouTube playback therefore needs that page to be reachable; each additional referrer-gated provider needs its own relay and helper.

Bridge calls must originate in the main frame at `slop://app` and match generated TypeBox envelopes. Serialized requests are bounded to 48 MiB, with storage bounds checked before blob materialization. SQLite uses NOFOLLOW and `trusted_schema=OFF`. Socket request/reply bounds are 1 MiB/16 MiB, one request per connection, with bounded concurrency/timeouts. The server command deadline is 30 seconds; the client waits 35 seconds.

Export destinations must be outside the source package. Capture stages output and publishes by atomic rename before its deadline. Failures do not replace existing output. A lost acknowledgement leaves an uncertain outcome; inspect the destination before retrying.

Slop-initiated blob/data downloads require a native save confirmation. Download bytes receive quarantine metadata before atomic installation, including replacement of existing files; a quarantine write failure leaves the destination unchanged.

Bridge resize requests respect the manifest’s resizable setting, including fixed-size skin windows. The validated `shape` becomes one immutable native silhouette that supplies layer clipping, hit testing and fallback PNG masking; authors' rules for shapes are in [manifest and windows](../../apps/landing/src/content/docs/docs/guides/manifest-and-windows.mdx).

## Capture and Finder integration

`defineSlop` mounts an internal editor boundary and optional lazy export/icon components against the existing document. The export component receives `mode: "preview" | "export"`. Framework-neutral targets must be direct body children. Native code consumes controller geometry and its `dedicated` flag rather than a separate DOM-marker protocol.

The host's window-filling sizing rules have zero specificity and are disabled during capture; the page reset remains. The Svelte icon target owns a transparent 512×512 canvas and centers authored artwork within it. Capture restoration uses the current window container if the user resized during capture.

Export and icon components have separate rendering-error boundaries. A component rendering failure rejects only that capture; restoration preserves the editor and clears the component failure for a later attempt. Editor rendering failures continue to report through the native application-error recovery path and prevent capture, including when authored code throws a falsy value.

Capture commits drafts, flushes persistence, waits for fonts, visible images, and stable layout, and blocks edits. Success and failure restore focus, selection, scroll, styles, and input rendering. Dedicated exports do not inherit native masks. Fallback capture can hide marked editing controls and replace native text inputs with wrapping text.

PDF recomposes WebKit's internal pages into one continuous page when needed. Output sizes are in [icons and exports](../../apps/landing/src/content/docs/docs/guides/icons-and-exports.mdx#export-from-the-host-or-cli).

Native PNG compression runs system zlib off the main actor. It preserves pixels, dimensions, transparency, and metadata, removes alpha only for fully opaque images, and retains the smallest successful candidate or the original. It covers template artwork, refreshed previews, Finder icon sources, and PNG exports; it does not rewrite existing packages or PDFs.

Build/register capture disposable copies, leaving masters state-free and their artwork unchanged. New documents derive Finder custom icons from `QuickLook/Icon.png`. Close refreshes `QuickLook/Preview.png` and, when an icon target exists, `QuickLook/Icon.png` using saved-state snapshots. The host atomically saves the icon PNG, then installs the same image as the Finder custom icon through `NSWorkspace.setIcon`; macOS manages its representation in the resource fork of the hidden `Icon\r` file. The catalog reads the refreshed PNG and updates without reopening. Failed captures or PNG writes retain the previous artwork; a Finder installation failure leaves the successful PNG update available to the catalog. Neither catalog nor Finder display requires loading the engine.

Live exports use the current editor width and selected view. Closed exports render a saved-state snapshot with the app's initial view.

## Telemetry

Release enables Firebase Analytics and Crashlytics after configuration; Debug/tests do not initialize Firebase. Collection defaults off in the app plist. Auth/App Check remain deferred. Existing Analytics events record launch, creation source, opening, duplication, and export format. Cancelled operations produce only fixed breadcrumbs.

Non-fatals carry a fixed operation, classification (platform/authored/rejection), and reason. Native storage and WebKit callbacks supply diagnostic categories before error text is flattened. Fixed category-specific domains and stable numeric codes distinguish issue groups; per-event keys can include export format. No document IDs, paths, titles, contents, guest codes, authored error strings, or raw error userInfo enter reports. Document context never uses global Crashlytics keys.

Save and renderer incidents report once until recovery; propagated close/quit/export errors add breadcrumbs. Authored errors, expected rejections, and background catalog/artwork failures share a two-report limit per app launch, once per category, and stop after the first foreground platform failure. Optional artwork and stale Recents remain normal fallbacks. Standalone CLI processes have no Firebase sink.

Release validation requires actual Firebase delivery and symbolication; unit tests cannot establish those. Use a disposable validation build and retain its dSYM and dashboard evidence, without shipping a crash trigger.

## Schema identity

A stored document opens only under an identical schema key: the canonical JSON of its
descriptor, with keys sorted recursively, array order kept, no whitespace and
JavaScript JSON number formatting. These rules are frozen by a golden vector in
`packages/document/tests/schema.test.ts`. Schema evolution is deferred.
