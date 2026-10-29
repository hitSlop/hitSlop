# Runtime reference (page shell, files, storage)

How an edit, a save and a close flow through the system is described in
[architecture](../architecture.md). This page is the reference for `.tiny` files, storage
limits, security, capture and telemetry.

## Page shell and ctx

The page shell owns the page and the whole lifecycle: it opens the document, then
imports the app's `assets/ui.js` and calls `default.mount(ctx, target)`
([abi.ts](../../packages/tinys/src/sdk/abi.ts)). The private page shell supplies the runtime. The single `tinys` npm package ships
the CLI, author SDK, contracts, templates, skills and locally served runtime. `ctx` is the only thing an app may rely
on at run time: the document (snapshot, handles, `change`, `flush`, `subscribe`),
`bind.text`, attachments, `speech.transcribe`, `window.resize`, optional `environment` and
`reportError`; apps read the theme only as `--tiny-*` CSS variables. The returned view may supply `rendered()` (wait for
pending UI updates) and `unmount()`. Reload replaces only the view; flush, close,
native readiness, themes and attachments stay in the page shell.

`subscribe` supports multiple listeners and idempotent disposal. `observe` is the single
framework dependency hook: replacing it installs the new adapter, and disposing the old
adapter cannot detach the replacement. Text composition, caret transforms and merges
stay in the host binding. Draft submission, motion and notice presentation are SDK
helpers compiled into the app, without separate host APIs.

`ctx.environment` supplies an immutable `current` snapshot (`platform`, `device`,
`layout`) and `subscribe(listener)` with idempotent disposal. `getEnvironment()` adapts
it to each Svelte editor mount. Native identity comes from the typed `host.environment`
host action, not the Rust owner's process or a user agent. Browser identity is unknown;
the shell computes layout from the document viewport using Rust's 640 CSS-pixel
breakpoint. The optional member has a real SDK fallback on older hosts: unknown identity
and live viewport layout. It adds no runtime ABI requirement or document fields.
New shells install `data-tiny-platform`, `data-tiny-device` and `data-tiny-layout` on
`<html>` before editor mount. They are absent from static captures and static dev views.
Older shells lack these attributes, so authored CSS keeps a usable base layout.

The complete ABI inventory lives in `sdk/abi.ts`: `data-tiny-root`, capture and
controls attributes, the notice region, `--tiny-*` variables, `__tinyCommands`,
`__tinyDescribe` and `__tinyRun`. Command sets use `Symbol.for("tiny.command-set")` with `{ document, specs }`. A
`tiny.operation-error` branded `DocumentError` carries document outcomes across bundles;
`refuse(message)` uses code `rejected` and reason `refused`. A `tiny.speech-error`
branded `SpeechError` carries `ctx.speech` failures (code `unavailable`, `cancelled` or
`failed`; `reason` for `unavailable`). Unhandled refusals dispatch
the cancelable `tiny:refused` event with the message as its detail. An app that cancels
it has displayed the message; other faults go to the host's issue path. The same file
lists the host-internal names apps never read (`tiny:render-error`, `data-tiny-host`,
`__tiny`, `__tinyPreview` and the preview transport's `tiny:*` messages).

`@hitslop/tinys/svelte` is the Svelte adapter compiled into each app:
the builder emits `svelteApp` from required root `schema.ts` and `App.svelte`, plus
required `Preview.svelte` and `Icon.svelte`, optional `Export.svelte` and `Widget.svelte`. These filenames are fixed and case-sensitive.
Metadata in `tiny.ts` and agent-only `commands.ts` stay outside the UI graph. Authors
import styles from their components. During component initialization, `getDocument(schema)` acquires the mount's live document
(`ctx.document` with Svelte reactivity; a handle's `value` registers its read through
`ctx.document.observe`). `bindText`, `EditableText`, `attachments` and `speech` forward to `ctx`. The generated entry is the one authoring path. Authoring requires `Preview.svelte` and `Icon.svelte`; export and widget components remain optional.

An edit promise resolves after native acceptance and the corresponding local snapshot update, before durability or framework rendering. `flush()` and successful CLI mutations acknowledge local persistence. Native host-to-page calls must finish within the page-call deadline; a page that does not answer counts as a dead renderer ([recovery](../architecture.md#layers)). Renderer death retains accepted native edits; text not yet sent from a field can be lost. There is no network acknowledgement or second document engine.

The Mac catalog combines immutable bundled starters, `~/.tinys/templates`, and Recents. Users place `<slug>.tiny` templates in that folder. Create makes a separate writable document. Bundled and installed templates have source-specific identities; categories come from each template's app metadata. See the [roadmap](../roadmap.md#later) for remaining catalog and sharing work.

## File layout

A `.tiny` file is one SQLite database (application ID `0x54494E59`, "TINY"; storage version in
`user_version`). Rust defines internal wires and package-format acceptance. `tiny.ts`
declares identity, window, ordered theme, capabilities and optional artwork. `schema.ts`
supplies the descriptor and initial values; fixed root component and command files
supply the views and agent commands. The build writes `packageFormat` and `runtimeABI`; readers check
these markers before interpreting current-format fields. Swift receives typed UniFFI
metadata/window/theme values and never decodes the stored definition itself.

```text
app          one row: package_format, runtime_abi, catalog columns, definition_json
assets       key, media_type, encoding, size, bytes: ui.js / preview.js / icon.js, optional ui.css / commands.js / export.js / widget.js, media…
artwork      preview and icon PNGs: built, then rewritten as an edited document's window closes
document     a document's identity; none in a template
history      the saved Loro state in order: a snapshot (data and theme overrides; a template's
             holds the initial values), then the updates saved after it
share        the room a shared document syncs through and its relay endpoint; none otherwise
attachments  sha256, sniffed media_type, bytes: imported files
```

A template has no `document`, `share` or `attachments` rows and one `history` row; it never
opens as a document, and a command refuses it. Create copies a template into a new file
and atomically adds its identity. A template is immutable (`is_template` refuses it as a
document), and bundled starters are also read-only on disk. Initial values seed only
a new document. Schema changes require new documents.

The page shell is served at `tiny://app/__shell__/`, from the one shell bundled with the app. The document's assets are served at `tiny://app/assets/`, whole or as byte ranges; the resource reader refuses private commands.js, widget.js, icon.js and export.js programs. Attachments use /attachments/<id> with nosniff, sandbox and first-touch hash checks, and the page receives the descriptor with its config. App bundles must not embed Loro or the document implementation. Preview serves the same shell over a session-scoped bridge to a native Rust owner of a temporary document. The local Chrome browser host uses the same Rust owner compiled to WASM, with SQLite in OPFS. Its app frame has a separate origin per copy and receives assets through a client-bound Service Worker channel. Runtime code is bundled with the CLI and served locally.

Both readers check the application/storage markers, then package-format and runtime-ABI requirements, then the exact SQLite layout. `summary` reads only safe catalog fields and artwork, never the definition, app assets, attachments or Loro payloads. It is not a validity certificate. Full acceptance checks the definition with its frozen package-format module, resource inventory and budgets, streaming PNGs and optionally SQLite integrity; owners also validate Loro state. A newer marker returns `requires_update` without writes. Reads never migrate. SQLite may roll back a hot journal from an interrupted transaction to restore committed bytes.

Assets are inserted first and the single app row last. The app row seals assets: unconditional update/delete guards and an insert guard reject replacements, even by rowid. Attachments and artwork keep separate lifecycles. Marker ownership and released-file guarantees are defined in the [compatibility contract](../engineering-contract.md#compatibility).


## Persistence and ownership

The Rust store (`tinys-core`'s `store`, on the platform SQLite) owns a document's saved state: the `document`, `history` and `attachments` tables, DELETE journaling, synchronous EXTRA and macOS fullfsync, a 2-second busy timeout, full auto-vacuum, and opaque Loro bytes. Replacing the history with a checkpoint and freeing the pages it covered are atomic. Nothing outside the core opens the file, so one SQLite library holds its locks. Every connection is defensive: no symbolic links, no trusted schema, cell-size checks, no memory mapping, and values no longer than the largest stored one. Readers beside the writer open read-write with `query_only`.

One OS flock, taken by the store, owns each document: on a lock file in the account's registry (`~/.tinys/live/<device>-<inode>.lock`), never on the database. Lock files are never removed; never bypass a busy writer. The holder's discovery file beside it names its socket; an owner removes a crashed session's as soon as it takes the lock. A busy writer with unreachable discovery is an error, never permission for another writer. A document with a second hard link is refused, because SQLite names its journal after the path. A rename or replacement while open stops saving (`Moved`); moved back, the store reconnects and saves.

Checkpoint maintenance runs at 256 saved updates or 4 MiB. Native limits are 4,096 update rows and 32 MiB aggregate checkpoint/update bytes. The [saving flow](../architecture.md#saving) defines scheduling, close-time history trimming and failed-save recovery; [sync](../architecture.md#sync) defines the shared-document exceptions. `tiny import` writes a JSON value as one `replace` operation ([CLI](../guides/cli.md#operations)).

Generic mutations are not replayed; a named command may reevaluate once after a definite pre-admission conflict, and after an uncertain result you run `get` before another edit. A live `get` returns owner-accepted state; text still in an open window's field is not included. Every request names its command protocol, which the owner checks before reading anything else, so an engine of another build is refused with which side to update. A reply carries its state whole, as large as the document; the CLI validates each reply's method and required result fields (`SocketReply`). A live and a closed `get` of one saved state print the same value.

The [close and capture flow](../architecture.md#close-export-and-capture) defines ownership release, retry and quit ordering. Close before moving or renaming documents. iCloud and other synced folders are unsupported; the Mac offers to move an iCloud document to `~/tinys`, and the [Apple library](../architecture.md#synced-library) carries a person's own devices.

Opaque imported attachments live in the file's `attachments` table by SHA-256, outside Loro. `attachments.import(file, (tx, ref) => …)` stores the bytes, then submits the collector's reference edits; the close and capture barriers wait for both. An agent's `batch` carries its files (`tiny apply --attach`), stored before its operations in the same request, so no close falls between a blob and its reference. A collector that throws, or an edit the core refuses, leaves the stored blob unreferenced. After the final save of an unshared document, the owner deletes blobs whose ID appears in no string, text or map key of the saved state (`Store::reclaim_attachments`). Shared and Synced Library documents retain every attachment, including unreferenced ones, because another replica can still reference it. Closing or deleting a reference does not free their attachment budget; an unshared clean copy retains only referenced files. The store enforces the [attachment limits](../../apps/landing/src/content/docs/docs/guides/files-and-web.mdx#attachments) (10 MiB per file, 100 MiB and 256 files per document) and verifies hashes on read. Duplicate and Share Tiny File carry only the attachments their state references, with no history and artwork rendered for the copy (`Store::copy_clean`); templates contain none.

Theme overrides are limited to 256 declared colors, checked on write and on open. Their storage, publication and undo behavior are described in [themes and attachments](../architecture.md#themes-and-attachments). Fonts, arbitrary CSS overrides and layout changes need authoring source and a rebuild.

## Opening and recovery

Document windows remain hidden until the page reports ready, which it does once the app has mounted and every web font it declares has loaded (each declared face is loaded directly from the app's assets, without forcing layout; bounded to 12 seconds, after which opening fails; a page with no `@font-face` skips the wait), then reveal directly and let WebKit paint normally. There is no extra animation-frame timeout or transparent-window staging. Opens taking more than one second from the original request display cancellable native progress. Checking the file, writer ownership and SQLite setup run on the preparation queue, and `tiny://` assets are read off the main thread; cancellation disposes acquired storage before returning. Window skins reuse their validated image for that open. The hover toolbar and editor discovery are created only on first hover. Closing cancels presentation. Startup failures reveal native error UI. A document's artwork serves Quick Look, Finder and catalog display, not an opening placeholder.

Application-render errors and save failures have separate recovery paths. Renderer recovery retains the writer lease and the socket, and attaches a new WebView to the same owner, including its unsaved edits. Text not yet sent from the dead page cannot be recovered. Host and CLI exports flush before capture; expired captures cannot publish output.

## Security boundaries

For author examples and platform availability, see [the public capability guide](../../apps/landing/src/content/docs/docs/guides/capabilities.mdx).
The declaration and host enforcement described below do not create a per-tiny OS grant.

Authored code can change or damage its own document. Runtime operation validation is not a separate security boundary from code sharing that page. Native code validates the file, bridge envelopes, and resource sizes. Credentials never belong in authored code.

The page shell synthesizes the page. The resource scheme exposes the app's page assets (`/assets/`, never its private `commands.js`, `widget.js`, `icon.js`, `preview.js` or `export.js`), the document's attachments (`/attachments/<id>`, served with `nosniff` and CSP `sandbox`) and the bundled page shell; saved state, artwork and the app's other columns are not resources. Decoded resource paths reject empty, dot and parent segments before anything else. An asset is read from the file through its own connection, opened read-write with `query_only` as every reader beside the writer is, whole or as a range. Packing stores text (HTML, JavaScript, CSS, JSON, SVG) and WebAssembly Brotli-compressed when that is smaller, and is decoded whole to serve; everything else is stored as it is and read by range without loading the rest. Limits count decoded bytes: each asset is at most 25 MiB, and an app at most 256 assets and 50 MiB. Responses carry `Content-Length` and answer single byte ranges with 206, which WebKit's media loader requires for audio and video assets.

The native, browser and development-preview CSPs are defined in Rust
(`crates/tinys-core/src/wire/limits.rs`, `wire/browser.rs` and `bindings.rs`) and re-exported by `packages/tinys/src/schema/policy.ts`; only local origins and the development HMR connection differ. CSP permits local scripts and WebAssembly compiled at runtime (`'wasm-unsafe-eval'`), local and HTTPS connections/media, HTTPS frames, inline styles, and local/data/HTTPS/blob images. CORS remains enforced. Remote scripts and JavaScript eval remain blocked; fonts stay local/data. Native navigation cancels external navigation of the main frame; explicit HTTP(S) links in the app itself open in the system browser, while HTTPS sub-frames may load and navigate on their own and a click inside one never opens the browser.

| Capability | Boundary | Availability |
| --- | --- | --- |
| `camera`, `microphone` | Web APIs, admitted by the native WebKit delegate or browser permission policy. | Interactive documents with the declared devices and OS/browser permission. |
| `speech` | Typed `speech.transcribe` host request carrying an attachment ID and optional locale, never audio bytes. | Owned native document page with on-device recognition support and speech permission. |

Omitting `capabilities` declares none. Changing declarations requires a rebuilt template
and a new document, never a data edit. Playback, file selection, attachments and allowed
HTTPS requests do not acquire a capability requirement. A declaration restricts admission;
it is not consent. Code-bound permissions remain a [proposal](../ideas.md#permissions-bound-to-the-apps-code).

The iPhone app denies camera and microphone capture even when declared; its Files picker and HTTPS embed navigation use the shared native policies.

Camera and microphone use the web APIs (`getUserMedia`, `MediaRecorder`), never a `ctx` wrapper. An app lists what it uses in `capabilities` (`camera`, `microphone`, `speech`). Rust stores them sorted and unique in the definition and exposes only the ones this build implements; authoring refuses any other value, while opening ignores a saved value it does not know, so a later build can add one without a new package format and an older host simply never grants it. The Mac host's WebKit media delegate grants a request only for the current, ready, owned document page, from the main frame at `tiny://app`, in a visible window, when the declaration covers every requested device; a replaced or closed page's capture is stopped. macOS still asks the person once for tinys (`NSCameraUsageDescription`, `NSMicrophoneUsageDescription` and the hardened-runtime device entitlements); there is no per-tiny prompt yet. WebKit exposes `navigator.mediaDevices` at `tiny://app` only because the app's Info.plist carries those usage descriptions. The development preview sends `Permissions-Policy` and an iframe `allow` for the declared devices only, and the local browser host delegates `camera`/`microphone` to a copy's frame only when its app declared them.

Speech is a typed host service, not a web API. The page sends `speech.transcribe` with a stored recording's attachment ID and an optional locale identifier (at most 35 ASCII letters, digits, `-` and `_`), never the recording's bytes, so a clip near the 10 MiB attachment limit is not sent through the page request a second time. Rust decodes and bounds the request, normalizes locale separators from `_` to `-`, answers `unavailable` itself when the stored app did not declare `speech` (`notDeclared`), and otherwise delivers a `HostAction` to the host through the same view fence as other page work. The native host serves it only for the ready page of an owned document, reads and verifies the recording through the owner's resource reader, checks the provider's format support, and transcribes it with `SFSpeechRecognizer` with `requiresOnDeviceRecognition` on every supported macOS. The Mac provider accepts m4a, mp3, wav, flac, mp4 and mov; other formats return `unavailable` (`format`). Missing or damaged attachments return `failed`. Format and on-device locale support are checked before asking for permission, so a locale the Mac recognizes only through Apple's servers is `unavailable` (`locale`) without a prompt; `NSSpeechRecognitionUsageDescription` covers the one system prompt, and a declined or restricted permission is `unavailable` (`permission`). Each transcription belongs to the page that asked: close preparation, page replacement, interface reload, renderer loss or teardown cancels it, and the page receives `cancelled` when its transport remains available. Saved-source exports, copies and widget refreshes do not cancel transcription. Rust serializes the outcome, shortening messages to the diagnostic limit and refusing a transcript over 1 MiB. The iPhone app, `tiny dev`, the browser host and the WASM core answer `unavailable` (`host`).

Transcripts are returned without being saved; authors write them through document handles
and guard against the recording changing during recognition. `isSpeechError` recognizes
cross-bundle `unavailable`, `cancelled` and `failed` errors, with unavailability reasons
`notDeclared`, `host`, `system`, `locale`, `permission` and `format`. Ogg/WebM storage does not imply
native transcription support. `SpeechAnalyzer` and browser speech remain deferred.

Embedded frames are third-party web content inside the document's window. They cannot reach the bridge (main frame only), the file picker or downloads, and the web data store is non-persistent, so no cookies or logins reach them. Frames still expose the user to whatever page an author embeds, including hidden or phishing-styled frames. Revisit this policy (an allowlist, or frame origins the author declares and the host shows on open) before public sharing rollout or a hosted catalog ships.

A tiny's page, `tiny://app`, has no HTTP(S) referrer, and some providers refuse to embed without one (YouTube reports error 153; a `127.0.0.1` origin gets error 150). The SDK embeds YouTube through a static relay page, `https://tinys.click/embed/youtube.html`, served from `apps/landing/public/embed/`. `@hitslop/tinys/youtube` builds the relay and thumbnail URLs as plain strings, and a static view may import it; `@hitslop/tinys/embed` adds reading a pasted link, the title lookup and the player, which need `URL`, the network or the page, none of which a capture has, so a static view may not import it. The relay takes the video from the URL fragment, so the server never sees it, and relays only the player's allowlisted `postMessage` commands. YouTube playback therefore needs that page to be reachable; each additional referrer-gated provider needs its own relay and helper.

Bridge calls must originate in the main frame at `tiny://app` and decode into the Rust serde request types. Page requests are bounded to 16 MiB, which holds the largest attachment upload (10 MiB, base64-encoded to about 13.3 MiB), with storage bounds checked before blob materialization. SQLite uses NOFOLLOW and `trusted_schema=OFF`. Socket requests are bounded to 1 MiB (16 MiB for a batch carrying attachments), one request per connection, with bounded concurrency/timeouts. The server command deadline is 30 seconds; the client waits 35 seconds.

An export never replaces the document it renders. Closed and live exports share one publisher: it stages the bytes beside the destination, syncs them, checks the deadline immediately before publishing and publishes by atomic rename, then syncs the folder. A failed write leaves no destination file. Existing output is never replaced. A lost acknowledgement, or publication whose directory sync is unconfirmed, leaves an uncertain outcome; inspect the destination before retrying.

Tiny-initiated blob/data downloads require a native save confirmation. Download bytes receive quarantine metadata before atomic installation, including replacement of existing files; a quarantine write failure leaves the destination unchanged.

Rust resolves window defaults, frame capabilities and silhouettes once for the native host, page shell, development preview and browser host. Viewport fitting and drawing stay in each host. Bridge resize requests respect the window's resizable setting, including fixed-size skin windows. The native silhouette supplies layer clipping and hit testing; authors' rules for shapes are in [manifest and windows](../../apps/landing/src/content/docs/docs/guides/manifest-and-windows.mdx).

## Capture and Finder integration

### Stored capture program ABI (runtimeABI 1)

The private Preview, Icon and Export programs expose `__tinyCapture(json)`;
Widget exposes `__tinyWidget(json)`. Each is synchronous and returns a JSON string
containing `{frames, fonts}`. A view request contains `kind` (`preview`, `icon` or
`export`), `format` (`png` or `pdf`), `width`, `height`, `snapshot`, `theme` and
`attachments`. A widget request contains `snapshot`, `theme`, `attachments` and
`viewports` (each with `family`, `width`, `height`). `attachments` maps referenced
supported image IDs to media types (`image/png`, `image/jpeg`, `image/gif`,
`image/webp`), never bytes. The host request grows only additively.

Frames contain `{html, css, width, height, kind?}`; views return one frame with
their kind, widgets three without a kind. Fonts contain `{family, bytes, weight?,
italic}`, with base64 packaged font bytes. Hosts read the batch, frames and fonts
openly, ignoring unknown members while checking required fields and bounds.
`attachmentURL(id)` returns `tiny-attachment:<id>` for an inventoried image and
throws for anything else. IDs are 64 lowercase hexadecimal SHA-256 characters.
The host scans frame HTML and CSS, verifies and loads only the attachments they
reference, then sends markup and those images to a separate raster child. Authored
JavaScript never receives attachment image bytes; the raster child never executes JavaScript.
A failed output does not discard successful sibling outputs.

Programs run in bare QuickJS, without `window`, `document`, `navigator`,
`requestAnimationFrame`, `fetch`, `XMLHttpRequest`, `Image`, `Worker`, `URL`,
`URLSearchParams`, `TextEncoder`, `TextDecoder`, `AbortController`, `Blob` or
`structuredClone`. The SDK emits `data-tiny-capture` (`static`, `icon`, `widget`),
`data-tiny-capture-target` (the view kind). Static view captures hide
`[data-tiny-export="hide"]`.

Released behavior includes a 512 × 512 icon, widget canvases at most 512 per side
(164 × 164, 344 × 164, 344 × 344), at most 16 MiB of serialized markup including
fonts, 64 font faces (family names at most 256 bytes; weights 1–1000), and 24 MiB
of referenced images per output. Initial canvases are at most 16,384 per side.
PNG is bounded to 16,384 pixels per side and 24 megapixels, with a 36 MiB encoded
reply; widgets have an 8 MiB encoded reply. PDF allows measured content up to
1,000,000 CSS pixels, uniformly scales to 14,400 points per side, and has a 24 MiB
byte budget and 36 MiB encoded reply. Each child has a 64 MiB input and 512 MiB
resident-memory guard; markup and PNG have a 3-second wall/CPU budget, PDF raster
has a 10-second wall and 8-second CPU budget. QuickJS execution is interrupted
after 2 seconds. Captures must remain faithful and usable; renderer fixes may
change pixels. Pixel equality is evidence for a renderer refactor, not the ABI
guarantee.

Every host capture uses synchronous Svelte SSR and Takumi. Mac and Linux use the
restricted Rust child; iPhone uses the same capture pipeline in process, as described
in [capture architecture](../architecture.md#close-export-and-capture). An open document first drains its
page writes within the page-call deadline; a page that does not answer counts as a dead
renderer. It saves and backs up SQLite to a temporary `.tiny`; the editor capture barrier
ends after the backup. Rendering then uses that independent copy, including its app,
attachments, data and theme. The editor's focus, selection, scroll and local selected
view are not used or changed. No read transaction remains open during rendering.
Source acquisition leaves speech, media access, file selection, downloads and resize
eligible under their normal page policy. The native acquisition queue still coordinates
close, discard, reload and reopen. Close batches preview, icon and eligible widgets from
one source; Rust prepares shared inputs once and returns independent output results.

A capture's CSS is exactly what its own Preview/Export/Icon/Widget role imports, in
import order, including component CSS. Captures no longer inherit the editor's
stylesheets; `tiny dev` still mounts static views inside the editor page.

The CLI requires `Preview.svelte` and `Icon.svelte` beside `App.svelte`; `Export.svelte` and `Widget.svelte` are optional.
Preview supplies compact Quick Look/catalog artwork; Export supplies complete PNG/PDF layout.
Without Export, native export/share-image actions are hidden and CLI exports are refused.
State that should determine an export, such as a selected report, must be saved in the
document. Capture components receive a deeply readonly `snapshot` and an `attachmentURL` function as props; they cannot change saved
state to prepare their view.

The builder emits separate synchronous SSR programs for every preview, icon and declared export.
These read their immutable `snapshot` prop in a fresh QuickJS realm and render with Takumi in
the restricted Rust child. Icons are 512×512; export PNGs are 2× with measured natural
height, and previews crop at the larger of the saved window height and three widths.
Native capture preserves the saved viewport's layout while extending the image canvas
to include horizontal overflow; viewport units retain the saved window dimensions.
The build rejects retained browser lifecycle/DOM, locale formatting, async code or
host helpers with a role/source diagnostic. The `attachmentURL(id)` prop resolves referenced
PNG/JPEG/GIF/WebP images from the saved source; capture cannot import or read blobs.
The inventory is capped at 24 MiB before base64 encoding. Missing or unsupported
images fail capture and retain prior artwork. Native failures never change engines.

A dedicated export does not inherit the native window mask. Preview, Export and Icon
captures append a platform stylesheet after the app's CSS and theme: it resets the page
instead of applying the editor's window-filling sizing, hides `data-tiny-export="hide"`
elements, and gives the icon target a transparent 512×512 canvas that centers authored
artwork. The static HTML carries `data-tiny-capture` (`static` or `icon`) and wraps the
view in `data-tiny-capture-target`. A rendering error rejects the capture.

Widget capture requires `Widget.svelte`: it has no App or Export fallback. The component
receives the Rust-generated `WidgetProps` (`family`, `width`, `height`). A private synchronous
Svelte SSR program reads the saved `snapshot` prop in a fresh QuickJS realm. Takumi renders the
three fixed canvases at 2× in the same restricted child, using bundled fonts and resources.
No browser code runs during native widget capture. The browser authoring
preview still mounts the component normally. The host publishes PNGs outside the document.
Widget freshness includes saved state, renderer/font bytes and canvas configuration;
startup refreshes stale images even when the document has not been edited. Failed
captures retain the previous images, and WidgetKit controls visible reload timing.
The [macOS widget architecture](../architecture.md#macos-image-widgets)
describes selection, opening and refresh policy. The extension never runs authored code.

Dedicated PNG and PDF captures use the export's measured width, including compositions
wider than the editor window. Native previews and exports rasterize at 2×.
PDF uses the experimental Takumi vector backend from the same pinned Git revision, sharing SSR, layout, fonts and resources
with PNG. It keeps selectable text on one continuous page, with one CSS pixel mapping
to one PDF point before uniform viewport scaling to at most 14,400 points per side.
PDF has an independent 24 MiB output budget (36 MiB encoded reply), a 10-second wall watchdog and 8-second CPU limit; it is not bounded by PNG pixels. The markup child accepts at most 16 MiB of rendered markup. Each output loads only the image attachments its HTML and CSS reference, with a separate 24 MiB budget; unrelated damage never prevents capture. Images are verified by the core and decoded in the restricted raster child. Output sizes are in [icons and exports](../../apps/landing/src/content/docs/docs/guides/icons-and-exports.mdx#export-from-the-host-or-cli).

The core stores artwork losslessly optimized with oxipng: `pack` at level 2, and a closing window's artwork at level 0, because the writer lock is released only after it is written ([measurements](../evidence/artwork-optimization-2026-10-05.md)). Pixels, dimensions and transparency are preserved, fully transparent pixels keep their colour, alpha is removed only from fully opaque images, and metadata that does not affect display is stripped. A result is kept only when it is smaller; artwork oxipng cannot read, or would decode past 64 MiB, is stored as it is. PNG and PDF exports are written as rendered.

On macOS and Linux, build and register render every declared view from the staged template inside the same `pack` request that writes it: Preview, Icon, Export PNG/PDF and all Widget families. Supplied artwork does not bypass this gate. Only unsupplied Preview/Icon artwork is stored; a failed view render publishes nothing. New documents copy their template's artwork. A window closing a document its session changed, or one without a preview, renders the preview and, when an icon target exists, the icon from a saved backup through Svelte SSR and Takumi in the restricted Rust child; the owner writes them through its writer connection before the file closes. The file's Finder custom icon is a local copy of that artwork (the icon, else the preview), written after a close that wrote artwork, at create and at copy; Finder shows it without Quick Look's icon-mode tile. It is file metadata, never a source: tools that drop metadata lose only the copy, and Finder falls back to the Quick Look thumbnail. The catalog reads the new artwork and updates without reopening. A failed capture or write keeps the previous artwork and never stops the close; a close that fails shows the window again. Quick Look extensions show the artwork in Finder, Mail and the share sheet, for documents that left this Mac too: sandboxed, read-only, through the core. Catalog, Finder and Quick Look display read stored artwork without running authored code or creating a live document owner.

Open and closed exports use independent saved-state renderers. An open export uses the flushed backup described above; a
closed export takes no writer lock and can render while another process holds the document.

## Telemetry

Release enables Firebase Analytics and Crashlytics after configuration; Debug/tests do not initialize the default Firebase app. Collection defaults off in the app plist. Live collaboration uses Sign in with Apple through a separately named Firebase app (Analytics and Crashlytics initialization is unchanged). It starts only when the person signs in or opens an account or sharing panel, or on a device that has signed in before, so documents never shared start no Firebase, including in Debug and test builds; App Check remains deferred. Existing Analytics events record launch, creation source, opening, duplication, and export format. Cancelled operations produce only fixed breadcrumbs.

Non-fatals carry a fixed operation, classification (platform/authored/rejection), and reason. Native storage and WebKit callbacks supply diagnostic categories before error text is flattened. Fixed category-specific domains and stable numeric codes distinguish issue groups; per-event keys can include export format. No document IDs, paths, titles, contents, guest codes, authored error strings, or raw error userInfo enter reports. Document context never uses global Crashlytics keys.

Save and renderer incidents report once until recovery; propagated close/quit/export errors add breadcrumbs. Authored errors, expected rejections, and background catalog/artwork failures share a two-report limit per app launch, once per category, and stop after the first foreground platform failure. Optional artwork and stale Recents remain normal fallbacks. Standalone CLI processes have no Firebase sink.

Release validation requires actual Firebase delivery and symbolication; unit tests cannot establish those. Use a disposable validation build and retain its dSYM and dashboard evidence, without shipping a crash trigger.

## Schema identity

Saved state belongs to the descriptor in the `app` row it is stored with. Packing
writes that row once and a new document copies it from its template, so a document's
state and its descriptor never part. The app declares the descriptor it was built for (`TinyApp.descriptor`; `svelteApp` sets it from its schema), and the
shell refuses to mount an app on a document of another: key order never matters. Schema evolution is deferred.

The native page protocol has one request/reply envelope for document edits and host
services. Text edits are batches: a binding's `apply` carries a `set` with `from` and
`selection`, and the reply adds the merged selection
([architecture](../architecture.md#text)). Rust owns it in `crates/tinys-core/src/wire/page.rs`; core payloads are in
`wire/core.rs`. Requests carry no correlation ID or view token: WebKit
correlates promises and Swift supplies its own lifecycle view token alongside the
request after checking the sender. The page never chooses that token.
The host enters the shell through `__tiny` for publications and lifecycle.
Apps use the restricted `ctx.document` facade and its explicit durability barrier,
`flush()`. The initial owner state includes effective theme values, and ordered publications include
them when they change (defaults come from `tiny.ts`'s `theme`). Authored layout stays in CSS.

Rust decodes page messages once, routes them to owner replies or typed native host actions, and encodes replies. The shell checks the outcome envelope. Agent commands use the restricted native child evaluator; arguments and resulting intents are checked by Rust. Pages edit through handles. The page context and evaluator prelude are dispatched by the stored runtime ABI.

## Resource integrity and responses

Opening checks the app resource inventory, encoding and size bounds, required entrypoints,
private command program and window skin. Other UI resources decode on demand. App media
is served without re-hashing: `pack` verified each content address and the app row seals
it. Attachments retain first-touch hash and media-type verification, cached per resource
reader; compressed assets reuse a bounded decoded cache. A damaged ordinary resource fails to load without
preventing document state from opening. A damaged command program or skin still refuses
at open under its existing checks.

Rust supplies resource response status, headers and byte spans to native, browser-copy
and preview attachment transports. Single closed, open-ended and suffix ranges are
supported; malformed, unsupported-unit or multiple ranges are ignored. Unsatisfiable
ranges return 416 with `Content-Range: bytes */size`. Responses carry `nosniff`, `no-store`
and `Accept-Ranges: bytes`; attachments also carry sandbox CSP. HEAD has the same response
metadata without a body. Browser delivery keeps bounded chunks and cancellation.
