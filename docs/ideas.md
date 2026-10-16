# Ideas

These are proposals, not commitments. [Direction](roadmap.md) holds the order we intend;
this page holds the reasoning. **Contract change** marks an idea that would change the
[engineering contract](engineering-contract.md) and needs a decision before work starts.

## The thesis

Three things we read point the same way:

- **Ink & Switch, [*Malleable Software*](https://www.inkandswitch.com/essay/malleable-software/)
  (2025).** AI code generation alone doesn't give people agency over their software. It
  needs an environment where generated tools get persistence, sharing and composition for
  free ("a kitchen, not a food court"), and a *gentle slope* from using a tool to changing
  it, without cliffs.
- **The Multiplayer AI manifesto (2026).** Agents belong next to the work, not in a
  separate chat. Nobody should copy and paste between them, and anyone should be able to
  see what an agent did.
- **The Hacker News discussion of the essay.** Most people will never climb a steep slope.
  They trust tools that keep their data somewhere they can take it.

hitSlop already has the environment. A slop gets merging storage, saving and recovery,
export, Finder icons and theming without its author doing anything. The owner socket lets
a person and their agent edit the same live document. What's missing is the slope:

| Rung | Today |
| --- | --- |
| Use a slop | The window |
| Tweak its look | The theme panel in the window, and shared theme files |
| Ask for a change to its data | An agent you run in a terminal |
| Change the app | Only with its source project; a `.slop` you receive has none |
| Make a new one | `slop init` and an agent |

## Build the slope

### Ask from the window

- **What:** an "Ask…" field in the toolbar. hitSlop runs the person's agent CLI headless,
  with the document's path and the `hitslop-document` skill the CLI installs. The agent
  edits through the owner socket, so the person watches its edits land.
- **Why:** puts the agent next to the work for people who never open a terminal.
- **Builds on:** `installedAgents()` in `packages/hitslop/src/cli/agents.ts` (Codex, Claude Code,
  Gemini CLI, OpenCode) and the one edit path. The agent is just another CLI client.
- **Open questions:** permissions when an app launches an agent, choosing the agent, and
  showing its cost.

### Attribution and "Undo that"

- **What:** record who made each change (the window, the CLI, or a named agent), plus an
  optional message, as Loro commit metadata. The window then shows "Claude edited 3 rows ·
  Undo", and Undo reverts that origin's change.
- **Why:** the manifesto's "who changed what", at the size of one document. Trust is what
  lets people hand an agent their things.
- **Builds on:** Loro commit messages. Agent commits already carry the message `agent`,
  and Edit ▸ Undo already reverts an agent's edits made while the document is open. What
  remains is naming the agent, an optional message (`doc.change` takes none today),
  showing them in the window, and undoing an agent's edits made while it was closed,
  which needs the kept history to reach them.

### `slop watch` and `slop mcp`

- **What:**
  - `watch` streams the owner's publications as JSON lines, so an agent can react to edits
    made in the window.
  - `mcp` exposes open and recent documents as MCP resources, and get, apply, batch,
    export and theme as tools whose input schemas come from each document's descriptor.
- **Why:** today an agent learns about a person's edits only by polling `get`. MCP lets
  clients without a shell work on slops.
- **Builds on:** the owner socket and its serde types in `crates/hitslop-core/src/wire`. The
  socket takes one request per connection today, so streaming needs a new CLI protocol
  with both sides updated, rather than an additive method in the current protocol.

### Gradient theme tokens

- **What:** a template declares gradients beside its colors, and the theme panel's picker
  gains a gradient mode: a few color stops and an angle.
- **Why:** a background carries much of a slop's look, and one color can't give it the
  soft, layered backgrounds people know from Arc.
- **Builds on:** the palette and the panel's picker. A gradient can't be the value of a
  color token: templates use `--slop-*` colors in `color`, `border-color` and
  `color-mix()`, where a gradient isn't valid CSS. It needs its own declared kind, usable
  only in `background-image`. That kind is checked in `theme.rs`, typed in `defineTheme`,
  carried in theme files and covered by a fixture.
- **Open questions:** whether a stop can name a palette color, so the gradient follows a
  changed accent, and how the window shape, icon and export use a gradient.

## Change the app, keep the data

### Remix

- **What:** a template may carry its source as an immutable part of the file the page
  never serves. "Remix…" writes it out as a project, with a `BRIEF.md` saying where it
  came from, and launches the agent.
- **Why:** the essay's in-place toolchain: the tool you hold carries what you need to
  change it. Without it, the slope ends at a cliff.
- **Builds on:** `slop init`'s agent handoff and the build staging in
  `packages/hitslop/src/cli/build.ts`.
- **Contract change:** templates contain no source today. Shipping source as a new asset class
  raises `packageFormat`: the asset-key set is closed and has no reserved private prefix.
  It also raises questions about licenses and private notes in briefs.
- **Constraints:** compressed and optional, with a size limit and provenance; source
  files and dependency metadata only, never dependencies, build caches, credentials or
  chat history; extracted without executing code and never served to the page.
  Rebuilding produces a new app.

### Additive app upgrades

- **What:** let a document move to a newer build of its app when the new app keeps an
  equivalent descriptor (new UI code only) or its descriptor only adds things:
  - a field with an explicit initial value for existing data;
  - an optional field;
  - an enum value;
  - wider bounds.

  Rust checks that the change is additive, keeps existing values and `$id`s, and commits
  the new app, descriptor and transformed state in one transaction, so a reopen after a
  failure finds the complete old version or the complete new one. Renames, deletions and
  type changes come later, each with explicit transformation rules; they are never
  guessed.
- **Why:** remixing is a dead end if every schema change strands existing data.
- **Builds on:** saved state stored with its app row, so replacing the row compares the
  old and new parsed descriptors in the same commit; and the
  [compatibility](engineering-contract.md#compatibility) markers and corpus.
- **Storage:** the `app` row and `assets` are sealed by triggers. The upgrade is a write
  under the writer lock that drops the seal, replaces the app and assets, and recreates
  the identical triggers in the same transaction (the exact layout check then passes);
  keep the triggers rather than weakening them for this.
- **Identity:** safety comes from the descriptor comparison, so an explicit "move this
  document to that template" needs no lineage. Offering "a newer version of this app
  exists" does, and neither a slug nor an SDK version provides it. The exact revision is
  a digest of the app (its `app` row and assets) that Rust computes; lineage comes with
  publishing and signing.
- **Contract change:** schema evolution is deferred, and documents keep the app version
  they were created with. An upgrade is always an explicit operation: installing hitSlop
  never replaces the app inside an existing document. This is the one deferral worth
  pulling forward.

### Permissions bound to the app's code

- **Today:** apps declare camera, microphone and on-device speech use; the host gates
  requests and macOS consent belongs to hitSlop. There is no per-slop decision or
  code-bound permission store. See [capabilities](reference/runtime.md#security-boundaries).
- **What:** grant a permission (microphone, camera, MIDI) to one app's code: the hash of
  its `app` row and assets, which in a single file are all the code there is. A remix
  or an upgrade asks again.
- **Why:** powerful slops need device access without trusting every later version.
- **Builds on:** the single file, and the page policy that already refuses code from
  outside it.
- **Contract change:** a permission store outside the document, keyed by code hash.

### Agent notes and prompt buttons

- **What:** authors ship notes for agents beside the generic skill ("to plan a week, add
  rows to `days`…"). The manifest can declare prompt buttons that run through Ask.
- **Why:** teach the agent once, and every copy of the slop benefits.
- **Builds on:** the file's `app` row, which `slop inspect` and `slop schema` already read
  for agents.
- **Contract change:** optional notes and prompt declarations can be added as members of
  `definition_json` without a marker, because readers ignore unknown members. Anything
  an older host must understand still raises its owning marker.

## Tools, not apps

### Open with another view

- **What:** slops with the same descriptor open each other's documents: a checklist as a
  kanban board, or as a printable sheet. Translating between different schemas (lenses, as
  in Ink & Switch's Cambria) can come later, if ever.
- **Why:** data should outlive any one interface.
- **Builds on:** the descriptor in each document's app row
  ([runtime reference](reference/runtime.md#schema-identity)), which the core parses, so
  two apps' descriptors compare by meaning.
- **Contract change:** a document bundles its app today; the app would be chosen when the
  document opens.

### Shortcuts and a share sheet

- **What:** App Intents for get, apply and export, plus intents derived from the
  descriptor: a list of rows becomes "Add to Grocery List". A "Send to slop" share
  extension.
- **Why:** the no-code rung, on the Mac's own automation, with Siri and Spotlight for free.
- **Builds on:** closed editing, which runs the owner without WebKit or authored code.

### Native integrations and richer media

These are future proposals, not supported capability declarations. Camera and microphone
capture and on-device transcription of saved recordings already work on a supported Mac;
the [capability guide](../apps/landing/src/content/docs/docs/guides/capabilities.mdx)
describes today's APIs and platform limits.

- **Calendars and reminders:** let a planner work with your calendar, or a grocery slop
  add a reminder, with explicit permission for the access it needs.
- **Notifications:** let timers, habits and plans notify you at the right time, with
  controls for which slops may send them.
- **Shortcuts, Siri and sharing:** the [App Intents and share extension proposal](#shortcuts-and-a-share-sheet)
  makes actions such as "Add to Grocery List" and "Send to slop" available outside its window.
- **MIDI:** connect music tools to instruments and controllers after designing device
  declarations and permissions.
- **Workers and audio worklets:** package local entry points for heavier background
  processing and richer audio libraries. Keep the existing refusal of blob/data executable
  code; this needs explicit build and host support.
- **Broader transcription:** support transcription in browser copies and investigate a
  newer native speech backend, while retaining a useful path when unavailable.
- **Permissions for each slop:** build on [code-bound grants](#permissions-bound-to-the-apps-code),
  so a person chooses which app can use an integration and reviews that choice after
  a remix or upgrade.

Each integration needs its own platform support, consent and lifecycle design before an
API is offered. Calendar/reminder synchronization rules, notification scheduling and
background execution remain design work; this proposal sets no new API names or delivery order.

### Query a document with SQL

- **What:** `slop query DOC 'SELECT …'`, read-only SQL over a snapshot. The engine
  registers virtual tables generated from the descriptor (a list of `$id` rows becomes a
  table, as sqlelf does over ELF), computes them from Loro and stores nothing. Today the
  same works without new code: `slop get DOC > doc.json`, then
  `sqlite3 :memory: "SELECT … FROM json_each(CAST(readfile('doc.json') AS TEXT), '$.items')"`.
- **Why:** Farid Zakaria's [SELF](https://fzakaria.com/2026/08/23/your-executable-is-a-sqlite-database)
  posts, which inspired the `.slop` file, show how much tooling collapses into SQL once the
  program and its state are one database. A `.slop` already answers `SELECT` for its app,
  assets and artwork, but not for its saved values, which are Loro bytes.
- **Builds on:** `get`'s snapshot. A stored SQL copy of the data would be a mirror, so
  the tables exist only for the query.
- **Open questions:** table naming freezes into a public boundary at launch, so wait for
  a request a JSON read can't answer.
- **Nearby:** a semantic `slop diff` of two files' apps and data, which pairs with
  [additive app upgrades](#additive-app-upgrades) and starts from `inspect`'s
  `appDigest`; a Spotlight importer that indexes saved text through the core, as Quick
  Look reads artwork; and registering the `HSLP` application ID in SQLite's `magic.txt`
  so `file` names a `.slop`.

### Export data

- **What:** "Export data…" as JSON, CSV or Markdown, next to PNG and PDF.
- **Why:** people trust tools whose data they can take elsewhere.
- **Builds on:** `get` already prints the document as JSON.

## On the desktop, and with other people

- **Menu-bar slops and widgets.** A `menubar` presentation for timers and players, and
  WidgetKit widgets rendered from the icon or export capture of saved state.
- **History scrubber.** A timeline of whatever history remains under the
  [current retention policy](architecture.md#saving). Saved history can be pruned while
  open or on close; reopening does not guarantee a retained timeline, and undo starts empty.
  Restore applies an available old version as a new edit. A guaranteed longer timeline
  would need a retention rule that bounds the cost of deleted content, which Loro keeps
  in the starting state of any cut before the latest version.
- **Household sharing.** A family's grocery list as home-cooked software, through the
  room described next.

### Other platforms

The Swift host stays the Mac host. Windows, iOS, Android and Linux each get a thin host
around the same Rust core and page shell. A host never owns document semantics, so a new
one is a window, a WebView, one bridge and a lifecycle, not a second engine. Nothing here is
scheduled; the [roadmap](roadmap.md#next) lists the prerequisites.

**What every host owns, and nothing else.**
- The window: shape, hit testing, pin level, fullscreen, toolbar.
- `slop:` resources, served from the document through the core's resource reader with the
  production CSP and headers.
- One bridge channel. A message is admitted by the native sender (main frame and security
  origin), never by a URL the page can reproduce. Swift does this in
  `DocumentSession.swift`'s `didReceive` handler.
- The session: the push queue, the host-to-page deadlines, the renderer-death path, flush,
  `prepareClose` and `cancelClose`.
- Failure and save-failure screens, quit and close handoff, the catalog, updates and telemetry.

When a second host exists, this list should move into the [engineering
contract](engineering-contract.md) as the rule for hosts.

| Platform | Host | Notes |
|---|---|---|
| iOS | Native Swift: SwiftUI and `WKWebView`, reusing the Apple packages through UniFFI | WidgetKit, Quick Look and share extensions are Swift on Apple platforms anyway. No child processes, so the restricted evaluator needs an in-process form. |
| Android | Native Kotlin: Activity and Android WebView, the core through UniFFI's Kotlin bindings | The WebView message-listener API reports the sender's origin and whether it is the main frame, which would give native admission. Not yet verified here. Same in-process evaluator need as iOS. |
| Windows and Linux | One Rust desktop host on `tao` and `wry`, with the bridge admission and window-shape code owned by per-OS adapter modules | One codebase for two systems is the only case where `wry` pays for itself. Windows may use `webview2-com` directly; a Windows spike decides. |

`wry` is not a fit for mobile, where it still needs Swift or Kotlin glue. Tauri is the same
`wry` underneath, with a command layer the core already replaces. Electron has no per-pixel
click-through for shaped windows. Qt WebEngine is heavy and LGPL. CEF through `cef-rs` with
offscreen rendering would give exact shapes and frame identity everywhere, and is the
fallback if a platform cannot give native frame identity; it is a large dependency with
young Rust bindings.

**Before any of them.** The core runs on Unix today: the owner socket, the writer registry,
exclusive rename publication and the child-process evaluator. A Windows host first needs the
platform-driver seam from the roadmap and a port of those drivers. iOS and Android need the
evaluator to run in process.

**What the [macOS `wry` spike](evidence/wry-host-spike-2026-10-16.md) found.**
- Stock `wry` hands the page's IPC handler only the frame's URL, not `isMainFrame` or the
  security origin. A same-URL sandboxed iframe was indistinguishable from the main frame.
  A native message handler registered beside `wry`'s, about 90 lines on public WebKit API,
  restored Swift's admission, and `wry`'s own `ipc` handler must never carry the document bridge.
- `wry` makes a window transparent through a private WebKit key, so assume no Mac App Store.
- Cursor polling makes clicks on transparent pixels reach the window behind, at about 9 ms
  p95 with a small race at the edge. Swift's `hitTest` override, ported to the spike, was
  reported by hand not to pass clicks through on macOS 26 (not instrumented); whether the
  Swift host itself does is unchecked. Click-through is not required for a first host.
- A real third-party slop opened, rendered, saved an edit and reopened with every asset served
  from the document. That needs the host to link the core; the preview-owner protocol
  cannot serve app assets.

**Open risks, not yet tested.** Native frame identity on Linux (the `wry` documentation says
the IPC URL does not identify the iframe on Linux and Android), WebView2's synthetic
origin and its effect on the CSP, WebView2 frame handling, and mixed-DPI hit testing on
Windows.

### Sync between your own devices

A Mac and an iPhone editing one document can use the same room model as people
collaborating. Each keeps a `.slop` replica and its local writer lease, edits offline and
merges through the relay when it reconnects. Replicate Loro updates, not SQL rows or a
second JSON document. Attachments
travel separately by hash, so installed data and a complete exportable file are distinct
states until its blobs arrive.

### Realtime collaboration on Durable Objects

The selected direction is local owners and a byte relay;
[architecture](architecture.md#sync) describes it, and the [roadmap](roadmap.md#later)
tracks what remains. What stays open:

- **Presence:** cursors and who is here, probably through Loro's `EphemeralStore`, never
  saved.

Per-person persistent preferences remain a separate `s.local(node)` proposal. Today,
keep temporary view state in Svelte and document content in fields. A new local-field
kind would need Rust, SDK, fixtures, export semantics and a compatibility decision;
it is not implied by the relay.

The restored slops supply useful cases: Slide Deck's selected slide; volume
and mute in Alien Radio, Metronome and Pocket Pod; Pocket Pod's playback and repeat;
Pixel Art's selected color; Morning Pages' current key; and Wordle's mode. Decide each
field's intended scope before introducing locality.

| State | Survives reopen | Shared | Fresh export sees it |
| --- | --- | --- | --- |
| Svelte `$state` | No | No | Default local state only |
| Proposed `s.local(...)` | Yes | No | Policy to qualify |
| Document field | Yes | Yes | Yes |
| Presence | No | Yes | No |

Potential SDK additions remain presence, actor-aware attribution and selective undo.
Index-addressed scalar-list edits need an explicit stale-base rule so another person's
insertion cannot silently redirect an edit. These are separate design tasks; the relay
does not freeze their API or promise that every future addition is compatible.

## What we won't take

The manifesto also argues for cloud sandboxes, organization-wide governance and routing
between model providers. Those serve an enterprise agent platform. hitSlop takes the
principles (agents next to the work, visible authorship, nothing starts from scratch) and
stays local, with no account.

## Possible command extensions after launch

Commands currently serve agents and take flat scalar arguments. Page command calls
could add a context method behind a new runtime ABI; `s.row(list)` could add a checked
argument kind, and nested arguments could widen package-format acceptance. None is
required for launch. Pages share plain functions over `Scope` with commands today.
