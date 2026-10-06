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
lock and the save policy. Attachments are immutable rows in the file, stored and read
through the core. `DocumentOwner` schedules saves (one write in flight on the persistence
queue) and owns the socket and ordered delivery to the page. The private
`@hitslop/shell` package holds immutable snapshots and no CRDT;
`@hitslop/document` contains the author SDK. Never add a second document engine, a JSON
mirror, a JavaScriptCore evaluator or app-specific Swift schemas.

**One edit path.** The CLI forwards to the live owner's socket or takes the lock and runs
the owner in-process (`DocumentCommand.connect`). The writer lock and the live owner's
discovery live outside the file, in the core's registry (`~/.hitslop/live`, keyed by the
file's device and inode); never remove a lock file there or bypass a busy lock, and never
lock or open the database file outside the core. Closed edits never start WebKit. A
document is a local regular file: never a link, and never with a second hard link.

**Errors.** Classify every failure once, through `RequestOutcome`: rejected (with a core
code), replaced, closing, invalidated, save failed, or unknown. Only unknown leaves the
outcome uncertain; after it, run `slop get` before another edit. A failed save keeps
ownership and shows a native retry; close and export flush first.

**Contracts.** TypeBox in packages/schema generates the socket, page, manifest and
core wire; run `bun run schema:generate` and never edit generated files. The core checks
envelopes and manifests against those schemas and parses payloads strictly. WebKit
correlates page replies; Swift checks the sender and supplies its native view token.
Keep native view fences and socket epochs when changing the page protocol. Released
documents stay openable: follow AGENTS.md's Compatibility rules, and keep
`tests/compat` passing (`CompatCorpusTests`, `bun run test:compat`).

**Windows and captures.** `SlopSilhouette` builds paths from the core's parsed shape and
is the one mask for clipping, hit testing and window-sized PNG captures. Dedicated
`Export.svelte`/`Icon.svelte` captures are never masked. Captures hold `capturing` for
their whole run; page resizes are refused meanwhile.

**Checks.** `bun run build` builds runtime resources and the native helper;
`bun run swift:test` covers WKWebView, live and closed CLI, save failure, close and
export; `bun run test:native` runs the CLI against the helper;
`bun run bench:windows` measures window scaling. Templates: `bun run build:templates`,
then `bun run test:render`.
