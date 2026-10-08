# Next phase: host-owned documents without Loro

> **Superseded 2026-09-29** by [LoroRustCutover.md](LoroRustCutover.md): documents stay on Loro, owned by a host Rust core. This JSON-ops/JavaScriptCore plan failed its own gates above 1k rows (`spikes/document-authority/REPORT.md`).

Status: proposal, 2026-09-28. Nothing here is an active contract yet. When a
phase lands, its rules move into `AGENTS.md`, `docs/engineering-contract.md`,
`docs/versioning.md` and the test ledger. This file then becomes history.

## 1. Why

hitSlop has Loro only so that collaboration can happen later. We now require
shared slops to be **online while editing**, with a Cloudflare Durable Object
(DO) as the single ordering authority. Under that rule, no part of the product
needs a CRDT. We still pay for one everywhere:

| Cost today | Where it lives |
|---|---|
| Two CLI edit paths. If the window is open, edits go over the socket into the WebView's Loro. If it is closed, an invisible WebKit session boots the engine. | `HitSlopDocument/DocumentCommand.swift`, `DocumentSession.swift` (headless), `SchemeHandler.swift` headless page, `packages/document/src/headless.js` |
| Opaque checkpoint and update storage, capacity accounting, and a minimum reader revision tied to Loro pins | `HitSlopDocument/Storage.swift`, `StorageBridge.swift`, `StorageProbe.swift`, `state/document.sqlite` format 2 |
| Counter checkpoints, plus an unfixed Loro counter replay bug (accepted 1, replayed 0) | `spikes/engine-placement/counter-probe.ts`, REPORT.md |
| Merged anomalies preserved and flagged (`issues`), peer-ID rules, fork staging for atomicity | `packages/document/src/document.ts`, `operations.ts`, `projection.ts` |
| A WASM engine booted in every window (~50 MiB per window, and slower opens at small sizes) | `spikes/engine-placement/evidence/table.md` |
| JSON import that maps values onto CRDT operations | `packages/document/src/json-import.ts` (405 lines) |

The engine-placement spike already concluded that the authoritative document
should live outside the renderer, with one semantic implementation shared by
every consumer. It assumed Loro stays. This plan keeps those conclusions and
drops that assumption.

## 2. Decisions

1. **The host owns the document.** The WebView is a view: it receives snapshots
   and sends operations. It never holds the authoritative state.
2. **The document is plain JSON plus typed operations.** No CRDT, locally or in
   collaboration.
3. **One reducer, written in TypeScript**, `apply(schema, doc, batch)`. It is
   pure and has no dependencies. It runs in:
   - the Mac app and the native CLI helper, in an in-process `JSContext` (no
     WebView, no Node, no Bun)
   - `slop dev`, in the browser, against memory
   - the future collaboration DO, in Workers

   This replaces the "no JavaScriptCore evaluator" rule. That rule prevented a
   *second* engine; this is the *only* engine.
4. **One edit path.** The process that holds `writer.lock` owns the document.
   Every edit, from the UI, the CLI or an agent, goes to that owner.
5. **Clean break.** A new runtime identity. Documents made with Mac 1.1.x (Loro,
   SQLite format 2) are refused at open with a clear message, the same way
   pre-v1 documents are today. Sealed runtime bytes and release records stay in
   the repo as history.
6. **SDK freedom.** The DSL and SDK may change. The names
   `defineDocument`/`s`/`useDocument`/`bindText`/`<Slop>` are kept where the
   meaning still fits.
7. **Narrow first.** Every example except Quick Checklist moves to
   `archive/slops/`. Small Expenses returns as the second fixture once the
   checklist works end to end. Other slops return one at a time, on demand.

## 3. Target architecture

```
 ┌─────────────────────┐   ops batch    ┌──────────────────────────────┐   ops batch   ┌───────────┐
 │ WebView (slop view) │ ─────────────▶ │ DocumentOwner (Swift)        │ ◀──────────── │ CLI/agent │
 │ snapshot + drafts   │ ◀───────────── │  • writer.lock               │   (socket or  └───────────┘
 └─────────────────────┘ {rev, value}   │  • JSContext running core.js │    in-process)
                                        │  • state/document.json       │
                                        └──────────────────────────────┘
```

### 3.1 Ownership and the single edit path

- If the document is **open**, the app holds `writer.lock` and runs the
  `DocumentOwner`. The CLI helper forwards requests over the existing Unix
  socket.
- If the document is **closed**, the `hitslop-native` helper takes
  `writer.lock` and constructs the same `DocumentOwner` in-process. It
  applies, saves and releases. **No WebKit is involved.**
- The only branch is "forward vs. own", and both sides run the same Swift type
  and the same `core.js`. We never bypass a busy lock or unlink `writer.lock`
  (unchanged rule).
- Open question for spike S5: can the app open a document while a helper holds
  the lock? Proposed behavior: the app waits briefly, then shows "being edited
  by the CLI" with retry. It never steals the lock.

### 3.2 Storage

- `state/document.json` = `{ "format": 1, "schema": "<key>", "rev": N, "value": {…} }`.
- Save = write `state/.document.json.tmp`, `fsync`, `rename`, then `fsync` the
  directory.
- `rev` increments once per accepted batch. It is a local counter, not a
  version vector.
- Saves are coalesced: the first unsaved batch schedules a save in ≤200 ms, the
  same policy as today. `flush()` is a barrier.
- A failed save keeps the edits in memory, keeps the lock, and shows the
  existing native retry. Close and export wait for a successful save
  (unchanged rule).
- `state/attachments/` is unchanged: host-owned immutable blobs, referenced
  from ordinary fields.
- Capacity: a hard cap on serialized size (proposed 16 MiB; S4 confirms). The
  cap is checked before the batch is accepted, so the in-memory state never
  exceeds what can be saved.
- The AGENTS.md ban on `stores/data.json` exists to prevent a mirror. This file
  is not a mirror; it *is* the store. Update the rule.

### 3.3 Schema subset (v1 of the new runtime)

| Descriptor | JSON value | Ops |
|---|---|---|
| `s.text()` | string | `set` (whole string; see §3.5 for later `splice`) |
| `s.string()` | string (short, not bound to a text editor) | `set` |
| `s.boolean()` | boolean | `set` |
| `s.number({ int?, min?, max? })` | finite number | `set`, `increment` |
| `s.enum([...])` | one of the listed strings | `set` |
| `s.optional(T)` | `T` or absent | `set`, `set` with `undefined` removes the value |
| `s.object({...})` | object | `set` on a field; `set` on the whole object only if it contains no lists |
| `s.list(s.object({...}))` | array of objects, each with a host-minted `$id` | `insert`, `remove`, `move` |

Deferred until a slop needs them: rich text, trees, records, scalar lists and
attachment-specific descriptors (an attachment ref is a `string` for now).

`s.counter()` is gone. A counter is `s.number({ int: true })` plus
`increment`. That removes the Loro counter bug entirely.

Descriptors remain **data** (`state.schema.json`). The reducer interprets them.
TypeBox owns the descriptor, op and state-file contracts.

### 3.4 Operations (shared wire shape for the SDK, the socket and the CLI)

```ts
type Seg = string | { id: string };          // key, or list row by $id
type Path = Seg[];
type Anchor = { before: string } | { after: string } | {};  // {} = end

type Op =
  | { type: "set";       path: Path; value: unknown }
  | { type: "increment"; path: Path; by: number }
  | { type: "insert";    path: Path; value: object; at?: Anchor; id?: string }
  | { type: "remove";    path: Path; id: string }
  | { type: "move";      path: Path; id: string; to: Anchor };

type Batch = { ops: Op[]; baseRev?: number; message?: string; origin?: "view" | "cli" | "import" };
type Result = { rev: number; ids: string[] } | { error: { code: string; opIndex: number; message: string } };
```

Rules:

- **A batch is atomic.** The reducer works on a copy-on-write copy, validates
  and applies each op, and commits only if every op succeeds.
- **Paths never use indices.** An agent that read the document 30 seconds ago
  still targets the right row.
- **The host mints `$id`s.** The reducer takes an injected id source, which
  keeps it deterministic in tests. `insert` may pass `id` when the caller
  must reference the new row later in the same batch; the reducer rejects
  duplicates.
- **`baseRev` is optional.** It is required for whole-document replacement
  (`import --replace --if-version`). Ordinary ops do not need it, because `$id`
  paths already express intent.
- **Error codes** are stable strings: `path_not_found`, `type_mismatch`,
  `out_of_range`, `duplicate_id`, `stale_rev`, `too_large`,
  `unknown_op`. Agents rely on these.
- The reducer exports `apply`, `validateDocument`, `createInitial(schema,
  initialJson, idSource)` and `get(doc, path)`.

### 3.5 Host ↔ view protocol

- On host → view mount, and after every accepted batch, from any origin:
  `{ type: "state", rev, value, origin, batchId? }`.
- Start by sending the **full snapshot**. It is trivially correct and tiny at
  checklist size. Switch to patches only if S2 shows it matters.
- View → host: `{ type: "batch", batchId, batch }` → the reply is `Result`.
- **The view does not apply ops locally.** It waits for the next `state`
  message. S2 must confirm that the round trip stays under one frame at
  realistic sizes.
- **The SDK keeps structural sharing.** When a new snapshot arrives, rows whose
  JSON is unchanged keep their previous object identity, so Svelte keyed
  `{#each}` does not re-render them.
- **Text binding (`bindText`) is the one place with local state:**
  - While the element is focused, the element's value is the draft. Input
    sends `set` (debounced ~150 ms, and immediately on blur or Enter).
  - Echoes of the binding's own batches are ignored.
  - When a *foreign* change (from the CLI or another origin) arrives for a
    focused field, the draft wins locally, and the next send overwrites the
    foreign change (last writer wins, per field). Once the field is unfocused,
    the element adopts the snapshot.
  - IME composition never sends mid-composition.
  - Later, for collaboration: add a `splice { path, index, delete, insert }`
    op plus server-side text OT for text fields only. This is additive and
    changes no existing op.

### 3.6 SDK (`@hitslop/document`) after the change

```ts
const doc = useDocument(schema);
doc.current                      // immutable snapshot (structurally shared)
doc.rev                          // last rev the view has seen
doc.fields.tasks.insert({...})   // op builders: send a single-op batch, return Promise<Result>
doc.fields.tasks.move(id, { before })
doc.at(row).done.set(true)
doc.change(tx => { ... }, { message })  // collects ops into one batch
doc.flush()                      // resolves after the host reports a durable save
doc.status / doc.error           // "saved" | "saving" | "save-failed"
```

Removed: `issues`, `full`, preview and uncommitted state, and every handle type
that exists only because of Loro containers.

Kept unchanged on `ctx`: `bind`, `capture`, `attachments`, `theme`, `window`
and `reportError`. The ABI number increases. The rule that "the runtime owns
the page and apps reach it only through `ctx`" still holds.

### 3.7 CLI

| Command | New behavior |
|---|---|
| `get DOC [--path]` | Reads from the owner (live, or local if closed). Returns `{rev, value}`. |
| `apply DOC --op JSON` | One-op batch. |
| `batch DOC --ops JSON` | Atomic batch. |
| `import DOC --from FILE [--replace --if-version REV]` | Validates the whole document, then replaces it, with a `baseRev` check. |
| `schema DOC` | Prints the descriptor and the op reference. |
| `create`, `open`, `export`, `theme`, `attachments` | Unchanged surface. |
| `compact` | **Removed.** There is no log to compact. |

`get --snapshot` becomes plain `get`, because the snapshot is the document.

## 4. What gets deleted

- **`packages/document/src`:** the Loro boot and `headless.js`, most of
  `document.ts`/`operations.ts`/`projection.ts`/`json-import.ts`, the storage
  bridge in `storage.ts`/`session.ts`, and the Loro-shaped `handles.ts`. These
  are rewritten as op builders.
- **`HitSlopDocument`:** `Storage.swift`, `StorageBridge.swift`,
  `StorageProbe.swift`, the headless page in `SchemeHandler.swift`, the Loro
  entries in `RuntimeCatalog.swift`, and the engine half of
  `DocumentSession.swift`. What remains of `DocumentSession` becomes a view session.
  The module is renamed `HitSlopDocument`.
- **Loro and WASM resources** in the runtime bundle. `loro-swift` is no
  longer referenced.
- **Contract text** about checkpoint/update storage, counter checkpoints,
  minimum reader revision, peer IDs, preserve-and-flag merges, and
  "Loro stays in the WebView".
- **Examples:** everything except `quick-checklist` (and later
  `small-expenses`) moves to `archive/slops/`, and `bundled.json` is trimmed to
  match.

Reused as is: `DocumentWriterLock.swift`, `Files.swift`, `SocketServer.swift`
(new method set), `SlopAttachments.swift`, package validation, the catalog,
windows and the toolbar, export and the icon renderer (fed the owner's
snapshot), Sparkle, Firebase and TCA features.

## 5. Spikes (run before and alongside the build)

Each spike lives in `spikes/<name>/`, is disposable, and ends with a short
`REPORT.md` covering the question, method, numbers, verdict, and what changes
in this plan. Where possible, reuse the engine-placement harness
(`spikes/engine-placement/run.ts`, `web/`, `Sources/Harness`). It already
measures checkbox, move, splice, paste and sustained typing at 1k/5k/40k rows
in fresh processes, and it holds the WASM and native Loro baselines.

S1 and S2 decide whether the JavaScriptCore direction holds. Run them first.

### S1: Reducer in JavaScriptCore (go/no-go for decision 3)

- **Question:** Is a TS reducer in an in-process `JSContext` fast and robust
  enough to be the only engine on the Mac?
- **Method:**
  - Write a minimal `core.js` (set, insert, remove, move, increment; checklist
    schema) and load it into a `JSContext` from a Swift executable.
  - Keep the document *inside* the context. Swift sends batch JSON and gets
    back `{rev, snapshotJSON}`.
  - Measure:
    - `JSContext` creation plus `core.js` evaluation, cold and warm
    - loading `document.json` into the context at 1k/5k/40k rows
    - p50/p95 latency to apply a single checkbox `set` and a `move`
    - snapshot serialization back to Swift
    - resident memory
  - Also try `JSValue` handoff vs. string JSON for the snapshot.
- **Pass:**
  - checkbox/move apply plus snapshot serialization p95 ≤ 5 ms at 5k rows and
    ≤ 30 ms at 40k rows
  - context plus load ≤ 100 ms at 5k rows
  - memory ≤ the native-Loro candidate's numbers in the engine-placement
    evidence
- **Kill or adjust:** If snapshot serialization dominates, go straight to
  patches (S2b). If JavaScriptCore itself is the problem, fall back to "Swift
  reducer + TS twin with shared fixtures" and record why.
- **Also check:**
  - The JIT is available to the sandboxed, hardened-runtime app and to the
    helper. If not, measure interpreter-only mode.
  - Exceptions and timeouts: use a watchdog around `evaluateScript` for
    pathological batches.

### S2: View round trip without local apply

- **Question:** Without optimistic local apply, does WebView → host →
  full-snapshot → Svelte feel instant?
- **Method:** Add a fourth candidate, `json-jsc`, to the engine-placement
  matrix. It uses the same Svelte view, rows and scenarios, with the host
  running S1's context. Record acceptance latency and the two-frame render
  opportunity, at 1/5/10 windows.
- **Pass:** at 1k and 5k rows, checkbox and move render-opportunity p95 is no
  worse than the native-Loro candidate by more than one frame (16.7 ms), and
  sustained typing drains in under 50 ms.
- **S2b (conditional):** If full snapshots fail at 5k rows, prototype
  `{rev, patches}` (JSON-pointer-like, by `$id`) and re-run. Only adopt
  patches if they are needed.
- **Also measure:** the cost of structural sharing when the SDK rebuilds
  snapshots, with and without identity preservation. This shows how many rows
  Svelte re-renders per edit.

### S3: Text binding under the host-owned model

- **Question:** Does draft-while-focused `bindText` behave correctly with IME,
  selection and foreign edits?
- **Method:** Run in a real `WKWebView` window with a real keyboard (scripted
  events plus a manual checklist):
  - Japanese/Chinese IME composition in the title and a task text field
  - selection and caret stability while typing fast
  - paste
  - undo within the field (browser-native)
  - a CLI `set` on the same field while it is focused, then after blur
  - a CLI `remove` of the row whose text is focused
- **Pass:**
  - The caret never jumps.
  - Nothing is sent mid-composition.
  - A foreign change while focused behaves as documented (last writer wins on
    the next send) and is adopted when the field is unfocused.
  - Removing the focused row blurs the field cleanly, with no error and no
    resurrection of the row.
- **Output:** the documented `bindText` contract plus a Bun test at the SDK
  boundary (with a fake host) for the non-IME cases.

### S4: Atomic JSON save

- **Question:** Is temp + fsync + rename durable and fast enough, and what is
  the size cap?
- **Method:**
  - Measure save time at 10 KB / 1 MB / 16 MB on APFS, with `F_FULLFSYNC` vs.
    plain `fsync`.
  - Run a `kill -9` loop during saves (1,000 iterations) and verify that each
    reopen yields either the old or the new document, never a torn one.
  - Test full-disk and read-only-volume failures, and verify the retry UI
    path keeps edits.
- **Pass:** a 1 MB save p95 ≤ 20 ms, zero torn files, and failures surface as
  save-failed with edits retained.
- **Decide:** the size cap, and whether `F_FULLFSYNC` runs on every save or
  only on close.

### S5: Single edit path and lock races

- **Question:** Is "forward vs. own" race-free across the app opening or
  closing while the CLI runs?
- **Method:** A scripted harness that interleaves:
  - CLI `batch` while the app is opening the document
  - CLI `batch` while the app is closing it (the save-before-close barrier)
  - two CLIs at once on a closed document
  - the app opening while a helper holds the lock
  - the helper crashing mid-batch (the lock is released by the OS; the file
    is old or new)
- **Pass:**
  - Every batch is either applied exactly once or rejected with a retryable
    code.
  - Nothing is lost silently.
  - Nobody bypasses the lock.
  - The window shows CLI edits within one `state` message.
- **Output:** the exact socket method set and retry codes, which feed the
  TypeBox contracts.

### S6: Helper cold start

- **Question:** Is `hitslop-native apply` on a closed document fast now that
  it no longer boots WebKit?
- **Method:** Time process launch → lock → JSContext → load → apply → save →
  exit, and compare with today's headless WebKit path.
- **Pass:** ≤ 150 ms end to end at 1k rows. Today's path is the baseline to
  beat by a wide margin.

### S7: Collaboration feasibility (design check only; not blocking launch)

- **Question:** Does the same `core.js` work as a DO room authority, and does
  client rebase stay simple under online-only editing?
- **Method:**
  - Deploy a throwaway Worker plus DO that loads `core.js`. It accepts
    `{clientId, mutationId, batch}`, dedupes, applies, assigns `rev`, and
    broadcasts.
  - A Swift client keeps a pending queue. It applies locally, sends, and on
    each confirmation resets to the server state and replays what is still
    pending.
  - Run two Mac clients plus one CLI agent against the same room.
  - Inject: reordering, duplicate delivery, lost acks, reconnect, and a DO
    restart from its snapshot plus log tail.
  - Prototype text `splice` with a minimal server-side OT transform for
    concurrent typing in one field.
- **Measure:** Worker bundle size, CPU per batch, and memory at 5k rows.
- **Pass:**
  - The clients converge in every injected case.
  - The `core.js` changes needed are additive (the `splice` op only).
  - Converting a local document to a shared one means uploading
    `document.json`.
- **Output:** a sketch of the collaboration protocol, and confirmation that
  nothing in §3 must change before launch to allow it.

### S8: Agent ergonomics check

- **Question:** Can a coding agent edit a checklist correctly from
  `schema` + `get` output alone?
- **Method:** Give an agent only the CLI and the embedded
  `hitslop-document` skill. Ask for 10 realistic edits: add tasks, reorder,
  archive finished tasks, rename, fix a typo in the third task. Record the
  failures.
- **Pass:** ≥ 9 of 10 succeed on the first try with `$id` paths. Error
  messages alone lead to a correct retry.
- **Output:** tweaks to the op names and error text *before* they freeze.

## 6. Build phases

Each phase ends green on `bun run check && bun run test`, and phases 3 and
later also on `bun run build && bun run swift:test && bun run test:native`.

**Phase 0: Spikes S1, S2, S4, S6.** Record the verdicts. If S1 fails, stop
and revise decision 3 before continuing.

**Phase 1: Reducer.**
- `packages/document/src/core/`: `apply`, `createInitial`,
  `validateDocument`, `get`, the id source and error codes.
- Rewrite `schema.ts` to the §3.3 subset. Descriptor, op and state-file
  contracts are in TypeBox, then run `bun run schema:generate`.
- Bundle to `core.js` (IIFE, no DOM).
- **The conformance fixtures are the contract:**
  `packages/document/test/core/*.json` with
  `{schema, before, batch, after | error}`. They run in Bun now, in Swift via
  JavaScriptCore in phase 3, and in the DO later.

**Phase 2: SDK and dev runtime.**
- Op builders, `change`, snapshot store with structural sharing, `flush`.
- The new bridge messages, and `bindText` per S3.
- `slop dev` host = `core.js` + memory in the page.
- Bump the ABI; update `packages/document/src/abi.ts` and the tests in
  `tests/abi`.

**Phase 3: Swift owner.**
- `HitSlopDocument/DocumentOwner.swift`: lock, JSContext, the
  `{rev, value}` store, coalesced atomic saves, `flush`, and failure retry.
- The view session relays `batch`/`state`.
- The socket method set comes from S5.
- `create` writes `document.json` from `initial.json` via `createInitial`.
- Export and the icon are fed the owner snapshot.

**Phase 4: CLI.**
- `DocumentCommand`/`NativeCLI` implement forward-or-own.
- The TS CLI commands follow §3.7.
- The new runtime identity refuses old packages with a clear message.
- Remove `compact`.

**Phase 5: Slops.**
- Archive the examples and trim `bundled.json`.
- Port Quick Checklist. Its schema already fits; `App.svelte` only needs to
  handle promises from the op builders where it cares.
- Update the `init` starter template and the embedded `hitslop-document`
  skill.
- Then port Small Expenses as the second black-box fixture.

**Phase 6: Contracts and docs.**
- Rewrite the affected lines of `AGENTS.md` and
  `docs/engineering-contract.md`.
- Update `docs/versioning.md` (the new runtime identity, and the refusal of
  Loro-era documents).
- Update `docs/testing.md` and the test ledger with the five-column entries
  for changed contracts.
- Update the README's Tiny Wins example (`s.number({int:true})` +
  `increment`) and the CLI docs.

**Phase 7: Port more slops as needed.** Add descriptors (records, scalar
lists, rich text) only when a returning slop needs them. Each one needs
reducer fixtures first.

## 7. Testing strategy

- **Bun owns semantics.** The reducer fixture table covers:
  - atomic rollback on the Nth op
  - `$id` targeting after a reorder
  - `move` before/after/end
  - a `move` onto itself
  - `remove` of a missing id
  - type, enum and range rejection
  - `increment` on a non-int
  - optional clears
  - `baseRev` staleness
  - the size cap
  - `createInitial` minting unique ids
- **Swift proves native integration only:**
  - `DocumentOwner` save/reopen
  - save failure keeps edits and the lock
  - a busy lock forwards and never bypasses
  - a JSContext load or eval failure surfaces as an error
  - the Swift test target also runs the Bun fixture table through
    JavaScriptCore (one test that iterates the fixture files, which proves
    JavaScriptCore behaves like the Bun runtime)
- **Native end to end (`test:native`), with Quick Checklist:**
  1. Create a document and add tasks in the window.
  2. With the window open, run `slop apply … insert`; the row appears.
  3. Close the window and run the same command. It succeeds with no WebKit
     process, and `get` shows it.
  4. Reopen: the state has persisted. PNG and PDF export, and the Finder
     icon, match the saved state.
- **Remove old coverage deliberately.** Loro-specific tests (counter
  checkpoints, merge anomalies, storage revision, compaction) are deleted
  together with the behavior they protect. Record each deletion in the ledger.

## 8. Risks

| Risk | Mitigation |
|---|---|
| Hardened runtime or sandbox disables the JavaScriptCore JIT, making the reducer slow | S1 checks this explicitly; the reducer is small enough that interpreter mode may suffice; the Swift twin is the fallback |
| Full-snapshot publishing is too slow for large slops | S2b patches; per-field subscriptions later |
| Last-writer-wins text surprises users when an agent edits a focused field | Rare locally; documented; `splice` + OT arrives with collaboration |
| Dropping Loro closes the door on offline collaborative editing | Accepted: shared slops require online editing. Local slops are fully offline. |
| Clean break strands 1.1.x users' documents | Accepted. Optional: the old app's `get --snapshot` JSON imports with `import --from` if the shape matches. This is best effort, not a contract. |
| Reducer drift between Mac, dev and the DO | Impossible by construction (one `core.js`); the fixture table runs in all three hosts |

## 9. Open questions

- Should `rev` be exposed to slop authors, or kept SDK-internal?
- Should undo (deferred) be a host-side inverse-batch stack? The reducer can
  return inverse ops cheaply. Decide before freezing `Result`.
- Should document history (deferred) be an append-only batch log beside
  `document.json`? It is not needed for launch, but the format should leave
  room for it (`format` field).
- Should theme overrides stay a separate host file, or become a reserved
  top-level key? Proposed: keep them separate (unchanged).
