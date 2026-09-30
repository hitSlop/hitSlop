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
| Tweak its look | Theme tokens, through the CLI only |
| Ask for a change to its data | An agent you run in a terminal |
| Change the app | Only with its source project; a `.slop` you receive has none |
| Make a new one | `slop init` and an agent |

## Build the slope

### Tweak panel

- **What:** a popover from the hover toolbar with a color or font picker for each declared
  theme token, and Reset.
- **Why:** the first rung without a terminal. Today the styling guide has to say "The Mac
  app currently has no built-in theme picker".
- **Builds on:** the store's `theme` function (`crates/hitslop-core/src/store.rs`)
  already validates and saves get, set and reset. Open windows already apply overrides
  live.

### Ask from the window

- **What:** an "Ask…" field in the toolbar. hitSlop runs the person's agent CLI headless,
  with the document's path and the `hitslop-document` skill packaged in every slop. The
  agent edits through the owner socket, so the person watches its edits land.
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
- **Builds on:** Loro commit messages, and an `UndoManager` scoped by origin, which the
  collaboration plan in [Direction](roadmap.md#later) already expects. `doc.change` takes
  no message today.
- **Contract change:** undo UI is deferred.

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

## Change the app, keep the data

### Remix

- **What:** a package may carry its source as an immutable resource the page never serves.
  "Remix…" unpacks it into a project, writes a `BRIEF.md` saying where it came from, and
  launches the agent.
- **Why:** the essay's in-place toolchain: the tool you hold carries what you need to
  change it. Without it, the slope ends at a cliff.
- **Builds on:** `slop init`'s agent handoff and the build staging in
  `packages/cli/src/build.ts`.
- **Contract change:** packages contain no source today. Shipping source also raises
  questions about licenses and private notes in briefs.

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
  repaired), which already keeps unexpected values safe, and the canonical schema key.
- **Contract change:** schema evolution is deferred, and documents keep the app version
  they were created with. This is the one deferral worth pulling forward.

### Agent notes and prompt buttons

- **What:** authors ship notes for agents beside the generic skill ("to plan a week, add
  rows to `days`…"). The manifest can declare prompt buttons that run through Ask.
- **Why:** teach the agent once, and every copy of the slop benefits.
- **Builds on:** the skill that build embeds in every package (`packages/cli/src/build.ts`).
- **Contract change:** the manifest gains a field.

## Tools, not apps

### Open with another view

- **What:** slops with the same schema key open each other's documents: a checklist as a
  kanban board, or as a printable sheet. Translating between different schemas (lenses, as
  in Ink & Switch's Cambria) can come later, if ever.
- **Why:** data should outlive any one interface.
- **Builds on:** the exact canonical schema key
  ([runtime reference](reference/runtime.md#schema-identity)).
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

- **One-click Share.** Save, package and hand the slop to the share sheet or AirDrop,
  instead of "close it, then zip it".
- **Menu-bar slops and widgets.** A `menubar` presentation for timers and players, and
  WidgetKit widgets rendered from the icon or export capture of saved state.
- **History scrubber.** Checkpoints keep full history, so a timeline can show the document
  at any version. Restore applies an old version as a new edit.
- **Household sharing.** Exchange Loro updates peer to peer, or through a relay that stores
  opaque bytes: a family's grocery list as home-cooked software. **Contract change:**
  collaboration is deferred, and the prerequisites in [Direction](roadmap.md#later)
  still apply.

## What we won't take

The manifesto also argues for cloud sandboxes, organization-wide governance and routing
between model providers. Those serve an enterprise agent platform. hitSlop takes the
principles (agents next to the work, visible authorship, nothing starts from scratch) and
stays local, with no account.
