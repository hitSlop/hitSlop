# Direction

Today pop is a signed Mac app, an iPhone app and one matching npm package. People
create documents from bundled or installed templates, edit them in the window or through
the CLI (their coding agent included), keep them in sync across their devices, edit them
live with others, and export PNG and PDF.

The iPhone app (iOS 26+) is in TestFlight. It opens, edits and creates pop,
bundles the Popular templates and imports from Files. The current
iPhone host is portrait-only; per-pop orientation declarations remain future work. The Mac/iPhone Synced Library
is being replaced by Firestore discovery and persistent Cloudflare documents;
see the [cloud document specification](cloud-documents.md) for the approved cutover. iPhone Home Screen
widgets render library pop with a widget view locally, open them on tap, and refresh
in the background when iOS allows.
[Architecture](architecture.md#synced-library) owns the library design.

Live sharing is available in the Mac app and the iPhone app on TestFlight. The
Share popover's **Live collaboration** action creates a link anyone can use to edit
in the Mac/iPhone app or Chrome at share.dotpop.click. The owner signs in with Apple;
Stop ends the link and everyone keeps their copy. See the
[relay architecture](architecture.md#relay) and [sharing operations](../apps/sharing/README.md).

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

## Rust foundation redesign

This work permits breaking internal crate APIs while preserving released document and
runtime contracts and immutable release artifacts. A completed item records its owning checks; proposals are not completion
claims. Builds are coordinated centrally, with filtered checks during iteration and one
integration run per batch rather than one full rebuild per agent.

- [x] Immediate defects: owner callback containment and settlement, overflowing frame
  lengths, creator identity outages classified as retryable.
- [x] Contracts and rendering: extract evaluator contracts and native renderer; update
  packaging, fingerprints, template cache and feature builds.
- [x] Core/storage/runtime/library/capture extractions and explicit dependency direction.
- [x] Typed owner replies, bounded admission, lifecycle states and persistence ownership.
- [x] Structured errors and local poisoned-state recovery policy.
- [x] Layout 2: generation-aware creation, default initialization, clear/recreate/undo,
  row deduplication, saved acceptance and publication fidelity.
- [x] Retention: independent membership, checkpoint epochs, safe deep compaction,
  conditional publication, dirty-stale recovery copies and attachment reclamation.
- [x] Shared document authority handoff: relay-only bytes while a room is active;
  CloudKit catalog/discovery only, then a CAS-published final library snapshot on Stop.
  Catalog format 2, start/stop crash recovery and discoverable recovery copies are
  implemented and locally qualified. Guests keep their existing
  local copy; there is no final-download window. See
  [shared-history epoch maintenance](ideas.md#shared-history-epoch-maintenance).
- [x] Sharing: one codec, shared retry/compaction policy, cancellable native networking,
  control/relay/resource boundaries and testable server decisions.
- [x] Explicit runner providers, additive features and typed jobs/replies. Phone command
  invocation remains deferred; ordinary phone editing and capture remain supported.
- [x] Derived row indexing and pruning; benchmark before adding a ranked sequence.
- [x] State-contract/app-revision identities; no embedded-app updater in this phase.
- [x] Generated adapter contracts, complete build fingerprints and documentation cleanup.
- [x] Test consolidation with unique coverage retained; native/browser/sharing qualification.

Local qualification completed on `rust-foundation-redesign`, based on `ed2e8560`:

- `bun run verify` and `bun run verify --native` passed, including all 521 ordinary
  Rust tests, 189 SDK/shell tests, 102 CLI tests, the Swift integration suites,
  native artifacts, rendering and package checks. All three published compatibility
  corpora replayed. Strict Rust checks cover the workspace, the mobile host feature
  configuration and the relay WASM target.
- `bun run verify browser --parallel=1` passed all 32 browser tests, including real
  Chrome OPFS recovery, interrupted catalog promotion, guest-copy persistence and
  native reopening. Browser qualification used the final Rust and host artifacts.
- The freshly rebuilt sharing host/Worker passed `bun run verify --no-build sharing`
  against local Durable Objects and R2. Frozen historical relocations remain
  byte-identical and permanent history pins are unchanged.
- The 13-scenario replacement benchmark retained independent value oracles; its
  measured results are below. No timing bounds were added to correctness tests.

This is local engineering qualification. The full shipping `release:check`, Linux
matrix, real CloudKit/two-device account transitions and physical iPhone/TestFlight
qualification remain release work. Phone commands and embedded-app updating remain
deferred features.

Every new test must name an observable failure, an independent oracle, the coverage gap
and its owning boundary. Do not add tests for a mechanical move or private call sequence.
Bug regressions run red before the fix. Keep convergence, refusal atomicity, publication
replay and reopen oracles. Transfer unique coverage before removing duplicate randomized
loops; performance measurements belong in benchmarks rather than timing assertions.

New findings belong with their subsystem here or in the architecture/ideas pages. Agents
report evidence and impact before expanding their assigned scope.

### Implementation work packages

The numbered scope below keeps the larger changes reviewable. A source change alone
does not complete its work package; its existing boundary checks must pass after integration.

| # | Change | Acceptance evidence |
|---|---|---|
| 1 | Introduce layout 2 while preserving released layout-1 behavior and relocate historical assets outside obsolete crate directories | Frozen bytes and permanent history pins unchanged; future layouts refused without writes |
| 2 | Extract passive Rust contracts, limits and semantic errors | No Loro, SQLite or execution dependency; generated TypeScript unchanged except intentional API changes |
| 3 | Extract native raster/PDF rendering and its fonts/notices | Existing image/PDF oracles and packaging checks |
| 4 | Extract private-library coordination | Existing install, merge, receipt, identity and cleanup tests at the library boundary |
| 5 | Extract file and SQLite storage | One acceptance path, writer lock and transactional publisher; existing storage refusal/atomicity tests |
| 6 | Extract saved-source capture | Capture reads an accepted saved source; existing role, resource and export tests |
| 7 | Extract owner/runtime orchestration | One edit path; native, browser and closed CLI adapters use the same owner |
| 8 | Encapsulate semantic history exports used by storage | Storage never reaches into `Document` or duplicates Loro acceptance |
| 9 | Select evaluator providers explicitly | Process and in-process capture coexist under unified features; phone commands stay refused |
| 10 | Give evaluator jobs and replies explicit types | Invalid job/reply combinations refused; capture programs retain their ABI contract |
| 11 | Contain listener, completion and sync callback panics | Subsequent edits save and reopen after injected callback panics |
| 12 | Settle queued callbacks during shutdown | Requests queued behind Stop complete once, without applying their mutation |
| 13 | Bound outstanding owner work and retained payload capacity | Immediate Busy before mutation; reserved Flush capacity; permits recycled before callbacks |
| 14 | Make owner lifecycle and persistence ownership explicit | Close/save/retry transitions preserve ownership and accepted state |
| 15 | Preserve structured failures and define poisoned-state recovery | No retry decisions from error text; uncertain state is fenced and recoverable |
| 16 | Merge concurrent container creation | Both merge orders retain independently created children and remain editable |
| 17 | Materialize canonical defaults without conflicting creation writes | Defaults never overwrite another creator's explicit values; deterministic template seeds |
| 18 | Isolate clear/recreate generations | Stale edits stay hidden; fresh recreation and undo/redo preserve application identity |
| 19 | Align stored acceptance and incremental publications with layout 2 | Reopen and independent publication replay equal the semantic value |
| 20 | Find attachment references in visible accepted state | Undo retains blobs during a session; close reclaims hidden references; record-key references survive |
| 21 | Rebuild sanitized copies from accepted visible state | Deleted private payloads absent; merged values beyond authoring limits remain copyable |
| 22 | Separate replication membership from retention policy | CloudKit and relay membership do not imply perpetual history/blob retention |
| 23 | Introduce explicit checkpoint epochs for deep compaction | Fresh accepted state releases hidden roots; ordinary checkpoints preserve identity and undo |
| 24 | Preserve dirty stale replicas before epoch refresh | Recovery copy before replacement; concurrent publication uses a conditional update |
| 25 | Share maintenance eligibility and retry scheduling | Native/browser decisions agree; ordinary 256-save checkpoints never cause epoch swaps |
| 26 | Cancel native network work during shutdown | Stalled HTTP bodies, upgrades, sockets and owner waits cannot delay Stop until network timeouts |
| 27 | Keep typed local owner failures through sharing retries | Definite conflicts, unavailable identity and unknown outcomes choose distinct actions |
| 28 | Separate creator control, byte relay and resource authorization | Guest capabilities and server quotas remain enforced independently of creator sign-in availability |
| 29 | Keep one sharing frame codec | Frozen binary and refusal vectors pass in every consumer |
| 30 | Maintain derived row indexes and prune removed subtrees | Existing reorder/replay oracles; removed generations release their cache entries |
| 31 | Measure row lookup, replacement and publication costs | Opt-in benchmarks with independent value checks; no wall-clock assertions in ordinary tests |
| 32 | Separate state-contract identity from app-revision identity | Schema/theme compatibility independent of cosmetic bundle bytes; updater remains deferred |
| 33 | Generate adapter contracts from Rust | TypeScript and Swift adapters carry the owning Rust types and refusal codes |
| 34 | Fingerprint every local implementation dependency | Engine/app identity and template invalidation include extracted crates and renderer assets |
| 35 | Consolidate tests around unique observable coverage | Preserve convergence, refusal atomicity, publication replay and reopen; remove distribution/count assertions |
| 36 | Qualify the integrated graph and update contracts | Full verifier, native/FFI checks and feature builds once the batch is stable; record deferred platform qualification |

### Row replacement measurements

The opt-in `cargo run --release -p pop-core --features acceptance --example bench_replace`
benchmark checks every result against independently edited JSON outside the timed section.
These local Mac measurements compare the redesigned layout before and after removing
repeated full-order scans from replacement. Each replacement sample starts with a fresh
document; medians exclude the first of six trials. They are observations, not test budgets
or end-to-end UI latency claims.

| Replacement | Rows | Before median | After median |
| --- | ---: | ---: | ---: |
| Append one row | 1,000 | 374.9 ms | 51.6 ms |
| Append one row | 5,000 | 8,767.0 ms | 301.6 ms |
| Reverse | 1,000 | 476.2 ms | 70.7 ms |
| Reverse | 5,000 | 11,488.7 ms | 551.9 ms |
| Remove, reorder and append | 5,000 | 8,787.2 ms | 488.8 ms |
| Set one field by row ID | 5,000 | 0.086 ms | 0.077 ms |

All 13 scenarios retained their independent value checks and sparse publication sizes.
Unchanged/full-field replacements remain proportional to the rows they reconcile. The
operation-local index uses dense tokens and integer positions; moves still shift a vector,
so extreme reorder workloads may justify a ranked sequence later. No such dependency is
needed for this improvement. The separate two-intent page batch over roughly 4 MiB of text
remained around 82 ms and needs its own profile before changing transaction machinery.

Product questions to retain during implementation:

- Private-library snapshots currently cap the whole file at 16 MiB, below the document's
  total attachment allowance. Larger local files remain usable, but library replication
  refuses them. Decide whether this cap is the intended product limit before changing it.
- Theme defaults participate in the state contract because stored overrides are accepted
  relative to them. A future bundle updater needs an explicit palette policy even when
  the authored data descriptor is unchanged.
- Session undo retains causal history while a document stays open. Saved-file limits and
  owner admission budgets do not bound Loro's total memory. Choose any session-undo or
  memory budget after the long-lived memory study; it is a separate product policy.
- Deep compaction intentionally creates an offline cutoff. A stale copy with edits needs
  a recovery document and a clear refresh flow; it cannot silently lose those edits.
- After sharing ends, an already mounted browser frame may need a manual reopen for
  resources it has not loaded yet. The durable local copy and later local opens use its
  saved resources. Do not reload an active page automatically and discard unsent fields.

## Open now

- **App Store release.** Release the iPhone app on the App Store. The Mac app with
  Synced Library, the iPhone app in TestFlight, Files import and Home Screen widgets
  are implemented.
- **Sharing rollout.** Account deletion/Apple revocation, rejoin after re-share,
  guest entry-point rate limits and metrics, frozen v1 HTTP/binary fixtures, universal
  links and Save Unshared Copy are implemented. Privacy disclosures cover relay
  storage and account deletion. Validate the Developer ID profile for associated
  domains and the sharing Keychain group; Mac Apple sign-in uses the web flow,
  without the native Apple sign-in entitlement. Qualify the signed Mac release with
  sign-in, universal links, account deletion and broader device/resource scenarios
  before publication.
  [Architecture](architecture.md#relay) and the [sharing README](../apps/sharing/README.md)
  own the technical description and operating limits.
- **Post-1.1 follow-ups.** Qualify background widget refresh on devices and library
  sync on more accounts, and expand starter coverage.

- An additive authoring environment API exposes native platform/device identity and
  reactive compact/regular viewport layout to Svelte and editor CSS. Browser identity
  stays unknown. `pop dev` device presets preserve accepted edits; static captures
  remain independent. See the [authoring guide](guides/authoring.md#platform-device-and-responsive-layout).

- Native `window.fullscreenable` and the local Chrome browser beta are implemented.
  The same Rust owner/store powers independent OPFS copies, bounded resource reads,
  Save As downloads back to native. Identity, export barriers,
  failed writes and recovery are covered at their owning boundaries. Browser launch
  polish, sustained-edit/near-limit performance qualification and Safari/mobile browser
  support remain deferred. Hosted links use the live sharing service described above. `pop dev` keeps its native owner.
  [Hardening evidence](evidence/browser-host-hardening-2026-10-08.md) records the scope
  and verification limits.
- A text draft whose control is destroyed after an unknown-outcome send is retained in
  the page shell. Native close/quit offers to copy the complete text, discard it or cancel;
  cancelling retains the draft and resumes editing. Recovery after renderer termination
  remains deferred. Qualify the prompt with the next release candidate.
- Qualify the release candidate with the current [release checks](guides/releasing.md).
  Complete-value replacement, template acceptance, command declaration checks,
  dedicated captures and document UUID copy rules are implemented. Historical foundation
  runs do not certify the current candidate.
- Finish the bundled-library license review; completion is not recorded here. App bundles
  retain library license comments; fonts and other non-JS media rely on the review because
  those comments do not cover them.
- Requalify every widget family and size on the release candidate; the earlier
  hands-on reviews predate the two-stage capture ABI.
- Historical hitSlop releases 1.0.0, 1.0.1 and 1.1.0 carry frozen
  [compatibility corpora](testing.md#compatibility-corpus). Keep the permanent 1.0.0
  hash pin and capture one new entry per release, even when markers stay unchanged.
  The candidate must replay every published corpus and its own entry.
- Storage version 2, whenever it comes, ships with: a backup of each file before its first
  migration (a clean copy, `file/copy.rs`); a test that a frozen version-1 file migrated by
  a write equals a newly created version-2 file in layout and value; and a test that an
  interrupted migration leaves the version-1 file intact.
- Runtime ABI 1's command prelude is frozen by the published corpora. Capture records
  its digest, and `scripts/build/runner.ts` refuses to change those released bytes.
- Check Quick Look by hand on a document received by Mail and AirDrop. Quarantined copies,
  a file another process is writing and a crashed write (the document icon until the app
  recovers it) are checked.
- Published attachment, asset and storage limits stay governed by their owning format
  markers. Qualify whole-document Time Machine copies at those limits.
- Exercise the release engines on darwin-arm64, linux-x64 and linux-arm64 (Windows is
  outside the current release targets; a native Windows host is a longer-term direction).
- Loro is pinned to the exact crates.io release 1.17.1, which contains the former
  `c00c9fa` pin and its undo fixes. Qualify this upgrade against every published
  corpus and capture the next entry with the exact candidate writer.
- Check system IME composition and Edit ▸ Undo by hand (typing, ⌘Z inside a field, an
  agent edit between steps); no evidence file covers them.
- The production-window shadow refresh, and an opening-only black strip beneath a window's
  content, which was not reproduced and has no confirmed cause
  ([evidence](evidence/shape-lab-interaction-2026-09-30.md)).
- A long-lived memory study across windows, captures and close/reopen cycles, including
  owner-queue growth and capture concurrency before tuning admission caps. The window
  benchmarks record footprints only and make no leak claim.
- **Older documents on iPhone.** Edge-to-edge editors require safe-area padding and
  a reservation for the floating actions menu. Existing documents keep their compiled
  app and may lack these styles. Qualify released documents on the phone, including
  top and bottom controls, menus and keyboard entry. Record overlap as a known
  TestFlight layout issue; updating starter templates does not fix old files.
  Inaccessible editing controls still need a host fallback before qualification.

## Next

- **Sharing qualification.** The foundation redesign implements cancellable HTTP and
  attachment transfers, typed owner failures, explicit reconnect outcomes, generated
  sharing types and shared maintenance policy. Complete native/browser and release
  qualification before treating those changes as shipped behavior; the work packages
  above track integration evidence.
- **Review the restored examples in the app.** The archived examples now live in
  `pop/` in the current format, with model modules and commands. They have been checked
  in the dev preview; individual native checks do not qualify the entire collection.
  Check each in the built app and PNG/PDF export, and review the bundled selection.
  One example stays archived because its engine is AGPL.
- **Author diagnostics.** Add a read-only [`pop doctor`](ideas.md#pop-doctor) for
  installation and PATH conflicts, CLI/SDK alignment, engine availability and skill links.
  Explain why a rebuilt template does not update an existing document, and suggest
  concrete next steps. Repairs remain explicit actions.
- **Preview asset serving.** `pop dev` serves only the pop folder and the CLI, so
  assets inside packages (fonts, models) must be copied into the pop to preview.
- **Worker and worklet assets.** The shared page CSP is defined in Rust for native and
  browser pages. Explicit worker policy and build support for worker and
  worklet entry points remain: emit them as files in `assets` so `new Worker` and
  `addModule` load local URLs. Keep `blob:` and `data:` code refused and make failures
  understandable. WebKit does not isolate `pop:` pages, so there is no
  `SharedArrayBuffer`.
- **Failure hardening.** Add an aggregate close-time artwork deadline; rendering under
  the writer lock is currently bounded only by per-child watchdogs. Consider a cap on
  concurrent restricted children after measurement. Recovery after renderer termination remains open above.
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
  compiler) matched `svelte-check` on every pop and 14 of 15 injected mistakes, and runs
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
  recovery use the same renderer-death path. Reports include the pop's and build's format
  and runtime markers. Widget location refresh follows renames and same-volume moves
  without replacing cached images.

- Declared `camera` and `microphone` capabilities gate web capture in Mac documents,
  development previews and local browser copies, with OS/browser consent still required.
  Declared `speech` enables on-device Mac transcription of stored recordings; it is
  unavailable in the iPhone app, development previews and browser copies. There is no per-pop permission UI or
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
  even with supplied artwork. `pop dev` still mounts static views inside the editor page.
- Active templates are discovered under `pop/`; `bundled.json` chooses the starters.
  Xcode's embedding phase refreshes `generated/templates/` through the template cache
  before copying selected starters into the app. Prepare changed tooling first; see
  [the build workflow](guides/development.md#build-current-templates-with-the-app).
- Rust owns edit admission, save scheduling, discard, close, command dispatch and the
  live socket. Swift delivers events and provides native UI and rendering.
- The CLI's engine creates, edits and validates documents on macOS and Linux, renders template artwork and exports closed documents itself. The Mac app links
  its core independently.
- The CLI, SDK, contracts, templates and skills ship as one `pop` npm package with
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
- `pop.ts` declares metadata and artwork; `schema.ts` supplies the document,
  `App.svelte` the editor, required `Preview.svelte` and `Icon.svelte`, plus optional `Export.svelte`, `Widget.svelte`
  and `commands.ts` the other roles. Captures use saved-state Svelte SSR and Takumi,
  with restricted children on Mac/Linux and in-process capture on iPhone; there is no
  WebKit capture, automatic fallback or public per-pop override.
  `pop dev` runs the native owner on a disposable copy per preview page.

A historical summary of the ownership decisions and integration findings is retained in
the [pre-launch simplification review](evidence/prelaunch-simplification-2026-10-04.md).

## Later

**Catalog capture fidelity.** The [visual review](evidence/slop-capture-review-2026-10-12.md)
finds blank icons, lost SVG styling and collapsed exports in its historical build. The
current Takumi capture compositions cover the bundled pop, including saved cells and
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

**Menu-bar presentation.** Explore [menu-bar pop](ideas.md#menu-bar-pop) for timers,
players and quick capture, alongside the implemented widgets. Consider creating the
WebView only when its editor is opened. Ownership, saving, quit behavior and any
background work need a lifecycle design before this becomes an authoring option.

**Permissions and device services.** Code-bound grants and per-pop consent remain a
[proposal](ideas.md#permissions-bound-to-the-apps-code), as do additional devices such
as MIDI. [Native integrations and richer media](ideas.md#native-integrations-and-richer-media)
also proposes calendar and reminder access, notifications, Shortcuts/Siri, a "Send to pop"
share extension, packaged worker/audio-worklet support and broader transcription.
Camera, microphone and supported on-device Mac transcription already work; browser
transcription and a newer native speech backend are not implemented. New capability
names require an implementation and owning-boundary tests; the proposed additions are
not authoring options today.

**Hosted templates.** The goal is an app-store-style catalog where people discover,
publish and install community pop. A publishing service may accept validated immutable template
artifacts, expose a generated OpenAPI catalog, and download immutable cached masters.
Creating a document still makes a writable local copy, and bundled and cached masters
work offline. Plan a separate Cloudflare HTTP module with R2 artifacts and catalog
metadata, with Rust acceptance authoritative. Publisher identity, upload limits, package isolation
and checksums, immutable release identity and abuse controls are prerequisites. Any
worker kept in a local `deferred/` archive is unsupported scaffolding. The local app
needs no document server.

**Further collaboration design.** Beyond the [release qualification](#open-now), person-level
permissions, read-only links, browser creator accounts, CLI share/join, reliable offline
web reopening remain later work. Keep qualifying
[container-creation and undo conflicts](reference/document-types.md#storage-layout)
as collaboration grows. Keep the existing Rust owner and relay boundaries described in
[architecture](architecture.md#relay).

**Sync beyond Apple devices.** Persistent document replication uses Cloudflare; personal
references use Firestore. Future native platforms can reuse the Rust document client
and add their own account/catalog adapter. See the [cloud specification](cloud-documents.md).

## Deferred

App Check, a document history UI, schema evolution,
direct editing of loose files in iCloud Drive and other synced folders (the Synced Library covers a person's own devices), and [other native platforms](ideas.md#other-platforms). [Ideas](ideas.md) proposes
pulling additive schema changes and undoing an agent's change forward. Historical source
may be kept in the optional, Git-ignored `deferred/` archive; it is not in fresh clones.
Pre-launch documents are not migrated.

## Persistent cloud documents

Approved replacement of CloudKit with Firestore discovery and persistent Cloudflare
document state is implemented on `cloud-document-sync`, stacked over the Rust foundation
PR. The [cloud document specification](cloud-documents.md) owns identity, permissions,
local/offline behavior, clean cutover and future template boundaries. The document
service and client catalog rules were deployed on 2026-11-04. Physical-device
qualification remains pending; see the [deployment evidence](evidence/cloud-documents-2026-11-04.md).
