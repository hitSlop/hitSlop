# Architecture

hitSlop documents are local packages (`.slop`) that pair an immutable authored app with
a structured document. One Rust core, `hitslop-core` on Loro, owns document semantics.
The Swift `DocumentOwner` owns the live core, the writer lock, SQLite and delivery to
the page. The WebView renders immutable snapshots and holds no CRDT.

```text
            page (WebKit)                               host (Swift)                  core (Rust)
 Svelte app ─ ctx ─ SDK store/handles ── open/apply/text/flush ──▶ DocumentOwner ── UniFFI ──▶ hitslop-core
            ◀────────── ordered pushes (__hitslop.publish) ────────┘   │ owner queue                (Loro)
                                                                        ▼
 CLI (slop / hitslop-native) ── socket (live) or writer lock (closed) ─┘ persistence queue ──▶ state/document.sqlite
```

The same core compiles to WASM for `slop dev` and the Bun tests only. The app, helper,
page shell and CLI are built from one tree; nothing has shipped, so there is no version
negotiation between them.

## Layers

| Layer | Where | Owns |
|---|---|---|
| Core | `crates/hitslop-core` | Descriptors, validation, `$id` rows, atomic batches, publications, issues, counters, text merges, frontier version tokens |
| Adapters | `crates/hitslop-core-{ffi,wasm}` | Records and typed errors (`Rejected`, `Invalidated`); no semantics |
| Owner | `HitSlopDocument/DocumentOwner.swift` | Owner queue (core calls), persistence queue (SQLite), save scheduling, epochs, view tokens |
| Session | `HitSlopDocument/DocumentSession.swift` | WebView, the `hitslop` message handler, the push queue, socket and discovery |
| Page shell | `packages/document` (served at `/__shell__/`) | Store, handles, text binding, write queue, barrier, attachments, theme application |
| Contracts | `packages/schema` (TypeBox) | Manifest, core wire, page protocol, socket; `bun run schema:generate` emits Rust and Swift |

## An edit

1. **Page.** A handle write (`set`, `insert`, `remove`, `move`, `increment`) or a
   `change(tx => …)` collector becomes one batch. Batches go through one FIFO queue, so an
   `insert` followed by a `move` cannot reorder.
2. **Host.** The page posts `apply {view, batch}`. The owner job checks the view token and
   epoch, then calls `apply_batch`. The reply is `{sequence, ids}`.
3. **Push.** The core's publication, `{previous, sequence, version, ops, issues}`, is
   appended to the session's push queue on the owner queue, so pushes keep owner order.
   One drain at a time delivers everything buffered through a single awaited
   `__hitslop.publish(pushes)` call. Swift never parses publications.
4. **Store.** The page applies publications in sequence order, ignores any at or below
   its sequence, and copies only the objects on the changed paths; unchanged rows keep
   their identity. The write's promise resolves once the store reaches the reply's
   sequence, so the snapshot has updated when `await` returns.

A gap (`previous` above the store's sequence) or an apply failure makes the page call
`open` again and replace its state. Text still in a field survives: bindings keep their
DOM text and their confirmed version, which stays valid because versions name owner
history.

## Text

A text binding keeps the user's text in the field. It sends at most one request at a
time: `text {base, path, from, to, selection}`, meaning "this field was `from` at
`base` and is now `to`".

- The core computes the edit script on a throwaway document (its diff mutates while it
  runs, so it never touches the owner) and checks that the script reproduces `to`.
- **Fast path:** the owner's field still equals `from`, so the script applies directly.
- **Slow path:** the field changed concurrently. The script is applied on a fork at
  `base` and merged with Loro; the caret is mapped through cursors.
- The reply names `authored`, the version right after this edit on its own branch. If the
  user kept typing, the next request goes from the sent text at `authored`.
- During IME composition nothing is sent. Close and export commit a composition.
  Retargeting or unmounting a binding sends its unsent text first.
- Every token is checked against the document's history before Loro sees it, so a
  malformed or foreign base returns `stale_base`, never a panic.

A text handle's `set(value)` and the CLI's `set` replace the whole field as it is when
the owner applies it, through the same precomputed script.

## Saving

- The owner queue runs core calls; the persistence queue runs every SQLite call. At most
  one write is in flight, and edits during it coalesce into the next one.
- Each write records an attempt token in the same transaction. A lost reply is settled by
  reading the token back; it is never guessed from the generation.
- A write that fails keeps ownership and all edits. Failures are typed
  (`full`, `busy`, `moved`, `invalidated`, `io`) and reach the window, which offers retry,
  or discard for a full document.
- `flush` resolves when the saved sequence covers every edit accepted before the call.
  `close` refuses new edits, flushes, then releases the lock. `discard` rotates the epoch,
  waits for the write in flight, and reloads saved bytes; requests captured before it are
  refused with `owner_replaced`.
- Save status flows one way: owner to window, and owner to page as `saved`/`failed` pushes.

Storage is `document(checkpoint, schema_key, generation, doc_id, last_attempt)` plus
`updates(seq, bytes)`. A checkpoint replaces the log at 256 updates or 4 MiB; the limits
are 4,096 updates and 32 MiB.

## Close, export and capture

The page barrier sends unsent text, waits for queued writes and attachment imports, then
flushes; it never joins a flush that already passed its drain point. Inputs stay
enabled, so focus survives a cancelled barrier. Swift then saves and closes the owner.
An attachment import stores the blob and submits its reference through a collector
admitted past an active barrier, so a blob is never saved without its reference.

## CLI

`slop` forwards to `hitslop-native`. If the document is open, the command goes to the
owner's socket, which lives as long as the owner, not the page. Commands run on the owner
directly: they never blur the field being typed in, and a live `get` returns
owner-accepted state. If the document is closed, the helper takes the writer lock and
runs the owner in process, without WebKit or authored code. Edits print
`{ids, sequence, value}`; mutations are never replayed automatically.

## Themes and attachments

Theme defaults live in `assets/theme.json` and overrides in `state/theme.json`. Swift
validates every write (known tokens, UTF-16 lengths, no `{};`, known `var(--slop-*)`
references, 64 KiB); the page only applies values. Attachments are content-addressed
immutable blobs in `state/attachments`, written through the owner.

## Tests

Tests live at the boundary that owns the behavior; see [testing](testing.md).

| Boundary | Proves |
|---|---|
| Rust (`crates/hitslop-core/tests`) | Semantics, publications equal fresh snapshots, text merges (`text.rs`), token validation (`tokens.rs`) |
| SDK over WASM (`packages/document/tests`) | Write timing, snapshot identity, collectors, text binding, stream recovery, barriers, attachments |
| Swift (`apps/apple/Packages/HitSlopApple/Tests`) | Persistence scheduling, lost replies, writer lock, view and epoch fences, CLI, WebView bridge, export |

Performance evidence is in [`evidence/`](evidence/). At 1,000 rows a window opens in about
0.6 s and a checkbox is accepted in 14 ms (p95).
