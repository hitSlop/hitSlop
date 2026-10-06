# Slop commands

Status: spike built and tested, 2026-10-06. The decision to adopt waits on the
[agent test](#the-agent-test). The spike's code is uncommitted on branch `spike/commands`,
in the git worktree `../hitslop-spike-commands` (kept apart from this tree, which another
session was editing).

## The idea

Today an agent or the CLI edits a slop with path operations (`set`, `insert`, `move`…)
and has to work out what the fields mean and how the app would make a change. The idea,
borrowed from Convex:

- **The schema says what a slop stores.**
- **Commands say what an outside actor can do with it.** Each is a named, described
  action with typed arguments, written by the author: `addTask`, `fileFinished`,
  `turnFor`.
- **`describe` is what machines read.** It is one view of the slop: what it is, what each
  field means, its commands, its current value.
- **Loro stays underneath.** Nobody outside the core sees it.

An agent asked to "set the hourglass for 25 minutes" runs `turnFor {minutes: 25}`. It
doesn't work out that `start` and `end` are epoch milliseconds and set both. The app's
own buttons call the same commands, so a person and their agent do things the same way.

## What agents see today

The concern behind this plan was that agents must understand Loro. They don't, and
authors don't write TypeBox:

- Authors declare fields with the `s` builder in `schema.ts`. Rust parses that descriptor
  and maps it to Loro layout 1. TypeBox only defines the wire between the CLI, the app
  and the page.
- Agents send JSON operations over paths of field names, `{id}` rows and `{index}`
  elements. The `hitslop-document` skill teaches them in about 50 lines.
- Three CRDT details leak through: row `$id`s (agents handle these fine), the
  `--base VERSION` step for rewriting text, and kind-specific refusals (`exists`,
  increment vs set).

The real gaps are **meaning** and **verbs**. Descriptors carry no descriptions, so an
agent guesses what `start` holds. There are no domain actions: Quick Checklist's "file
finished" lives only in `App.svelte`, and an agent has to rebuild it as a hand-made
batch.

## First principles

- **Who changes a document, and what each needs:**
  - **The person, in the window:** fine-grained, frequent edits; merging text and
    previews. Bindings and handles serve them well, and commands are not for keystrokes.
  - **An agent:** to know what the slop is and what fields mean, safe domain actions,
    and a general fallback for edits nobody anticipated ("fix the typo in task 2").
  - **Automation (Shortcuts, Siri, App Intents):** typed actions with no reasoning in
    between. Only author-declared commands serve it.
  - **Other replicas (later):** Loro updates, not commands.
- **Commands are macros, not a validity boundary.** Convex is one serializable database.
  hitSlop is heading to CRDT merges, where two commands that each keep an invariant can
  merge into a state that breaks it. The app must render any descriptor-valid state, and
  the descriptor and the core stay the validity boundary. Commands carry intent and
  ergonomics.
- **Command code is third-party code.** Slops arrive from other people. Today their code
  runs only in WebKit's sandboxed web-content process. The Mac app itself is unsandboxed
  (its entitlements file is empty), so command code must never run in its process.
- **The general operations stay.** They make every slop editable by an agent without the
  author doing anything (the thesis in [ideas](../docs/ideas.md)), and they cover edits no
  author foresaw. Limiting a person's own agent to the author's verbs works against
  malleable software. Commands come first; `batch` remains the general tool.

## Decisions

| Proposal | Decision | Why |
|---|---|---|
| Schema = storage, commands = behavior, describe = machine contract | Adopt the framing | Verbs and meaning for agents and automation, readable names for undo and attribution, and a tool list for MCP and Shortcuts |
| Commands replace path operations | Reject | See "The general operations stay" above |
| Immer/Proxy drafts (`draft.tasks.push(...)`) | Reject | Diffing a draft loses intent: `count++` becomes a set and drops concurrent increments, `sort()` is ambiguous between moves and a replace, and `title = x` replaces the whole text. The existing `change(tx => …)` handles already map one-to-one to intents, so commands take `tx` |
| Run commands in Bun or Node | Reject | A received slop's code would get full file and network access; `node:vm` isn't a sandbox |
| Run commands in the app's process | Reject | Unsandboxed process; an engine bug would compromise the whole app |
| Run commands in `slop-engine` with QuickJS | Adopt | A separate executable that already ships for macOS and Linux. QuickJS beat Boa on speed, memory and size ([results](#spike-results)) |
| TypeBox for command arguments, plus a JSON Schema validator in Rust | Reject | Arguments are `s.*` fields, checked by the core's existing value rules. Authors keep one vocabulary and each rule has one owner |
| One MCP server per slop | Reject | One generic `slop mcp` with the document as an argument, added later as a thin layer over `describe`, `get`, `call` and `batch` |
| Generated typed clients | Not needed | TypeScript inference types `doc.commands.X()`; the file carries command metadata for `describe` |
| Move the schema into `defineSlop` | Reject | `slop.ts` is build-only and the build refuses it in `app.js`; `schema.ts` is what the app, the build and the runner all load |
| `query()` | Not in v1 | Same runner without `tx`; add it only if the agent test shows agents need it |
| `effect()`, per-command AI permissions | Defer | Effects need ambient authority, which the runner must not have |

## Design

### Authoring

Files are split by who loads them:

| File | Loaded by | Holds |
|---|---|---|
| `slop.ts` | the build only | Packaging: manifest, window, theme defaults, initial values |
| `schema.ts` | the build, the app and the command runner | The document: fields, their descriptions, its commands |
| `*.svelte`, `model.ts` | the app | The view |

```ts
// examples/slops/hourglass/schema.ts
export const hourglass = defineDocument({
  title: s.text({ description: "What the glass counts down to" }),
  start: s.number({ min: 0, description: "When the glass was turned over, in epoch milliseconds; 0 until the first turn" }),
  end: s.number({ min: 0, description: "When the sand runs out, in epoch milliseconds; 0 until the first turn" }),
})
  .command("turnFor", {
    description: "Turn the glass over so its sand runs out after the given minutes",
    args: { minutes: s.number({ min: 1, max: 525_600 }) },
    run({ tx, now }, { minutes }) {
      tx.fields.start.set(now);
      tx.fields.end.set(now + Math.round(minutes * 60_000));
    },
  })
  .command("turnAgain", {
    description: "Turn the glass over again for the same length of time as last time",
    run({ current, tx, now }) {
      if (!current.end || current.end <= current.start) throw new Error("The glass has never been turned");
      tx.fields.start.set(now);
      tx.fields.end.set(now + (current.end - current.start));
    },
  });
```

The app calls `await doc.commands.turnAgain()`. Bindings, `.value`, previews and
`bindText` are unchanged.

Rules:
- **Descriptions.** Every `s.*` node takes an optional `description` (1–500 characters).
  The core checks descriptions and keeps them in the stored descriptor, but leaves them
  out of the parsed one, so two descriptors that differ only in descriptions are the
  same schema.
- **Commands are chained.** `.command(name, spec)` returns a new definition, and the app
  exports the last one. A single `.commands({...})` map was tried; TypeScript could not
  infer each command's result type from it.
- **Names.** Command names are lowerCamelCase and unique. They are the CLI/MCP name and
  the undo label, and are frozen once a template ships.
- **Arguments.** `args` is an object of `s.*` nodes; with no `args`, a command takes none
  and can be called with nothing.
- **`run(ctx, args)` is synchronous and pure.** It receives exactly `current` (the
  frozen snapshot), `tx` (the collecting handles `change()` already provides; `insert`
  returns `{id}` at once), and `now` and `random()` from the host. It returns JSON.
  Throwing refuses the command with that message. This four-member `ctx` is the
  compatibility surface that must last.

### Running a command

- **In the page:** `doc.commands.X(args)` wraps `run` in `doc.change(...)`. It is one
  batch, with no runner round trip.
- **From the CLI** (`slop call PATH NAME --args JSON`), `slop-engine call`:
  1. reads the command's metadata and bundle from the file;
  2. checks the arguments with the core's value rules;
  3. reads the document through the same router as `get` (the live owner's socket, or an
     owner it opens while the document is closed);
  4. runs the command in the runner;
  5. sends the collected edits as one `batch` with `ifVersion`.

  If the document changed meanwhile, the core refuses the batch with `stale_base`, and
  the engine reads again and reruns, up to three times.
- **The runner** (`crates/slop-runner`): a fresh QuickJS engine per call evaluates a
  host-owned prelude and the slop's `document.js`.
  - **Scope:** each script runs in its own strict function scope; they meet only through
    `globalThis`.
  - **Determinism:** the prelude builds a headless `tx` from the shell's `handleFactory`
    and the shared snapshot path map, freezes the snapshot, and mints row IDs from a
    host-supplied seed. `Math.random` and `Date.now` throw.
  - **Limits:** a 2 s deadline (interrupt handler), a 64 MiB memory cap and a 512 KiB
    stack.
  - **No host:** no `fetch`, `require`, `process`, timers or QuickJS `std`/`os` modules.
- **Where it runs:** never in the app's process. On a Mac the CLI uses the engine inside
  hitSlop.app, which runs as its own process, as the CLI already does for document
  commands.

### What agents use

- `slop describe PATH` prints, in one read: what the slop is, each field's kind and
  description, its commands with their arguments, the current value with row IDs, and
  the version. It replaces `inspect`, `schema` and `get` as an agent's first step.
- `slop call PATH NAME --args JSON` prints `{result, ids}`. A refusal prints the
  command's own message (`command_failed`).
- The `hitslop-document` skill becomes: describe, then call when a command fits, else
  apply or batch.
- Later: `slop mcp` (generic), and a "Run slop command" App Intent with dynamic options.

### Storage

The spike stores command metadata as `assets/commands.json` and the bundle as
`assets/document.js`, so it needs no table change. The real version moves both into the
`app` row (a `commands` column for metadata and one for the bundle), which raises
`packageFormat`. That keeps the bundle out of the page's assets, and lets `validate-app`
check names, descriptions and argument descriptors in Rust, as it already checks the
descriptor.

## Spike results

Built:
- **Descriptions:** on every `s.*` node, in Rust and the SDK.
- **SDK:** `.command()` and `doc.commands` in `packages/document/src/schema.ts`.
- **Build:** command metadata and a `Bun.build` bundle of `schema.ts`, in
  `packages/cli/src/build.ts`.
- **Shared code:** the shell's snapshot path map, now used by the page and the runner.
- **Runner:** `packages/shell/src/runner.ts` (the prelude) and `crates/slop-runner`, with
  QuickJS by default and Boa behind a feature.
- **Precondition:** `ifVersion` on batches, in the core and on the socket.
- **Engine and CLI:** engine `call` and `commands`; CLI `call` and `describe`.
- **Examples:** both bundled examples use descriptions and commands.

Tests added:
- **Runner (8, on both engines):** edits and clock, row paths, seeded IDs and
  randomness, refusals, no host, scope isolation, runaway loop, memory cap.
- **Core (2):** description rules, and the version precondition.
- **Bun (2):** a command in the page is one change; the runner makes the page's edits
  from the same document.

The rust, bun, types and packed tiers pass. Hygiene and landing fail in the worktree only
because of its setup.

| Measure | QuickJS | Boa |
|---|---|---|
| `turnFor`, fresh engine per call, p50 | 0.9 ms | 3.0 ms |
| `fileFinished` on 1,000 rows, p50 | 8.4 ms | 51 ms |
| Peak memory (small / 1,000 rows) | 4.8 / 6.6 MB | 14.7 / 21.5 MB |
| Engine size added (release, macOS arm64) | 1.2 MB | 10.4 MB |
| Runaway loop | stopped by the deadline | stopped by a loop-iteration limit; no time-based interrupt |
| Memory cap | yes | no allocation cap |

- **Concurrency:** 40 `addTask` calls raced against 4 processes writing the title. One
  needed a rerun, and none failed.
- **Bundle size:** a slop's `document.js` is about 6 KB, and the prelude about 9 KB.

Problems the spike found:
- **Fixed: shared global scope.** The prelude and `document.js` ran as scripts in one
  global scope, so the SDK's top-level `freeze` replaced the prelude's, and every command
  received undefined arguments. Each script now has its own scope, and a regression test
  covers it.
- **Fixed: Boa wasn't strict.** Boa evaluates scripts in sloppy mode by default, so
  writes to the frozen snapshot were silently ignored. Both engines now run strict, as
  the page's ES modules do.
- **Open: author paths in bundles.** Bun's bundle comments contain the author's absolute
  paths. Strip them before anything ships.
- **Open: argument errors.** They say "Value does not match descriptor" without naming
  the field. The core's value errors should carry the path.
- **Open: undo grouping.** Consecutive agent batches merge into one undo step. Each
  command should be its own step, labelled with its name.
- **Open: live documents untested.** They need an app build that accepts `ifVersion`.

## The agent test

Commands exist to make agents (and automation) better at changing slops. Building them
can't show whether they do, so we measure it.

Each run gives Claude Code, run headless (`claude -p`), one everyday request on a fresh
document. A script then checks the saved document, and records turns, time and cost.

| Version | What the agent gets |
|---|---|
| A | Today: path operations, the released skill, no descriptions |
| B | A plus field descriptions and `slop describe` |
| C | B plus the slop's commands and `slop call` |

| Task | Request | Pass when |
|---|---|---|
| H1 | Set the hourglass to run out in 25 minutes | 25 minutes from about now |
| H2 | Count down to 3 hours from now, and call it "Bake bread" | Title and end time |
| H3 | How many minutes are left? (20 left) | Answer 17–21; nothing changed |
| H4 | Turn it over again for the same length as last time (40 minutes) | 40 minutes from about now |
| C1 | Add three tasks: buy milk, call mum, book the dentist | Three new open tasks |
| C2 | File away everything I've finished | Done tasks filed, others not |
| C3 | I filed "Send the first draft" by mistake; put it back | Unfiled; the app also unticks it (recorded separately) |
| C4 | Fix the typo in my walk task | "phone" spelled right, same row |
| C5 | Rename the list to "Weekend jobs" and tick off the walk | Both |
| C6 | Move "Make a little room for the weekend" to the top | Order, same rows |

- **How runs are isolated:**
  - the agent may run only `slop`, `date` and `python3`;
  - installed skills and MCP are turned off, and each condition's skill text is passed in
    the system prompt;
  - sessions aren't saved.
- **Size and cost:** one run per task and version is 30 sessions (about 10 minutes at
  four in parallel); three runs each is 90. Runs use the account's Claude usage, so they
  need a go-ahead and a choice of model (Sonnet suggested).
- **Harness:** `scripts/spikes/agent-eval.ts` in the worktree (`prepare`, `dry`, `run`).
  It has been checked without an agent: every template builds, every setup applies, and
  every task fails on an untouched document.

Decision rule:
- **C clearly beats B** (more requests right, or fewer turns): adopt commands.
- **B is about as good:** ship descriptions and `describe` only, and drop the runner,
  `call`, `ifVersion` and the SDK command API.
- **Either way,** record the numbers in `docs/evidence/commands-spike-2026-10.md`.

## Implementation plan if adopted

Each phase ends with `bun run verify` green, and `--native` where Swift or the helper
changed.

1. **Descriptions and `describe`** (ships either way)
   - Keep the spike's descriptor change and its test.
   - Move `describe` into the engine, so Linux and the app share one renderer, and give
     it a `--json` form for MCP.
   - Rewrite the `hitslop-document` skill around `describe`.
   - Document `description` in [document types](../docs/reference/document-types.md) and
     the public authoring guide.
2. **Command format**
   - Store `commands` (metadata) and the bundle in `app` row columns, and raise
     `packageFormat`.
   - Have `validate-app` check names, descriptions (1–500), argument descriptors, and
     that the bundle parses.
   - The build strips bundle comments and refuses imports that aren't plain modules (the
     rule `slop.ts` already follows).
   - Add a compatibility-corpus fixture with commands.
3. **Runner hardening**
   - Pin QuickJS (`rquickjs`, exact version) and drop Boa.
   - Name the field in argument and value errors.
   - Decide whether `slop-engine call` also applies an OS sandbox (seatbelt on macOS,
     Landlock/seccomp on Linux) on top of the empty QuickJS context.
   - Measure Linux binary size.
4. **Protocol and the live owner**
   - Add `ifVersion` and the engine's `call` to the command protocol: raise its version
     and keep serving the old one.
   - Make each command its own undo step, with the Loro commit message `agent:<name>`
     (prepares [attribution](../docs/ideas.md)).
   - Build the app and verify a live document: the window updates, Edit ▸ Undo reverts
     the command, and retries stay rare while the person types.
5. **SDK**
   - `.command()`, `doc.commands` and `CommandContext` become public, under a raised
     `runtimeABI`.
   - Author docs for when to write a command; the guidance is "domain actions, not
     keystrokes".
6. **Contracts and docs**
   - Update AGENTS.md and the [engineering contract](../docs/engineering-contract.md):
     closed edits never start WebKit, and authored command code runs only in the
     engine's runner, never in the app's process, with no ambient authority.
   - Update `schema.ts`'s "Neither the host nor the CLI evaluates authored callbacks".
   - Update [architecture](../docs/architecture.md), the CLI guide, the public CLI
     workflows and [ideas](../docs/ideas.md): Agent notes, Shortcuts, and `slop watch`
     and `slop mcp` move onto commands.
7. **Later**
   - `slop mcp`: one generic server, with the document as an argument.
   - A "Run slop command" App Intent.
   - "Claude ran File finished · Undo" in the window.

## Open questions

- **Naming (deferred by decision):** rename `schema.ts` to `document.ts`, and decide
  whether `defineDocument` becomes `defineSlop`, which `slop.ts` already uses.
- **Queries:** whether agents need them. Decide from the agent test.
- **Undo for runs:** whether an agent's run of several commands should still be one undo
  step.
- **Retries while typing:** how often a live document's typing forces a rerun.
- **Hiding general operations:** whether an author may ever do it. Proposed answer: no.
