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
  socket takes one request per connection today, so streaming needs a new envelope.

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
- **Contract change:** templates contain no source today. Shipping source also raises
  questions about licenses and private notes in briefs.
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
- **Contract change:** the `app` row gains a column, which raises `packageFormat`.

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

### Export data

- **What:** "Export data…" as JSON, CSV or Markdown, next to PNG and PDF.
- **Why:** people trust tools whose data they can take elsewhere.
- **Builds on:** `get` already prints the document as JSON.

## On the desktop, and with other people

- **Menu-bar slops and widgets.** A `menubar` presentation for timers and players, and
  WidgetKit widgets rendered from the icon or export capture of saved state.
- **History scrubber.** A timeline of the history a document keeps: all of it below
  4 MiB, otherwise only the open session's. Restore applies an old version as a
  new edit. A longer timeline needs a retention rule that bounds the cost of deleted
  content, which Loro keeps in the starting state of any cut before the latest version.
- **Household sharing.** A family's grocery list as home-cooked software, through the
  room described next.

### Sync between your own devices

A Mac and an iPhone editing one document can use the same room model as people
collaborating. Each keeps a `.slop` replica and its local writer lease. Shared edits
require a connection to the authority; ordinary unshared documents continue offline.
Replicate accepted Loro updates, not SQL rows or a second JSON document. Attachments
travel separately by hash, so installed data and a complete exportable file are distinct
states until its blobs arrive.

### Realtime collaboration on Durable Objects

The selected direction (2026-10-07) is **one semantic writer per room**. It supersedes
the earlier raw-update relay and independently writable replicas. A Durable Object is
one possible future host; a native loopback proof must qualify the model first.

- **Authority:** the existing Rust owner validates and orders the same batches and named
  commands as local documents. Commands evaluate against authority state; text uses its
  existing version-aware merge. Clients do not author disconnected shared changes.
- **Replicas:** each Rust owner keeps its file lock, installs validated accepted updates,
  publishes immutable snapshots and persists them normally. Loro bytes stay in Rust.
  Disconnection fences all shared mutation paths while preserving unsent drafts.
- **Identity:** one document UUID and matching app digest/layout in the handshake.
  Explicit Duplicate is independent and gets a fresh UUID. Bootstrap preserves the room
  document identity. UUIDs are never Loro peer IDs.
- **Transport:** a separate sync envelope around the existing mutation vocabulary,
  with bounded requests, updates and snapshot transfer. It never borrows the command
  protocol's number. Loro's framing and adapters may be useful for the hosted transport;
  a raw update relay does not implement command authority.
- **Local proof:** two editable development pages and CLI requests through one room,
  accepted-update equality, ordered publications, reconnect/snapshot fallback and
  identity refusals. No attachment imports or shared undo in this first proof. It
  changes no production endpoint and writes no sync metadata.
- **Durability track:** storage 2 adds atomic request receipts, client pending requests,
  durable replica progress and epochs. Reuse the same request ID after an uncertain
  reply; a changed payload under that ID refuses. Expired receipts do not prove an
  operation was never accepted. Resolve pending outcomes before replacing replica state.
- **Retention:** an authority can publish a new retained checkpoint and fence old bases.
  Slow clients resync rather than merge offline edits; stale text bases preserve drafts
  for recovery. Attachment transfer/reclamation and interrupted snapshot installation
  need explicit tests before production sharing.
- **Undo and presence:** whole-document `revert_to` is not personal undo. Authority
  peers and temporary text peers need actor mapping before selective shared undo.
  `EphemeralStore` remains a candidate for nondurable cursors and presence.
- **Hosting gates:** qualify native-owner-to-WASM portability, restricted commands,
  authentication, limits, Cloudflare execution and personal undo separately. Neither
  the local proof nor schema identity proves these work.

Per-person persistent preferences remain a separate `s.local(node)` proposal. Today,
keep temporary view state in Svelte and document content in fields. A new local-field
kind would need Rust, SDK, fixtures, export semantics and a compatibility decision;
it is not implied by the authority model.

The archived examples still supply useful cases: Slide Deck's selected slide; volume
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
insertion cannot silently redirect an edit. These are separate design tasks; the local
proof does not freeze their API or promise that every future addition is compatible.

## What we won't take

The manifesto also argues for cloud sandboxes, organization-wide governance and routing
between model providers. Those serve an enterprise agent platform. hitSlop takes the
principles (agents next to the work, visible authorship, nothing starts from scratch) and
stays local, with no account.
