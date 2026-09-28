# Runtime reference

## Architecture and client boundaries

A `.slop` combines immutable authored app code with a structured local document. Each WebView owns one live Loro replica. The host supplies the document SDK and pinned JS/WASM engine for its selected runtime contract. Svelte observes immutable snapshots and writes typed operations. Loro events patch the snapshot incrementally: only touched containers are re-read and unchanged rows keep their identity. Imported peer bytes are checked for decoding and dependencies; merged semantic anomalies are accepted, preserved and reported as `issues`, never repaired (see [versioning](../versioning.md#reads-and-writes)). Commits carry an origin (`ui` or `cli`) and optional history message; subscribers receive `{kind: "change", origin: "local" | "host" | "remote"}` or `{kind: "status"}`. Every accepted local commit is emitted once through `onLocalUpdate` for persistence and future synchronization. Lazily created map children (optional composites, record entries) use Loro mergeable containers so concurrent creation merges; independently created rows keep distinct containers. Row and tree `$id`s are application registers, not container IDs. Swift persists opaque bytes; it does not interpret application fields.

```text
App → typed operations → host-supplied Loro runtime
                              ↓ update bytes
                        native storage queue → SQLite
CLI → owner's Unix socket ────┘
CLI → exclusive lock → engine-only WebKit → Swift SQLite (closed document)
```

The runtime owns the page and the whole lifecycle: it opens the document, then
imports the package's `assets/app.js` and calls `default.mount(ctx, target)`
([abi.ts](../../packages/document/src/abi.ts)). `ctx` is the only thing an app may rely
on at run time: the document (snapshot, handles, `change`, `flush`, `subscribe`,
`issues`, `full`), `bind.text`/`bind.value`, capture hooks, attachments, theme,
`window.resize` and `reportError`. The returned view supplies `rendered()` (wait for
pending UI updates) and `unmount()`. Reload replaces only the view; flush, close,
native readiness, themes, attachments and capture coordination stay in the runtime.

`@hitslop/document/svelte` is the Svelte adapter compiled into each app:
`defineSlop(App)` is the package entry (the CLI generates it from `App.svelte` and
`styles.css`), and `useDocument`, `<Slop>`, `bindText`, `bindValue`, `capture` and
`attachments` forward to `ctx`. Svelte is an optional peer of the SDK. Other
frameworks supply `main.ts` exporting `default { mount(ctx, target) }` (type
`SlopApp` from `@hitslop/document/abi`), mark their root `data-hitslop-root`, and
use `ctx.capture` for custom export views.

An edit returns after in-memory acceptance. `flush()` and successful CLI mutations acknowledge local persistence. Renderer death can lose unsaved memory. There is no network acknowledgement or second document engine.

The macOS client retains Core, HitSlopWasm, Runtime, Host frameless windows and hover toolbar, TCA Features, local Catalog, Firebase Analytics/Crashlytics, Sparkle, and NativeCLI. HitSlopWasm integrates the shared engine; HitSlopRuntime connects it to the client. The installed helper needs neither Node/Bun nor a running app. Closed document operations never load authored app code.

The catalog combines immutable bundled starters, `~/.hitslop/templates`, and Recents. Users unpack external downloads before placing valid `<slug>.slop` masters in that folder. Create makes a separate writable document. Bundled and installed templates have source-specific identities; categories come from local manifests. Account UI, OpenAPI/Registry, hosted discovery, and sharing are deferred.

## Package layout

TypeBox defines the manifest and platform envelopes. Only `runtime: "hitslop-v1"` is accepted. The manifest requires author, slug, title, description, categories, and presentation.

```text
Example.slop/
  manifest.json
  assets/                       immutable compiled code, CSS, and resources
    runtime.json                contract, minimum revision, and provenance
    app.js                      app module: export default { mount(ctx, target) }
    app.css                     compiled app styling
    theme.json                  declared token defaults
  state.schema.json             {format:1, root:...} descriptor, not JSON Schema
  initial.json                  immutable creation-only values
  .agents/skills/hitslop-document/SKILL.md
  QuickLook/
    Preview.png                 refreshed in writable documents
    Icon.png                    optional immutable authored icon
  state/                        writable documents only
    document.sqlite             opaque checkpoint/update bytes and doc_id, format 1
    writer.lock                 permanent ownership inode
    host.lock                   live socket discovery
    theme.json                  optional token overrides
    attachments/<sha256>        optional immutable imported blobs
```

Templates contain no `state`, `stores`, source, dependencies, caches, or editable structural stylesheets. Builds add document guidance; missing or changed guidance does not prevent opening an existing document. A built or registered master is immutable and must be copied before editing. Initial values seed only a new database. Schema changes require new documents; pre-v1 packages are not migrated.

Host resources are served at stable `slop://app/__runtime__/` URLs, backed by the selected `runtimes/<contract>` directory. App bundles must not embed Loro or the document implementation. Preview uses the same runtime with disposable memory storage. No executable code is downloaded.

`assets/runtime.json` declares positive integer `runtimeContract` and `minRuntimeRevision`; SDK/Loro/protocol fields record provenance. Opening checks supported contract and minimum revision before storage opens. Authored source and CLI identities must match for compilation. Capture requires a helper supporting the requested contract/revision. See [versioning](../versioning.md) for the full compatibility contract.

## Persistence and ownership

Swift owns `state/document.sqlite`: `user_version=1`, DELETE journaling, synchronous FULL, and opaque checkpoint/update bytes. Checkpoint replacement and covered-row deletion are atomic. The exact canonical descriptor is the storage key. Generation is a storage token, not a Loro version.

One OS flock on permanent `state/writer.lock` owns each local package. Never unlink it or bypass a busy writer. `state/host.lock` is discovery only. A busy writer with unreachable discovery is an error, never permission for another writer.

Autosave runs after 200 ms. Flush commits text drafts and scalar previews, persists updates, and checkpoints at 256 updates or 4 MiB. Native load/write limits are 4,096 rows and 32 MiB aggregate checkpoint/update bytes. The engine checkpoints before appending beyond those limits. Checkpoints retain history; automatic history pruning is deferred. Before persisting a batch, the runtime checks its actual full snapshot against capacity. Oversized live edits remain visible and unsaved (`save-failed`, `full`), and close/export fail. Editing and retry remain available; successful saving or explicit discard clears the failure. Discard awaits reloading durable state under the same writer lock and remounts the view; failed restoration retains unsaved edits. Full history is kept; see [runtime reset](../runtime-reset.md#3-data-semantics-ready-for-collaboration). `bun scripts/v1/growth.ts` measures realistic growth (5,000 tasks of churn use about 2% of capacity). Writers never produce shallow checkpoints; valid existing shallow checkpoints remain readable. Oversized existing packages are refused intact; compact cannot promise recovery of unloadable packages.

After an append commits but acknowledgement is lost, the engine reloads metadata and retains the same Loro bytes. A later flush imports them idempotently. Repeating the user's operation is not idempotent. CLI mutations serialize and acknowledge persistence; no receipts, public retry identity, or automatic replay exist. After an uncertain result, use `get` before another edit. `get` flushes drafts and pending writes. `hello` supplies the WebView session epoch for mutation/export handshakes.

Prepare-close commits drafts and freezes edits. Failed saves retain ownership and native retry UI; cancel-close restores editing. Successful close removes discovery, destroys the WebView/bridge, drains storage, closes SQLite, and releases ownership. Quit prepares every document before releasing any. Close before moving or renaming packages. iCloud and other synced folders are unsupported.

Opaque imported attachments live at `state/attachments/<sha256>`, outside Loro. Imports store and fsync bytes before a synchronous typed document transaction commits the reference. Pending imports participate in flush/close and retain failed writes for native retry. The owner enforces 10 MiB per file, 100 MiB total, and 256 files, rejects links, and verifies hashes on read. Duplication copies attachments; runtime templates contain none. Unreferenced blobs remain until a future explicit garbage-collection policy.

Theme defaults are declared in immutable `assets/theme.json`. The runtime applies defaults and document overrides as CSS variables before mounting the app; new builds need no separate theme stylesheet. Existing packages may retain their generated `assets/theme.css` and remain supported.

Theme overrides are bounded host presentation state, not a document projection. The 64 KiB `state/theme.json` map contains declared token overrides. Use `slop theme get/set/reset`; never edit this file or compiled assets directly. Layout changes require authoring source and a rebuild. Theme commands share ownership with document operations. JavaScript validates declared names and CSS values; native code validates envelopes, isolation, and size. Arbitrary CSS override files are unsupported.

## Opening and recovery

Document windows remain hidden until runtime readiness and bounded font readiness, then reveal directly and let WebKit paint normally. There is no extra animation-frame timeout or transparent-window staging. Opens taking more than one second from the original request display cancellable native progress. Package validation, runtime selection, writer ownership and SQLite setup run on the preparation queue; cancellation disposes acquired storage before returning. Window skins reuse their validated image for that open. The hover toolbar and editor discovery are created only on first hover. Catalog launches prewarm WebKit by compiling the bundled runtime once in a throwaway WebView that serves no package files and is never handed to a document; a document open releases it. Closing cancels presentation. Startup failures reveal native error UI. Quick Look artwork serves Finder and catalog display, not an opening placeholder.

Debug `HITSLOP_STARTUP_TIMINGS=1` logs elapsed native preparation, runtime readiness, font readiness, presentation, and page-relative `hitslop:*` boot marks without document values or paths. `HITSLOP_STARTUP_BENCH=1 bun run swift:test --filter documentStartupTimings` retains the fresh-document benchmark. Use `HITSLOP_STARTUP_BENCH=1 bun run swift:test -c release --filter savedDocumentStartupTimings` for existing documents: it seeds saved edits in a separate helper process, then reopens Quick Checklist, Small Expenses, a 1,000-row checklist and a skinned fixture ten times each. Output records preparation/readiness/reveal durations and whether progress appeared. Set `HITSLOP_STARTUP_PREWARM=1` to prewarm for two seconds before measurement, `HITSLOP_STARTUP_CASE=quick-checklist` (or another case) to measure that case first in a fresh process, `HITSLOP_STARTUP_SAMPLES` for the sample count, and `HITSLOP_STARTUP_FOREGROUND=1` to activate the benchmark app. Compare the first sample separately from subsequent samples; report warmed median and p95 rather than enforcing machine-dependent CI thresholds. Caches and machine activity affect these diagnostics.

Application-render errors and save failures have separate recovery paths. Renderer recovery retains the writer lease while replacing the WebView and reopening saved bytes. Unsaved renderer memory cannot be recovered. Host and CLI exports flush before capture; expired captures cannot publish output.

## Security boundaries

Authored code can change or damage its own document. Runtime operation validation is not a separate security boundary from code sharing that page. Native code validates package isolation, symlinks, bridge envelopes, and resource sizes. Credentials never belong in authored code.

The runtime synthesizes the page. The resource scheme exposes only the descriptor, initial values, immutable assets, and host runtime resources. Databases and discovery files are not served. Decoded resource paths reject empty, dot and parent segments before normalization; the allowlist applies to the resolved path. Descriptor-relative no-follow reads reject nonregular files and enforce 25 MiB per resource. Immutable packages are limited to 256 entries and 50 MiB. Symlinks are rejected during opening and resource reads.

CSP permits local scripts/WASM, local and HTTPS connections/media, inline styles, and local/data/HTTPS/blob images. CORS remains enforced. Remote scripts and JavaScript eval remain blocked; fonts stay local/data. Native navigation cancels external navigation; explicit HTTP(S) links open in the system browser. Camera/microphone grants are not part of v1.

Bridge calls must originate in the main frame at `slop://app` and match generated TypeBox envelopes. Serialized requests are bounded to 48 MiB, with storage bounds checked before blob materialization. SQLite uses NOFOLLOW and `trusted_schema=OFF`. Socket request/reply bounds are 1 MiB/16 MiB, one request per connection, with bounded concurrency/timeouts. The server command deadline is 30 seconds; the client waits 35 seconds.

Export destinations must be outside the source package. Capture stages output and publishes by atomic rename before its deadline. Failures do not replace existing output. A lost acknowledgement leaves an uncertain outcome; inspect the destination before retrying.

Slop-initiated blob/data downloads require a native save confirmation. Download bytes receive quarantine metadata before atomic installation, including replacement of existing files; a quarantine write failure leaves the destination unchanged.

Bridge resize requests respect the manifest’s resizable setting, including fixed-size skin windows.

## Capture and Finder integration

Svelte's `<Slop>` boundary registers optional lazy export/icon snippets against the existing document. Framework-neutral targets must be direct body children. Native code consumes controller geometry and its `dedicated` flag rather than a separate DOM-marker protocol.

The host supplies page margin/padding resets and window-filling page, root, and mount-ancestor sizing in every presentation mode. Sizing rules have zero specificity and are disabled during capture; the page reset remains. Transparent and skinned modes override ordinary page backgrounds, while standard mode preserves authored backgrounds. The Svelte icon target owns a transparent 512×512 canvas and centers authored artwork within it.

Export and icon snippets have separate rendering-error boundaries. A snippet rendering failure rejects only that capture; restoration preserves the editor and clears the snippet failure for a later attempt. Editor rendering failures continue to report through the native application-error recovery path and prevent capture, including when authored code throws a falsy value.

Capture commits drafts, flushes persistence, waits for fonts, visible images, and stable layout, and blocks edits. Success and failure restore focus, selection, scroll, styles, and input rendering. Dedicated exports do not inherit native masks. Fallback capture can hide marked editing controls and replace native text inputs with wrapping text.

PNG uses 2× rendering, limited to 16,384 pixels per side and 24 megapixels. PDF retains text/vectors on a continuous page, recomposing WebKit internal pages when needed and scaling to a maximum 14,400-point dimension. Icons use a transparent 512px square.

Native PNG compression runs system zlib off the main actor. It preserves pixels, dimensions, transparency, and metadata, removes alpha only for fully opaque images, and retains the smallest successful candidate or the original. It covers template artwork, refreshed previews, Finder icon sources, and PNG exports; it does not rewrite existing packages or PDFs.

Build/register capture disposable copies, leaving masters state-free. New documents derive Finder custom icons from immutable `QuickLook/Icon.png`. Close refreshes `QuickLook/Preview.png` and optional Finder icon metadata using saved-state snapshots, without rewriting the immutable icon. Catalog/Finder display PNGs without loading the engine.

Live exports use the current editor width and selected view. Closed exports render a saved-state snapshot with the app's initial view. Quick Checklist's tab selection is not persisted, so a closed export starts in To do.

## Telemetry

Release enables Firebase Analytics and Crashlytics after configuration; Debug/tests do not initialize Firebase. Collection defaults off in the app plist. Auth/App Check remain deferred. Existing Analytics events record launch, creation source, opening, duplication, and export format. Cancelled operations produce only fixed breadcrumbs.

Non-fatals carry a fixed operation, classification (platform/authored/rejection), and reason. Native storage and WebKit callbacks supply diagnostic categories before error text is flattened. Fixed category-specific domains and stable numeric codes distinguish issue groups; per-event keys can include export format and the resolved bundled runtime contract/revision. No document IDs, paths, titles, contents, guest codes, authored error strings, or raw error userInfo enter reports. Document context never uses global Crashlytics keys.

Save and renderer incidents report once until recovery; propagated close/quit/export errors add breadcrumbs. Authored errors, expected rejections, and background catalog/artwork failures share a two-report limit per app launch, once per category, and stop after the first foreground platform failure. Optional artwork and stale Recents remain normal fallbacks. Standalone CLI processes have no Firebase sink.

Release validation requires actual Firebase delivery and symbolication; unit tests cannot establish those. Use a disposable validation build and retain its dSYM and dashboard evidence, without shipping a crash trigger.

## Schema identity

A stored document opens only under an identical schema key: the canonical JSON of its
descriptor, with keys sorted recursively, array order kept, no whitespace and
JavaScript JSON number formatting. These rules are frozen by a golden vector in
`packages/document/tests/open.test.ts`. Schema evolution is deferred.
