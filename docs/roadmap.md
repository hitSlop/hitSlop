# Direction

Today pop is a signed Mac app, an iPhone app and one matching npm package. People
create documents from bundled or installed templates, edit them in the window or through
the CLI (their coding agent included), sync them across their devices, edit them live
with others, and export PNG and PDF.

The iPhone app (iOS 26+) is in TestFlight. It opens, edits and creates pop, bundles the
Popular templates and imports from Files; its host is portrait-only, and per-pop
orientation declarations remain future work. iPhone Home Screen widgets render library
pop with a widget view locally, open them on tap, and refresh in the background when iOS
allows. [Cloud documents](cloud-documents.md) sync a person's devices and carry live
collaboration: the Share popover creates a link anyone can use to edit in the Mac or
iPhone app or in Chrome at share.dotpop.click, and Stop ends the link while everyone
keeps their copy. Operations are in [sharing](../apps/sharing/README.md).

## Where we're heading

AI made writing a small tool cheap. What's still missing is a place where those tools live,
keep their data and stay changeable. pop is that place. Every pop gets merging
storage, saving and recovery, export and a Finder icon without its author doing anything,
and a person and their agent edit the same live document. The next steps make the climb
from using a pop to changing it gentle:

```text
use ──▶ tweak ──▶ ask ──▶ remix ──▶ author
 ✓        ✓       CLI only  missing    ✓
```

In order, with the reasoning in [ideas](ideas.md):

1. [Ask from the window](ideas.md#ask-from-the-window): the person's agent, launched
   from the toolbar, editing the open document.
2. [Attribution and "Undo that"](ideas.md#attribution-and-undo-that): every change says
   who made it. Edit ▸ Undo already reverts an agent's edits.
3. [`pop watch` and `pop mcp`](ideas.md#pop-watch-and-pop-mcp): agents follow edits and
   reach pop without a shell.
4. [Remix](ideas.md#remix) with [additive app upgrades](ideas.md#additive-app-upgrades).
   Both change the engineering contract and need a decision before work starts.

Beyond those steps, the longer-term direction includes native hosts for iPadOS, Android,
Windows and Linux; sync beyond Apple devices; an app-store-style catalog for discovering,
publishing and installing community pop; and deeper native integrations. These are
directions without release dates or a platform delivery order. The
[README roadmap](../README.md#roadmap) gives the short version; proposals live in [ideas](ideas.md).

## Open now

Release qualification of the fresh 2.0.0 lineage. The record of what shipped is
[foundation rebuild](foundation-rebuild.md).

- **Deployment and release.** Complete the [operator checklist](../apps/sharing/README.md#operator-checklist),
  run `bun run release:check`, capture the first lineage-2 compatibility corpus on the
  candidate and pin it after publication ([releasing](guides/releasing.md)). Release the
  iPhone app on the App Store.
- **Hands-on sharing checks.** Two devices editing, offline then reconnect, growing a
  shared document past the base cap (the `Outgrown` pause and **Save Unshared Copy…**),
  stopping and rejoining, account deletion, and a guest upgrading to Apple sign-in with
  the same UID.
- **Outgrown warning.** Warn at about 75% of the 16 MiB base cap, before sharing pauses.
- **Large-value authoring check.** Rewriting a large scalar every commit stores the whole
  value each time; `pack` could flag large values that belong in attachments.
- **Membership policy.** A reset anonymous credential leaves its old UID's membership in
  place until revocation, and a document admits at most 64 members. Decide owner member
  management and any change to the limit; no quota changes have been made.
- **Resource leases after a member leaves.** See [ideas](ideas.md#resource-leases-after-a-member-leaves).
- **Who may compact, and who owns an ID.** Any invited editor, including an anonymous guest,
  can compact a room, and the Worker validates only the digest and frontier of the base it
  receives; decide owner-only compaction or validating the base. Document UUIDs are chosen by
  the client and the first named account to register one owns it, so a recipient who enables
  sync first can block the original owner; consider server-derived IDs or a nonce.
- **Structural follow-ups (no feature change).** A typed library membership state with
  `CHECK` constraints and dropping the unread `documents.revision` column; one owned object
  per browser connection attempt in `packages/pop/src/sharing/worker.ts`; small scheduling
  and write types in the Rust owner's persistence; sharing receipts moved into the library
  index database; splitting `CloudDocument::fetch`, `packages/pop/src/browser/host.ts::own`,
  `pop-library`'s `lib.rs` and `DocumentSession.swift`.
- **App Clip.** A spike after the iPhone app is on the App Store: whether the Rust sync
  client's sockets run in a Clip, and how a guest's anonymous Firebase identity reaches the
  installed app. A Smart App Banner on the join page is the cheaper first step.
- **Widgets.** Requalify every widget family and size on the release candidate, and
  background refresh on devices.
- **Older documents on iPhone.** Edge-to-edge editors need safe-area padding and a
  reservation for the floating actions menu; documents keep their compiled app and may
  lack these styles. Qualify released documents on the phone (top and bottom controls,
  menus, keyboard entry) and record overlap as a known layout issue.
- **Hand checks.** Quick Look on a document received by Mail and AirDrop (quarantined
  copies, a file being written, a crashed write); system IME composition and Edit ▸ Undo
  (typing, ⌘Z inside a field, an agent edit between steps); the native prompt for an
  unsent text draft.
- **Licenses.** Finish the bundled-library license review; fonts and other non-JS media
  rely on it because bundle comments do not cover them.
- **Dependencies and engines.** Qualify any Loro upgrade against every published corpus.
  Exercise the release engines on darwin-arm64, linux-x64 and linux-arm64.
- **Limits and memory.** Qualify whole-document Time Machine copies at the published
  attachment and storage limits. Run a long-lived memory study across windows, captures
  and close/reopen cycles, including owner-queue growth, before tuning admission caps.
- **Window shadow.** The production-window shadow refresh, and an opening-only black strip
  beneath a window's content that was not reproduced.
- **Optional history trim.** A slimmer history for shared documents using Loro shallow
  snapshots; see [ideas](ideas.md#slimmer-history-for-shared-documents).

## Next

- **Review the restored examples in the app.** Check each in the built app and PNG/PDF
  export, and review the bundled selection. One example stays archived because its
  engine is AGPL.
- **Author diagnostics.** A read-only [`pop doctor`](ideas.md#pop-doctor) for
  installation and PATH conflicts, CLI/SDK alignment, engine availability and skill
  links, explaining why a rebuilt template does not update an existing document.
- **Preview asset serving.** `pop dev` serves only the pop folder and the CLI, so assets
  inside packages (fonts, models) must be copied into the pop to preview.
- **Worker and worklet assets.** Explicit worker policy and build support for worker and
  worklet entry points, emitted as files in `assets` so `new Worker` and `addModule` load
  local URLs. `blob:` and `data:` code stays refused. WebKit does not isolate `pop:`
  pages, so there is no `SharedArrayBuffer`.
- **Failure hardening.** An aggregate close-time artwork deadline (rendering under the
  writer lock is bounded only by per-child watchdogs) and, after measurement, a cap on
  concurrent restricted children.
- **Platform drivers and feature discovery.** A driver seam for the registry, socket,
  exclusive rename and evaluator, the prerequisite for
  [other platforms](ideas.md#other-platforms), and a feature list in CLI `hello` and page
  config ([feature discovery](ideas.md#runtime-feature-discovery)).
- **Storage evolution.** A "needed by this write" parameter for lazy migration, split
  read/write versions in `user_version`, and a rename of `artwork.png`. A future storage
  version ships with a backup of each file before its first migration, a test that a
  frozen older file migrated by a write equals a newly created one, and a test that an
  interrupted migration leaves the older file intact.
- **Native Svelte checker.** `svelte-check-native` matched `svelte-check` on every pop and
  most injected mistakes and runs 8 to 10 times faster, but has one maintainer, unsigned
  binaries, no release CI and exits 0 on some syntax errors. Look again when TypeScript
  7.1 gives `svelte-check` a JS API or the native checker has provenance and CI.

## Later

**Catalog capture fidelity.** Broader theme and populated-state coverage of the Takumi
capture compositions remains product qualification; successful rendering does not imply
support for every browser CSS feature.

**Widget qualification.** Finish signed product testing of all sizes, appearance modes,
refresh budgets and sleep/wake, and cache management UI. See the
[architecture](architecture.md#macos-image-widgets).

**Menu-bar presentation.** Explore [menu-bar pop](ideas.md#menu-bar-pop) for timers,
players and quick capture. Ownership, saving, quit behavior and background work need a
lifecycle design first.

**Permissions and device services.** Code-bound grants and per-pop consent remain a
[proposal](ideas.md#permissions-bound-to-the-apps-code), as do MIDI, calendar and
reminders, notifications, Shortcuts/Siri, a "Send to pop" share extension, packaged
worker and audio-worklet support and broader transcription
([native integrations](ideas.md#native-integrations-and-richer-media)). Camera,
microphone and on-device Mac transcription already work. New capability names need an
implementation and owning-boundary tests.

**Hosted templates.** An app-store-style catalog where people discover, publish and
install community pop. A publishing service would accept validated immutable template
artifacts and serve a generated OpenAPI catalog; creating a document still makes a
writable local copy, and bundled and cached masters work offline. Publisher identity,
upload limits, package isolation, checksums, immutable release identity and abuse controls
are prerequisites.

**Further collaboration.** Person-level permissions, read-only links, browser creator
accounts, CLI share/join and reliable offline web reopening. Keep qualifying
[container-creation and undo conflicts](reference/document-types.md#storage-layout) as
collaboration grows.

**Sync beyond Apple devices.** Future native platforms can reuse the Rust document client
and add their own account and catalog adapter ([cloud documents](cloud-documents.md)).

## Deferred

App Check, a document history UI, schema evolution, direct editing of loose files in
iCloud Drive and other synced folders, and [other native platforms](ideas.md#other-platforms).
[Ideas](ideas.md) proposes pulling additive schema changes and undoing an agent's change
forward.
