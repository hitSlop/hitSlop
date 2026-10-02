# Direction

Today hitSlop is a signed Mac app and matching npm authoring packages. People create local
documents from bundled or installed templates, edit them in the window or through the CLI
(their coding agent included), and export PNG and PDF.

## Where we're heading

AI made writing a small tool cheap. What's still missing is a place where those tools live,
keep their data and stay changeable. hitSlop is that place. Every slop gets merging
storage, saving and recovery, export and a Finder icon without its author doing anything,
and a person and their agent edit the same live document. The next steps make the climb
from using a slop to changing it gentle:

```text
use ──▶ tweak ──▶ ask ──▶ remix ──▶ author
 ✓      CLI only  CLI only  missing    ✓
```

In order, with the reasoning in [ideas](ideas.md):

1. [Tweak panel](ideas.md#tweak-panel): theme tokens in the window.
2. [Ask from the window](ideas.md#ask-from-the-window): the person's agent, launched
   from the toolbar, editing the open document.
3. [Attribution and "Undo that"](ideas.md#attribution-and-undo-that): every change says
   who made it, and an agent's change can be undone.
4. [`slop watch` and `slop mcp`](ideas.md#slop-watch-and-slop-mcp): agents follow edits and
   reach slops without a shell.
5. [Remix](ideas.md#remix) with [additive app upgrades](ideas.md#additive-app-upgrades).
   Both change the engineering contract and need a decision before work starts.

## Open now

- Restore the templates in `archive/slops` as their document kinds land.
- Check system IME composition and native undo by hand; no evidence file covers them.
- Shape Lab: the production-window shadow refresh, and the opening-only black strip, which
  was not reproduced and has no confirmed cause
  ([evidence](evidence/shape-lab-interaction-2026-09-30.md)).
- A long-lived memory study across windows, captures and close/reopen cycles. The window
  benchmarks record footprints only and make no leak claim.

## Later

**Hosted templates.** A publishing service may accept validated immutable template
artifacts, expose a generated OpenAPI catalog, and download immutable cached masters.
Creating a document still makes a writable local copy, and bundled and cached masters
work offline. Plan a separate Cloudflare HTTP module with R2 artifacts and catalog
metadata, with TypeBox authoritative. Publisher identity, upload limits, package isolation
and checksums, immutable release identity and abuse controls are prerequisites. Any
worker kept in a local `deferred/` archive is unsupported scaffolding. The local app
needs no document server.

**Collaboration**, separate from hosted discovery. The owner can exchange Loro updates
with other replicas (it already imports and exports them), authenticate in Swift and
persist opaque updates remotely. Each replica keeps one writer and local SQLite storage.
Frontier version tokens and stateless text edits already work across replicas: a page's
text request names the history it saw, and the core merges it with whatever arrived
since. Closing trims history to the last editing session at most, so sync will need a
retention policy compatible with offline replicas.
Undo can use a Loro `UndoManager` scoped to local origins. Keep credentials outside
authored code, and add a dedicated sync envelope rather than overloading `apply`.
[Ideas](ideas.md#realtime-collaboration-on-durable-objects) sketches rooms on Cloudflare
Durable Objects, the SDK additions, and per-person `s.local` state.

Do not restore JSON room seeds, command/snapshot authority, guest snapshot reconciliation,
JavaScriptCore, data.json, or a second semantic validator. Convergence tests around
internal import/export do not constitute a shipped collaboration product.

## Deferred

Media import, account UI/Auth/App Check, undo UI, schema evolution, iCloud and other
synced folders, and other native platforms. [Ideas](ideas.md) proposes
pulling additive schema changes and origin-scoped undo forward. Historical source may be
kept in the optional, Git-ignored `deferred/` archive; it is not in fresh clones. There is
no migration of documents from earlier builds.
