---
name: hitslop-native
description: Work on hitSlop Apple hosting, the document owner, local storage, template caching, previews, export, window shapes, or the native CLI.
---

# Native host

Preserve the real macOS client and its target graph: Core, Document, Runtime, Host,
Features/TCA, Catalog/local templates, Firebase Analytics/Crashlytics, Sparkle and
NativeCLI. Do not replace it with a playground shell. Read docs/architecture.md first.

**Ownership.** A `.slop` is one SQLite file: the app (its `app` row, assets and artwork)
and, for a document, its saved state, theme overrides and attachments. `hitslop-core`
(Rust on Loro, linked through UniFFI as `HitSlopCoreBinding`) owns document semantics,
validation, window-shape geometry, error codes and durable storage: the file, the writer
lock, save policy, owner scheduling and Unix socket routing. Attachments are immutable
rows in the file, stored and read through the core. The shared Rust owner runs one write
in flight on its persistence worker. Swift `DocumentOwner` forwards typed requests and
events; `DocumentSession` delivers ordered publications to the page. The private
`@hitslop/shell` package holds immutable snapshots and no CRDT;
`@hitslop/document` contains the author SDK. Never add a second document engine, a JSON
mirror, a JavaScriptCore evaluator or app-specific Swift schemas.

**One edit path.** The CLI forwards to the live owner's socket or takes the lock and runs
the shared Rust owner in-process. The bundled `slop-engine` handles data commands; the
native helper supplies WebKit only for rendering and macOS services. The writer lock and the live owner's
discovery live outside the file, in the core's registry (`~/.hitslop/live`, keyed by the
file's device and inode); never remove a lock file there or bypass a busy lock, and never
lock or open the database file outside the core. Closed edits never start WebKit. A
document is a local regular file: never a link, and never with a second hard link.

**Errors.** Rust classifies command outcomes; `RequestOutcome` maps native UI failures
onto the same contract: rejected (with a core
code), replaced, closing, invalidated, save failed, or unknown. Only unknown leaves the
outcome uncertain; after it, run `slop get` before another edit. A failed save keeps
ownership and shows a native retry; close and export flush first.

**Contracts.** TypeBox in packages/schema generates the socket, page, manifest and
core wire; run `bun run schema:generate` and never edit generated files. The core checks
envelopes and manifests against those schemas and parses payloads strictly. WebKit
correlates page replies; Swift checks the sender and supplies its native view token.
Keep native view fences and the command protocol check when changing the page or socket protocol. Released
documents stay openable: follow AGENTS.md's Compatibility rules, and keep
`tests/compat` passing (`CompatCorpusTests`, `bun run test:compat`).

**Windows and captures.** `SlopSilhouette` builds paths from the core's parsed shape and
is the one mask for clipping, hit testing and window-sized PNG captures. Dedicated
`Export.svelte`/`Icon.svelte` captures are never masked. Flush and copy saved state before
rendering in an independent hidden page. The editor holds `capturing` only while acquiring
that copy; rendering can continue after the editor closes. Without `Export.svelte`, export
uses a fresh `App.svelte` with its default transient view state. Theme overrides live in
Loro and use the same publication, undo and saving path as data edits.

**Checks.** `bun run build` builds runtime resources and the native helper;
`bun run swift:test` covers WKWebView, live and closed CLI, save failure, close and
export; `bun run test:native` runs the CLI against the helper;
`bun run bench:windows` measures window scaling. Templates: `bun run build:templates`,
then `bun run test:render`.
