# Direction

Today hitSlop is a signed Mac app and one matching npm package. People create local
documents from bundled or installed templates, edit them in the window or through the CLI
(their coding agent included), and export PNG and PDF.

An unshipped Mac/iPhone library preview adds account-scoped CloudKit whole-file sync,
named creation, shared catalog branding, and phone-generated icon/preview artwork on
close. It starts with Quick Checklist on iOS 26; the Mac minimum stays 15.2. Production
qualification, broader templates, rename, widgets, and compaction are follow-ups.

## Where we're heading

AI made writing a small tool cheap. What's still missing is a place where those tools live,
keep their data and stay changeable. hitSlop is that place. Every slop gets merging
storage, saving and recovery, export and a Finder icon without its author doing anything,
and a person and their agent edit the same live document. The next steps make the climb
from using a slop to changing it gentle:

```text
use ──▶ tweak ──▶ ask ──▶ remix ──▶ author
 ✓        ✓       CLI only  missing    ✓
```

In order, with the reasoning in [ideas](ideas.md):

1. [Ask from the window](ideas.md#ask-from-the-window): the person's agent, launched
   from the toolbar, editing the open document.
2. [Attribution and "Undo that"](ideas.md#attribution-and-undo-that): every change says
   who made it. Edit ▸ Undo already reverts an agent's edits.
3. [`slop watch` and `slop mcp`](ideas.md#slop-watch-and-slop-mcp): agents follow edits and
   reach slops without a shell.
4. [Remix](ideas.md#remix) with [additive app upgrades](ideas.md#additive-app-upgrades).
   Both change the engineering contract and need a decision before work starts.

Beyond those steps, the longer-term direction is native hosts for iOS/iPadOS, Android,
Windows and Linux; realtime collaboration; offline-capable sync between your own
devices; an app-store-style catalog for discovering, publishing and installing community
slops; and deeper native integrations. These are directions without release dates or a
platform delivery order. The [README roadmap](../README.md#roadmap) gives the short version;
the work and prerequisites are below and in [ideas](ideas.md).

## Open now

- An additive authoring environment API exposes native platform/device identity and
  reactive compact/regular viewport layout to Svelte and editor CSS. Browser identity
  stays unknown. `slop dev` device presets preserve accepted edits; static captures
  remain independent. See the [authoring guide](guides/authoring.md#platform-device-and-responsive-layout).

- Native `window.fullscreenable` and the local Chrome browser beta are implemented.
  The same Rust owner/store powers independent OPFS copies, bounded resource reads,
  command workers and Save As downloads back to native. Identity, export barriers,
  failed writes and recovery are covered at their owning boundaries. Browser launch
  polish, sustained-edit/near-limit performance qualification, Safari/mobile and hosted
  delivery remain deferred. `slop dev` keeps its native owner.
  [Hardening evidence](evidence/browser-host-hardening-2026-10-08.md) records the scope
  and verification limits.
- A text draft whose control is destroyed after an unknown-outcome send is retained in
  the page shell. Native close/quit offers to copy the complete text, discard it or cancel;
  cancelling retains the draft and resumes editing. Recovery after renderer termination
  remains deferred. Qualify the prompt with the 1.0.1 release candidate.
- Qualify the release candidate with the current [release checks](guides/releasing.md).
  Complete-value replacement, template acceptance, command declaration checks,
  dedicated captures and document UUID copy rules are implemented. Historical foundation
  runs do not certify the current candidate.
- Before the first public release, finish the bundled-library license review. App bundles
  retain library license comments; fonts and other non-JS media rely on the review because
  those comments do not cover them.
- Requalify every widget family and size on the release candidate before capturing the
  first corpus entry; the earlier hands-on reviews predate the two-stage capture ABI.
- At launch, capture and freeze the first [compatibility corpus](testing.md#compatibility-corpus)
  entry; capture one permanent entry per released version thereafter, even when markers
  stay unchanged. From then on every released document stays openable.
- Storage version 2, whenever it comes, ships with: a backup of each file before its first
  migration (a clean copy, `file/copy.rs`); a test that a frozen version-1 file migrated by
  a write equals a newly created version-2 file in layout and value; and a test that an
  interrupted migration leaves the version-1 file intact.
- Freezing that entry also freezes runtime ABI 1's command prelude: capture records its
  digest, and `scripts/build/runner.ts` refuses to change it afterwards.
- Check Quick Look by hand on a document received by Mail and AirDrop. Quarantined copies,
  a file another process is writing and a crashed write (the document icon until the app
  recovers it) are checked.
- The first release ships with the current attachment, asset and storage limits; raising
  them later requires a storage-version bump. Qualify whole-document Time Machine copies
  at those limits.
- Exercise the release engines on darwin-arm64, linux-x64 and linux-arm64 (Windows is
  outside the current release targets; a native Windows host is a longer-term direction).
- Loro is pinned to a git commit of its main branch (`c00c9fa`), because the fixes undo
  relies on are not on crates.io. Return to an exact crates.io version once one ships
  them, with the corpus passing. Re-check crates.io immediately before tagging: on
  2026-10-14 the pin was 41 commits ahead of the latest release, 1.16.2. Capture the first
  corpus entry only with the final pin. Re-checked the same day: crates.io still ends at
  1.16.2, and Loro's main is three unreleased commits past the pin (history and import
  hardening, about 5,400 added lines, five days old), so the pin stays. Check again
  immediately before the first frozen capture.
- Check system IME composition and Edit ▸ Undo by hand (typing, ⌘Z inside a field, an
  agent edit between steps); no evidence file covers them.
- The production-window shadow refresh, and an opening-only black strip beneath a window's
  content, which was not reproduced and has no confirmed cause
  ([evidence](evidence/shape-lab-interaction-2026-09-30.md)).
- A long-lived memory study across windows, captures and close/reopen cycles, including
  owner-queue growth and capture concurrency before choosing admission caps. The window
  benchmarks record footprints only and make no leak claim.
- Recover unresolved text after its field unmounts. A draft whose outcome is unknown is
  reported and dropped when its control unmounts. Recovering it needs a UI outside the
  control; indefinitely holding close after unmount previously made windows impossible
  to close.

## Next

- **Review the restored examples in the app.** The archived examples now live in
  `slops/` in the current format, with model modules and commands. They have been checked
  in the dev preview; individual native checks do not qualify the entire collection.
  Check each in the built app and PNG/PDF export, and review the bundled selection.
  One example stays archived because its engine is AGPL.
- **Author diagnostics.** Add a read-only [`slop doctor`](ideas.md#slop-doctor) for
  installation and PATH conflicts, CLI/SDK alignment, engine availability and skill links.
  Explain why a rebuilt template does not update an existing document, and suggest
  concrete next steps. Repairs remain explicit actions.
- **Preview asset serving.** `slop dev` serves only the slop folder and the CLI, so
  assets inside packages (fonts, models) must be copied into the slop to preview.
- **Worker and worklet assets.** The shared page CSP is defined in Rust for native and
  browser pages. Explicit worker policy and build support for worker and
  worklet entry points remain: emit them as files in `assets` so `new Worker` and
  `addModule` load local URLs. Keep `blob:` and `data:` code refused and make failures
  understandable. WebKit does not isolate `slop:` pages, so there is no
  `SharedArrayBuffer`.
- **Failure hardening.** Add an aggregate close-time artwork deadline; rendering under
  the writer lock is currently bounded only by per-child watchdogs. Consider a cap on
  concurrent restricted children after measurement. Unresolved detached-text recovery remains open above.
- **Platform drivers and feature discovery.** Define a driver seam for the registry,
  socket, exclusive rename and evaluator, including an in-process evaluator story for
  iOS. Add a feature list to CLI `hello` and a host/feature handshake to page config;
  reply readers already allow these additive members. [Feature discovery](ideas.md#runtime-feature-discovery)
  should describe actual host support separately from an app's declarations and the
  person's permissions, so apps can offer useful fallbacks on native and browser hosts.
  API shape remains design work. The drivers are the prerequisite for
  [hosts on other platforms](ideas.md#other-platforms).
- **Storage evolution.** Give lazy migration a "needed by this write" parameter, split
  `user_version` into read/write versions, and rename `artwork.png`. Keep the storage-2
  backup and interruption checklist above.
- **Revisit the native Svelte checker.** `svelte-check-native` (Rust, TypeScript 7's native
  compiler) matched `svelte-check` on every slop and 14 of 15 injected mistakes, and runs
  about 8 to 10 times faster, but it has one maintainer, ships unsigned binaries without
  release CI, trails `svelte-check` by weeks, and by default exits 0 on CSS and some markup
  syntax errors. Look again when TypeScript 7.1 gives `svelte-check` a JS API (which lets
  everything move to TypeScript 7) or when the native checker has provenance, CI and a
  maintainer team. The repo itself type-checks cleanly on TypeScript 7 already.

## Implemented foundation

- Live and closed CLI exports share one staged, synced, exclusive publisher; a failed
  write leaves no destination file and an unsynced folder after publishing is an unknown
  outcome. Command evaluator panics complete as `host_fault` and leave the worker usable. Page
  admission uses one exhaustive capability gate; browser permissions policy has a record
  for every capability, and release checks assert required usage-description and
  entitlement keys. Command envelope readers ignore unknown members, the CLI handles
  unfamiliar outcome codes conservatively, and theme tokens reserve host namespaces.
- App identity uses a fixed encoding; see the
  [app identity contract](engineering-contract.md#app-identity).

- The native failure screen retains accepted edits and offers Reopen, Copy details and
  Get help; a pending save keeps its recovery sheet. Page-call deadlines and never-ready
  recovery use the same renderer-death path. Reports include the slop's and build's format
  and runtime markers. Widget location refresh follows renames and same-volume moves
  without replacing cached images.

- Declared `camera` and `microphone` capabilities gate web capture in native documents,
  development previews and local browser copies, with OS/browser consent still required.
  Declared `speech` enables native on-device transcription of stored recordings; it is
  unavailable in previews and browser copies. There is no per-slop permission UI or
  server fallback. [Capabilities](reference/runtime.md#security-boundaries) records the boundary.
- Saved-source capture preserves live speech, file selection, downloads and resize eligibility;
  close preparation, page replacement and renderer loss cancel pending transcription.
  Native capture uses a shared Rust runtime and per-output batch results; close batches
  artwork and widgets with shared prepared inputs. Moving live capture coordination
  fully into Rust, production-renderer dev previews, role-isolated dev previews, and a
  shared service admission/completion API remain deferred; no new personal-data services are included in that work.
- Compatibility readers ignore unknown members; versioned open/read/save limits and
  positive acceptance vectors are pinned by the corpus. Each release records its shipped
  writer by hash alongside its source commit. Corpora and exact writer packages live
  in immutable release assets; replay downloads and verifies them. The conformance
  fixture covers public data, command, media, presentation and capture boundaries.
  The candidate workflow captures and tests once; approval publishes those exact bytes.
- Static roles import their own CSS in import order, without editor stylesheet inheritance.
  Pack renders every declared view, including Export PNG/PDF and all Widget families,
  even with supplied artwork. `slop dev` still mounts static views inside the editor page.
- Active templates are discovered under `slops/`; `bundled.json` chooses the starters.
  Xcode's embedding phase refreshes `generated/templates/` through the template cache
  before copying selected starters into the app. Prepare changed tooling first; see
  [the build workflow](guides/development.md#build-current-templates-with-the-app).
- Rust owns edit admission, save scheduling, discard, close, command dispatch and the
  live socket. Swift delivers events and provides native UI and rendering.
- The CLI's engine creates, edits and validates documents on macOS and Linux, renders template artwork and exports closed documents itself. The Mac app links
  its core independently.
- The CLI, SDK, contracts, templates and skills ship as one `hitslop` npm package with
  the Mac app's release version.
- Rust types own every wire and the app definition; ts-rs generates TypeScript and UniFFI
  carries types to Swift. Command arguments are `s.*` descriptors the owner checks.
- Beta commands are agent-only and accept flat scalar arguments. They run in the owner's restricted evaluator; pages use document handles
  and the page bundle carries no command bodies.
- Theme overrides share Loro storage, sequence, publications, undo and saving with data.
- Sync's core and its proof: one Loro peer per session, three-way text merges without
  `base`, Loro's `UndoManager`, a merge-closed layout, the `history`/`share` storage tables,
  and the sync session with its owner hooks ([architecture](architecture.md#sync)). The
  core protocol was also qualified against a local Durable Object byte relay on
  Cloudflare's runtime ([evidence](evidence/sync-spikes-2026-10.md)). The experiment's
  relay source is outside this repository. The production relay and clients are
  [later](#later).
- `slop.ts` declares metadata and artwork; `schema.ts` supplies the document,
  `App.svelte` the editor, required `Preview.svelte` and `Icon.svelte`, plus optional `Export.svelte`, `Widget.svelte`
  and `commands.ts` the other roles. All production captures use saved-state Svelte SSR
  and Takumi in the restricted Rust child, including selectable vector PDF; there is no
  WebKit capture, automatic fallback or public per-slop override.
  `slop dev` runs the native owner on a disposable copy per preview page.

A historical summary of the ownership decisions and integration findings is retained in
the [pre-launch simplification review](evidence/prelaunch-simplification-2026-10-04.md).

## Later

**Catalog capture fidelity.** The [76-slop visual review](evidence/slop-capture-review-2026-10-12.md)
initially found blank icons, lost SVG styling and collapsed exports. The Takumi-only
pass repairs those cases and provides synchronous capture compositions for all 76
bundled slops. Rich browser views use static compositions; Pixel Art preserves its
saved cells, and image-based exports resolve saved attachments in the render child.
Seven additions from that review now have widget compositions: Water Tracker,
Grocery List, Reading Tracker, Habit Heatmap, Daily Planner, Assignment Tracker
and Countdown Milestones, alongside Quick Checklist. Ivy Lee Method was not
selected. Broader theme and populated-state coverage remains product qualification;
successful rendering does not imply support for every browser CSS feature.

**Widget qualification.** The macOS image-widget integration includes dedicated saved-state
captures, document selection, bookmark opening and optional approximate refreshes.
Saved edits and renderer/font changes invalidate image generations; the app watches
cached sources and catches up at startup. Shared tests cover publication, reload requests,
restart catch-up and retaining prior images on failure. Hands-on reviews of the signed
installation (Oct 12 images; Oct 13 edit, layering, dragging, rename/move) predate the
two-stage capture ABI and do not qualify every size or lifecycle; repeat them on the
release candidate. See [native capture evidence](evidence/takumi-capture-2026-10-12.md) for measured
macOS costs and the review checklist.
Finish signed product testing of all sizes, appearance modes, refresh budgets and
sleep/wake; the [platform spike](evidence/widget-images-2026-10-11.md) is narrower evidence.
The [October 13 widget review](evidence/widget-review-2026-10-13.md) records the location
metadata fixes and removal of stale development registrations.
The `slop dev` composition selector previews all three widget families with HMR. Broader desktop qualification, cache management UI and iOS hosting are pending.

**Menu-bar presentation.** Explore [menu-bar slops](ideas.md#menu-bar-slops) for timers,
players and quick capture, alongside the implemented widgets. Consider creating the
WebView only when its editor is opened. Ownership, saving, quit behavior and any
background work need a lifecycle design before this becomes an authoring option.

**Permissions and device services.** Code-bound grants and per-slop consent remain a
[proposal](ideas.md#permissions-bound-to-the-apps-code), as do additional devices such
as MIDI. [Native integrations and richer media](ideas.md#native-integrations-and-richer-media)
also proposes calendar and reminder access, notifications, Shortcuts/Siri, a "Send to slop"
share extension, packaged worker/audio-worklet support and broader transcription.
Camera, microphone and supported on-device Mac transcription already work; browser
transcription and a newer native speech backend are not implemented. New capability
names require an implementation and owning-boundary tests; the proposed additions are
not authoring options today.

**Hosted templates.** The goal is an app-store-style catalog where people discover,
publish and install community slops. A publishing service may accept validated immutable template
artifacts, expose a generated OpenAPI catalog, and download immutable cached masters.
Creating a document still makes a writable local copy, and bundled and cached masters
work offline. Plan a separate Cloudflare HTTP module with R2 artifacts and catalog
metadata, with Rust acceptance authoritative. Publisher identity, upload limits, package isolation
and checksums, immutable release identity and abuse controls are prerequisites. Any
worker kept in a local `deferred/` archive is unsupported scaffolding. The local app
needs no document server.

**Collaboration**, separate from hosted discovery: each device edits its own `.slop`, and
copies exchange Loro updates through a Durable Object byte relay. The core is in place and
proven against the relay on Cloudflare's runtime; [architecture](architecture.md#sync)
describes the design and its accepted limits, and
[ideas](ideas.md#realtime-collaboration-on-durable-objects) holds the open proposals
(presence, per-person `s.local` state).

The Rust Worker has passed a disposable hosted probe, and real owners have passed local
full-file seed and simulated CloudKit coexistence checks
([evidence](evidence/live-sharing-spikes-2026-10-18.md)). Signed isolated Mac/iPhone probes
passed shared Keychain delivery, native Rust relay convergence, real checkpoint throttling
and phone background/kill/relaunch
([device evidence](evidence/live-sharing-native-spike-2026-10-18.md)). A public fixture also
passed real HTTPS hitslop.com/dotslop.com origin/storage checks, native/browser concurrency,
late join, process reopen and SQLite download/native reopen. Connected application echo
RTT medians were 144–168 ms ([web evidence](evidence/live-sharing-web-spike-2026-10-18.md));
one-way and paint latency were not measured. Embedded Service Workers were denied under
Chrome cookie restriction; direct immutable fixture UI assets passed instead. A follow-up
private-resource experiment passed scoped grants, app/media verification, seed/new blob
delivery, offline import/reconnect and native/browser convergence
([resource evidence](evidence/live-sharing-resources-spike-2026-10-18.md)). The deliberately
simple per-record inventory checks raised Mac/browser echo medians to 430–439 ms in that
run. The earlier integrated preview added the existing app's Share/Copy Link/Join/Stop
controls, synchronizable opaque Keychain credentials, crash-safe staged publication,
aggregate owner queue admission and process-local 60-second CloudKit checkpoints.
It deploys a Loro-free Rust relay and private R2 resources, a trusted
`share.hitslop.com` browser host and isolated UUID `dotslop.com` app origins. Rust owns
HTTP DTOs/OpenAPI, the binary protocol and native transport. No oRPC or Swift HTTP
implementation is needed.

The deployed fixture campaign passes native/browser convergence, late join, restricted
cookie resources, interrupted receive ordering, connected browser process reopening,
SQLite download/native reopen and stopping/revocation
([integrated evidence](evidence/live-sharing-preview-2026-10-19.md)). The signed Mac app
also passed creation, Copy Edit Link, credential restoration, native/browser edits,
Stop and continued local editing. The feature stays disabled by default.

The beta foundation now shares Rust attachment selection and seeds transfer caches
from server inventory. The deployed reconnect check passes with zero attachment PUTs
after process reopen. A gated native Apple/Firebase panel, Rust/WASM token probe and
CloudKit diagnostic namespace/traces are added. Signed Mac Apple/Firebase sign-in and
deployed Rust/WASM verification pass. A later physical device pass confirms matching
Mac/iPhone UID and account recovery; see the [phone evidence](evidence/live-sharing-phone-2026-10-19.md).
See the [foundation evidence](evidence/live-sharing-accounts-foundation-2026-10-19.md)
for that proof and its device gates. The next account lifecycle slice is now implemented
behind the developer flag ([account evidence](evidence/live-sharing-accounts-2026-10-19.md)).

The account lifecycle now reserves rooms per UID/document/generation, recovers owner
invitations and checks a verified immutable owner UID in each room. Native creation,
recovery and Stop use refreshed Firebase identity; guests use device-local invitations.
The creator admin key, preview creation key, staging marker, synchronizable credential
storage and two-second polling are removed. A stable immutable SQLite seed is the local
retry journal. Rust callbacks report transport status; recovery runs on open,
foreground, sign-in and Share. Re-share creates a new generation; Stop wins over late
publication/uploads. Rust owns the 10-room/UID and 32-connection/room limits; new
reservation creation has a kill switch and UID/IP limits. HTTP APIs use `/v1/`.
The local real-WASM/SQLite DO/R2 check covers two creators and cold recovery. The
physical Mac/iPhone pass confirms initial CloudKit discovery, owner invitation recovery
without transferring a link, live edits both ways, background catch-up and second-device
Stop. It exposed a cold-launch library registration race, now protected by a native
integration regression. The repeat passes post-Stop CloudKit delivery, re-share recovery,
native Mac Join reusing the open owner and phone background/kill/relaunch. The updated deployed
browser campaign now passes with a signed Mac creator and signed-out native/browser
guests, including deliberate offline/reconnect, connected refresh/process reopen,
attachment barriers, download/native reopen, zero redundant attachment PUTs and Stop
([tester evidence](evidence/authoring-environment-2026-10-19.md)). Existing unversioned preview rooms
require remote retirement.

This slice targets a gated tester preview. Its deployed guest/offline/browser campaign
passes; broader device/resource qualification remains open. One follow-up: native guests cannot yet replace an ended room
membership when opening a new generation after Stop/re-share.

Before broader sharing, prioritize Save Unshared Copy from accepted in-memory state
when source history cannot save, then account deletion/Apple revocation and privacy
disclosures. Complete remaining resource/frame throttles and metrics, and freeze HTTP
and postcard-envelope contracts. Qualify broader phone resources and offline cases.
The full-display editor and native
Back/loading are verified; the user will check the physical edge-swipe gesture.
Keep connected web opening as
the first scope. Four same-device relay acknowledgement samples in the physical run
were 83–152 ms; cold bootstrap/inventory took longer. These are not one-way or paint
latency measurements, and do not establish a percentile.

Later: person-level permissions, read-only invitations, public catalog, browser creator
accounts, CLI share/join, reliable offline web reopening and room compaction/retention.
Initial seeds and retained relay updates each have a 16 MiB preview limit. Qualify
[container-creation and undo conflicts](reference/document-types.md#storage-layout)
before shipping sharing.
Do not restore JSON room seeds, a second command validator, guest snapshot reconciliation,
JavaScriptCore, data.json, or a second semantic validator. Convergence tests around
internal import/export do not constitute a shipped collaboration product.

**Sync between your own devices.** On Apple devices this is the CloudKit Synced Library
above: whole-file checkpoints merged by the Rust owner, no account or server. Relay-based
replication of a person's own documents is a later option for platforms without CloudKit
([proposal](ideas.md#sync-between-your-own-devices)).

## Deferred

App Check, a document history UI, schema evolution,
iCloud and other synced folders (the Synced Library covers a person's own devices), and [other native platforms](ideas.md#other-platforms). [Ideas](ideas.md) proposes
pulling additive schema changes and undoing an agent's change forward. Historical source
may be kept in the optional, Git-ignored `deferred/` archive; it is not in fresh clones.
Pre-launch documents are not migrated.
