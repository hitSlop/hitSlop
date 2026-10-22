# Direction

Today hitSlop is a signed Mac app and one matching npm package. People create local
documents from bundled or installed templates, edit them in the window or through the CLI
(their coding agent included), and export PNG and PDF.

The iPhone app (iOS 26+) is in TestFlight. It opens, edits and creates slops,
bundles the Popular templates plus Little Days, and imports from Files. The current
iPhone host is portrait-only; per-slop orientation declarations remain future work. The Mac/iPhone Synced Library
keeps managed slops in sync through the person's iCloud account. The Mac app
includes the Synced Library on CloudKit Production, matching TestFlight/App Store
iPhones. iPhone Home Screen
widgets render library slops with a widget view locally, open them on tap, and refresh
in the background when iOS allows.
[Architecture](architecture.md#synced-library) owns the library design.

Live sharing is available in the Mac app and the iPhone app on TestFlight. The
Share popover's **Live collaboration** action creates a link anyone can use to edit
in the Mac/iPhone app or Chrome at share.hitslop.com. The owner signs in with Apple;
Stop ends the link and everyone keeps their copy. See the
[relay architecture](architecture.md#relay) and [sharing operations](../apps/sharing/README.md).

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

Beyond those steps, the longer-term direction includes native hosts for iPadOS, Android,
Windows and Linux; sync beyond Apple devices; an app-store-style catalog for discovering,
publishing and installing community slops; and deeper native integrations. These are
directions without release dates or a platform delivery order. The
[README roadmap](../README.md#roadmap) gives the short version; proposals live in [ideas](ideas.md).

## Open now

- **Apple releases.** Release the iPhone app on the App Store and the Mac app with
  Synced Library on CloudKit Production. The iPhone app, Files import, library sync
  and Home Screen widgets are implemented.
- **Sharing rollout.** Account deletion/Apple revocation, rejoin after re-share,
  guest entry-point rate limits and metrics, frozen v1 HTTP/binary fixtures, universal
  links and Save Unshared Copy are implemented. Privacy disclosures cover relay
  storage and account deletion. Regenerate the Developer ID profile with Sign in
  with Apple and associated domains, then qualify the signed Mac release with
  sign-in, universal links, account deletion and broader device/resource scenarios
  before publication.
  [Architecture](architecture.md#relay) and the [sharing README](../apps/sharing/README.md)
  own the technical description and operating limits.
- **Apple follow-ups.** Qualify the library on Production, including background widget
  refresh on devices, and expand starter coverage.

- An additive authoring environment API exposes native platform/device identity and
  reactive compact/regular viewport layout to Svelte and editor CSS. Browser identity
  stays unknown. `slop dev` device presets preserve accepted edits; static captures
  remain independent. See the [authoring guide](guides/authoring.md#platform-device-and-responsive-layout).

- Native `window.fullscreenable` and the local Chrome browser beta are implemented.
  The same Rust owner/store powers independent OPFS copies, bounded resource reads,
  command workers and Save As downloads back to native. Identity, export barriers,
  failed writes and recovery are covered at their owning boundaries. Browser launch
  polish, sustained-edit/near-limit performance qualification and Safari/mobile browser
  support remain deferred. Hosted links use the live sharing service described above. `slop dev` keeps its native owner.
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
- Loro is pinned to the exact crates.io release 1.17.1, which contains the former
  `c00c9fa` pin and its undo fixes. Re-check crates.io immediately before the first
  frozen capture / tag, and upgrade only with the corpus passing. Capture the first
  corpus entry only with the final pin.
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
  socket, exclusive rename and evaluator for additional platforms. iPhone already
  uses in-process Rust capture; authored commands there remain unavailable. Add a feature
  list to CLI `hello` and a host/feature handshake to page config;
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

- Declared `camera` and `microphone` capabilities gate web capture in Mac documents,
  development previews and local browser copies, with OS/browser consent still required.
  Declared `speech` enables on-device Mac transcription of stored recordings; it is
  unavailable in the iPhone app, development previews and browser copies. There is no per-slop permission UI or
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
- The Synced Library, iPhone app and iPhone widgets are implemented,
  alongside the Mac desktop widgets. Live sharing connects native and browser
  copies through the existing owners. [Sync](architecture.md#sync) owns both designs;
  release and polish work is [open now](#open-now).
- `slop.ts` declares metadata and artwork; `schema.ts` supplies the document,
  `App.svelte` the editor, required `Preview.svelte` and `Icon.svelte`, plus optional `Export.svelte`, `Widget.svelte`
  and `commands.ts` the other roles. Captures use saved-state Svelte SSR and Takumi,
  with restricted children on Mac/Linux and in-process capture on iPhone; there is no
  WebKit capture, automatic fallback or public per-slop override.
  `slop dev` runs the native owner on a disposable copy per preview page.

A historical summary of the ownership decisions and integration findings is retained in
the [pre-launch simplification review](evidence/prelaunch-simplification-2026-10-04.md).

## Later

**Catalog capture fidelity.** The [visual review](evidence/slop-capture-review-2026-10-12.md)
finds blank icons, lost SVG styling and collapsed exports in its historical build. The
current Takumi capture compositions cover the bundled slops, including saved cells and
attachments. Broader theme and populated-state coverage remains product qualification;
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
Broader desktop qualification and cache management UI remain pending. iPhone background
refresh is tracked [above](#open-now); implemented widget behavior lives in the
[architecture](architecture.md#macos-image-widgets).

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

**Further collaboration design.** Beyond the [release qualification](#open-now), person-level
permissions, read-only links, browser creator accounts, CLI share/join, reliable offline
web reopening and room compaction remain later work. Qualify
[container-creation and undo conflicts](reference/document-types.md#storage-layout)
before public rollout. Keep the existing Rust owner and relay boundaries described in
[architecture](architecture.md#relay).

**Sync beyond Apple devices.** The implemented CloudKit Synced Library uses a person's
iCloud account. Relay-based replication of a person's own documents remains an option
for platforms without CloudKit ([proposal](ideas.md#sync-between-your-own-devices)).

## Deferred

App Check, a document history UI, schema evolution,
direct editing of loose files in iCloud Drive and other synced folders (the Synced Library covers a person's own devices), and [other native platforms](ideas.md#other-platforms). [Ideas](ideas.md) proposes
pulling additive schema changes and undoing an agent's change forward. Historical source
may be kept in the optional, Git-ignored `deferred/` archive; it is not in fresh clones.
Pre-launch documents are not migrated.
