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
- **Builds on:** `installedAgents()` in `packages/cli/src/agents.ts` (Codex, Claude Code,
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
- **Builds on:** the owner socket and its TypeBox envelopes in `packages/schema`. The
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
  `packages/cli/src/build.ts`.
- **Contract change:** templates contain no source today. Shipping source also raises
  questions about licenses and private notes in briefs.
- **Constraints:** compressed and optional, with a size limit and provenance; source
  files and dependency metadata only, never dependencies, build caches, credentials or
  chat history; extracted without executing code and never served to the page.
  Rebuilding produces a new app.

### Additive app upgrades

- **What:** let a document move to a newer build of its app when the new descriptor only
  adds things:
  - a field with an initial value;
  - an optional field;
  - an enum value;
  - wider bounds.

  Rust checks that the change is additive, and the document keeps its Loro state.
- **Why:** remixing is a dead end if every schema change strands existing data.
- **Builds on:** preserve-and-flag (merged anomalies are kept and reported, never
  repaired), which already keeps unexpected values safe; saved state stored with its
  app row, so replacing the row compares the old and new parsed descriptors in the same
  commit; and the [compatibility](engineering-contract.md#compatibility) markers and
  corpus.
- **Contract change:** schema evolution is deferred, and documents keep the app version
  they were created with. This is the one deferral worth pulling forward.

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

- **What:** a Mac and an iPhone editing one document are a person collaborating with
  themselves, so one path serves devices and people: each device keeps its own `.slop`
  file as a replica, and replicas exchange Loro updates computed from version vectors,
  through the room described next.
- **Why:** a slop made on the Mac should be in your pocket, without a second engine.
- **Builds on:** the single file (the app travels with the data) and the owner's Loro
  update import and export. Row-syncing SQLite services (Turso, SQLite Sync) were
  considered and rejected: they replicate rows, while the document's merges live in Loro.
- **Remaining work:** theme overrides already merge in Loro, last writer per color.
  Attachments travel separately, by hash, so "edits synced" and "file complete" are
  different states. Sync also needs retention and undo policies for offline replicas.

### Realtime collaboration on Durable Objects

- **What:** a shared document is a room, one Cloudflare Durable Object per share, reached
  over a hibernating WebSocket. Each Mac keeps its own replica, writer lock and SQLite
  file. The room keeps an update log plus a compacted snapshot, within the same caps as
  `StorageLimits`, and relays updates between replicas. It is pinned to the document's
  descriptor and template.
- **Protocol:**
  - On connect, the replica and the room exchange version vectors, and each sends what
    the other lacks (`export(updates(vv))`). Live local updates follow.
  - Import is idempotent. Each replica records the last version the room acknowledged
    in its SQLite file, so a reconnect resends by diff and needs no outbox.
  - Presence (cursors, who's here) travels as Loro `EphemeralStore` frames: timestamped,
    last-writer-wins keys that expire. The room relays them and never stores them.
  - Check Loro's own sync tooling before writing new framing.
- **Placement:**
  - The Rust core owns the sync messages and the acknowledged version (a `sync` feature).
  - A future Swift sync adapter could handle remote networking and authentication,
    carrying frames as opaque bytes. The Rust owner would schedule imports and saving;
    this is separate from the local command socket it already owns.
  - Remote frames enter the owner queue as `import`. The page sees them as publications,
    exactly like a CLI edit, and text bindings already merge concurrent edits from the
    history they saw.
- **The room is not an authority.** It may run hitslop-core compiled to WASM to check
  decoding, sizes and history-trimmed bytes, and to compact the log. Replicas keep
  preserve-and-flag for merged anomalies.
- **Prerequisites:**
  - the invitee has the same app, which a shared document carries;
  - capability links until accounts exist;
  - rate and connection limits;
  - history retention that works with offline replicas: a replica merges only updates
    made after the other's trimmed start, and closing trims to the last session at most.
- **SDK additions (additive):**
  - `presence`;
  - selective undo that preserves remote changes; today's raw replica imports clear
    undo/redo history, so they cannot be silently rolled back;
  - `change(fn, { message })` for attribution;
  - base versions on index-addressed scalar-list writes, so a remote insert cannot shift
    a `set(index)`.
- **Per-person state.** Today, view state that should survive a reopen lives in the shared
  document: Slide Deck's `activeSlideIndex`; volume and mute in Alien Radio, Metronome and
  Pocket Pod; Pocket Pod's now-playing, repeat and shuffle; Pixel Art's `selectedColor`;
  Morning Pages' `currentKey`; Wordle's `mode`. Under collaboration these would sync
  between people. Add `s.local(node)` for top-level fields: the same handles, snapshot and
  CLI paths, stored in a second per-replica Loro document in its own SQLite table that is
  never synced. Marking a field local changes how it replicates, not its data, so it can
  ship as a compatible upgrade of those templates.

  | Tier | Survives reopen | Synced | Seen by export and Quick Look | For |
  | --- | --- | --- | --- | --- |
  | Svelte `$state` | no | no | only in-page captures | selection, hover, drags, menus, tabs |
  | `s.local(...)` | yes | no | yes | a person's position and preferences |
  | Field | yes | yes | yes | the document's content |
  | Presence | no | yes | no | cursors, who's here |

- **Contract change:**
  - collaboration is deferred;
  - Swift carrying sync frames relaxes "Loro bytes never reach Swift";
  - `s.local` is a new descriptor kind, which lands in Rust, the SDK and a fixture together.

## What we won't take

The manifesto also argues for cloud sandboxes, organization-wide governance and routing
between model providers. Those serve an enterprise agent platform. hitSlop takes the
principles (agents next to the work, visible authorship, nothing starts from scratch) and
stays local, with no account.
