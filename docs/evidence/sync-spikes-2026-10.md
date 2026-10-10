# Sync spikes (2026-10)

The checks behind the sync design in [architecture](../architecture.md#sync): local owners
under one Loro peer per session, a merge-closed layout, Loro's `UndoManager`, three-way text
merges, and a Durable Object that relays Loro updates indexed by version vector.

- **Repeatable in this repository:** `crates/hitslop-core/tests/merge.rs` (merge closure),
  `sync.rs` (the relay protocol under faults, refusals and golden frames), `undo.rs`,
  `text.rs`, and the two-owner sync test in `owner.rs`. `bun run verify rust` runs them.
- **Historical local experiments:** a standalone crate and a TypeScript Durable Object
  under `wrangler dev`, kept outside the repository. Their results are recorded below;
  file names in their sections refer to that crate. They ran on the product's exact Loro
  pin, `c00c9fa501f8d32f68d6255eacb7035a67fb6ab6` (loro 1.16.2-dev, 2026-09-30).

## S8: the mergeable container encoding (desk check, 2026-10-09)

**Pass.**

- The synthetic-root cid encoding (`🤝:$root>key…`) and the marker
  (`MERGEABLE_MARKER_MAGIC = 00 'L' 'M' 01`) landed upstream in loro-dev/loro#1002
  (2026-06-08). `crates/loro-common/src/lib.rs` has not changed it since: the next and last
  commit, #1062 on 2026-08-08, is unrelated.
- The published `loro-common` 1.16.0 (crates.io, 2026-09-06) has a `new_mergeable` that is
  byte-identical to our pin. The published `loro` 1.16.2 (2026-09-21) exports
  `ensure_mergeable_*`.
- "No version byte" in `crates/loro-internal/docs/mergeable-container-id.md` refers to
  replacing an earlier *unpublished* encoding. The current encoding is the published one,
  so Loro's compatibility policy for released formats covers it.

Still open, to cover in S1: cid length at descriptor depth 16 with 64-character row ids,
and the cost of `ensure_*` on hot paths.

## S6: trimming and sharing

All of these confirm the retention rule: a shared document never trims its history.

| Case | Observed |
|---|---|
| (a) A trimmed replica imports a concurrent edit made before its shallow root | `Err(ImportUpdatesThatDependsOnOutdatedVersion)`. The untrimmed original merges the same edit |
| (b) Three joiners start from a seed that was trimmed before sharing, then edit concurrently | All converge |
| (c) A full-history snapshot imports into a partial replica that holds its own concurrent edits | `Ok`, and the replicas converge |
| (d) A shallow snapshot imports into an older replica with a real history gap (another peer interleaved, so changes can't merge) | `Ok` with `pending: {1: 7..8}` and **no state change**. The full snapshot of the same document imports and converges |

Consequences:

- Shared documents never trim, and server compaction uses full snapshots only.
- New finding from (d): a missing-history import does not fail loudly. It returns `Ok` and
  leaves ops pending. Sync imports may tolerate pending ops in transit, but pending ops that
  remain after `CaughtUp` mean the server lacks history. That is a protocol fault: pause sync
  and report it, never wait silently.
- The looser version of (d) passed only by coincidence. Loro had merged a peer's
  consecutive commits into one change, which the shallow snapshot carried whole.

## S9: counters

| | LoroCounter | Contribution map `{peer-hex: i64}` |
|---|---|---|
| Exactness past 2^53 | **Fails**. The pinned regrouping probe still gives source 9007199254740991, replica 9007199254740990, same VV | Exact by construction |
| 60 seeds × 300 steps, 3 replicas, regrouped delivery, UndoManager undo and redo | Exact while every op has \|δ\| ≤ 2^31. The final value matched the integer sum of every op in history and a fresh snapshot replay. Seed 1: 195 ops, Σ\|δ\| = 7.4e10 | Identical contributions on every replica, exact sums. Up to 31 keys (sessions reopened about 25 times) |
| Concurrent +5 and +3, then one side undoes its own step | 3 on both | 3 on both |
| What makes it exact | Every partial sum is exact while Σ\|δ\| over the counter's whole history is below 2^53. Enforcing that needs a per-op cap plus a global op-count ceiling, or per-counter Σ\|δ\| bookkeeping (scanning imports through JSON export took 8.4 ms per 10k atoms) | Nothing. Each key has a single writer, so last-writer-wins never races |
| Costs | A per-op cap and splitting large increments, or a documented lifetime limit | One key per session that increments. The sum is computed over keys |

Finding for S1 and S2: an undo step that *created* a mergeable child hides the child (it
removes the marker) rather than reverting its content, and a later `ensure_*` brings the
old content back. Two consequences:

- Projection and reads must use `get`, never `ensure_*`, because `ensure_*` is a write.
- Required containers come from the template seed, so no session's undo step creates them.

The first S9 run failed for exactly this reason, because the test read through `ensure_*`.

Decision (user, 2026-10-09): **plain LoroCounter**. "These are mini apps, they don't need
industrial-grade guarantees." The rules are safe-integer increments and a local range check.
There is no per-op cap and no lifetime bookkeeping. The contribution map is the fallback if
a realistic divergence ever shows up.

## Dropped spikes

- **S3, atomic batch rollback.** A refused multi-intent batch rebuilds with the same peer
  and resets undo. That is rare and good enough.
- **S10, memory without maintenance.** Accepted for mini apps, so nothing is measured.

## S1: merge closure of the layout

**Pass.** The experiment (`tests/s1_merge.rs`) prototyped the layout:

- every field is a mergeable child;
- rows are `{rows: map<$id>, order: movable list<$id>}`;
- counters are `LoroCounter`;
- the template is seeded on peer 1;
- reads never call `ensure_*`.

Three replicas made random edits: title text, `done`, the optional `note` (set, edit,
clear), record entries, `hits`, and row insert, remove, move and edit, with row ids drawn
from a pool of 6 so concurrent creates of one id happen. They also ran `UndoManager`
undo/redo, and exchanged full and partial deliveries in random order.

- Run with `S1_SEEDS=2000`: 2,000 seeds, 500,000 steps and 25,010 undo calls. Every state
  every replica reached passed the merge-closed acceptance with a total projection. After a full
  exchange, all replicas had identical deep values and projections. A fresh document
  reopened from a snapshot projected identically. The run took 32 s.
- Duplicate ids in `order` appeared in about half of the seeds: 1 in 648 seeds, 2 in 230,
  and 3 or more in 116. Projection dedupe is required, not theoretical.
- **Protocol rule found:** deliveries must ship whole commits. The first run cut a peer's
  history at an arbitrary op, which split a row creation. The receiving replica briefly saw
  the row with `name` but without `count`, and acceptance refused it. Our relay never does
  this, because pushes end at commit boundaries.
- loro-dev/loro#1112 (zero-sum counter diffs dropped): a split import and a whole import
  both project `hits = 0`. We aren't affected, because visibility comes from the parent
  marker.
- S8 leftover: a cid at descriptor depth 16 with a 64-character row id is 204 bytes.
  10,000 repeated no-op `ensure_mergeable_text` calls took 46 ms.

## S4: 3-way text merge

**Pass for mini apps.** The experiment's code was `src/text3.rs` (transform, caret
mapping, unit tests) and `tests/s4_text.rs`; the transform now lives in the core's `text.rs`. Each case applies two random concurrent edit
sequences to a shared base, and three merges are compared with the **truth**: Loro merging
both sides' real ops.

- **today:** Loro merges the remote's real ops with the user's edit applied as a
  caret-anchored diff on a branch at `base` (the current design).
- **3-way:** strings only. Both sides are diffed and transformed, and the concurrent
  insertion goes first at a tie.

| Scenario (20,000 cases each) | today: identical / reordered / different chars | 3-way: identical / reordered / different chars |
|---|---|---|
| Typing, words (one keystroke or backspace per request) | 99.1% / 0.9% / 0.0% | 95.8% / 3.8% / 0.4% |
| Typing, alphabet `{a,b}` | 94.2% / 5.8% / 0.0% | 81.5% / 16.0% / 2.4% |
| Harsh, words (up to 3 random edits per side) | 94.7% / 3.8% / 1.6% | 89.5% / 7.4% / 3.1% |
| Harsh, `{a,b}` | 73.9% / 15.4% / 10.7% | 61.4% / 23.3% / 15.3% |

- "Reordered" means the same characters, with the user's and the remote's insertions at
  one spot in a different order.
- "Different characters" in realistic typing is almost always both sides deleting the same
  repeated letter. The string diffs can't tell the two copies apart, so one survives (an
  extra character, not a lost one).
- These rates apply only when another edit lands in the *same field* within one request
  round trip. Otherwise `current == from` and the edit applies exactly.
- The unit tests cover:
  - concurrent inserts elsewhere;
  - a delete that spans a concurrent insert, which keeps the insert;
  - same-point inserts (the concurrent one first, the caret after mine);
  - emoji and surrogates;
  - both sides deleting the same character.

Decision 1 holds. If co-typing in one field ever matters, the hybrid is to keep `base` only
as an optional hint and compute the concurrent side exactly with `LoroDoc::diff`. Nothing
else is needed for that.

## S2: UndoManager

**Pass**, 6 of 6, in the experiment's `tests/s2_undo.rs`:

- typing runs per field;
- an agent run, a five-step color drag and a command are one step each;
- undo keeps a remote edit in the same field, and redo reapplies over later remote edits;
- undoing a row removal restores the row with a concurrent remote rename;
- remote imports alone record nothing;
- the `set_max_undo_steps(100)` cap holds.

Owner rules this establishes:

- **Start a new run:** `group_end(); group_start()`.
- **Continue a run:** `group_start()`, ignoring `UndoGroupAlreadyStarted`. Loro silently
  closes a group when an import touches its containers, and this reopens it, so the run
  splits into two steps around the conflict.
- **A command or other non-run edit:** `group_end()` first.

## S7: Cloudflare limits (docs, 2026-10-09)

**Pass, and no deploy was needed.** From developers.cloudflare.com (Durable Objects limits,
the state API, the SQLite storage API, and Workers limits):

| Limit | Value | Design consequence |
|---|---|---|
| Inbound WebSocket message | 32 MiB | No Loro-style 256 KB fragmentation. Backfill frames are cut at about 1 MiB anyway |
| SQLite row, string or BLOB | 2 MB | Record bytes are split across `chunks(seq, part, bytes)` rows of 1.9 MB or less |
| Storage per object | 10 GB (Paid); 1 GB (Free) | Ample for mini-app logs |
| Isolate memory | 128 MB | Backfill streams from a cursor and never loads the whole log |
| CPU | 30 s per message (configurable up to 5 min) | Fine |
| WebSockets per object (hibernation) | 32,768 | Fine |
| `serializeAttachment` | 16 KiB | Holds only `{joined}` |
| `setWebSocketAutoResponse` | 2,048 characters each way; answered without waking the object | Text `ping`/`pong` keepalive |
| Output gate | Outgoing messages wait for prior writes to flush, by default | The ack follows a durable store. The spike also calls `storage.sync()` |

## S5: relay self-heal

**Pass.** There are two parts.

**Part 1, deterministic fuzz** (`tests/s5_protocol.rs`). It runs the real sans-IO session
(`src/engine.rs`) and an in-memory relay with the DO's rules. Every frame crosses the binary
codec (`src/wire.rs`). The repository's `crates/hitslop-core/tests/sync.rs` now runs the same
faults against the session that ships. The faults:

- dropped connections, which lose in-flight pushes and acks;
- relay restarts;
- client crashes that reopen from the last local save under a new session peer.

The run was `S5_SEEDS=5000`, 400 steps each: 699,073 edits, 17,220 drops, 40,051 relay
restarts and 79,775 crashes. Every replica converged, and every one equalled the relay's
heads. No edit that was saved locally or acknowledged by the relay was lost. The only edits
lost (61,531 crashes) were neither saved nor uploaded, which is the expected outcome. No
`gap` was ever needed. Client sync state is in memory only: each reconnect's Hello
recomputes everything. There is no outbox, cursor, record id or hash.

**Part 2, end to end** (`tests/s5_e2e.rs`, `--ignored`). Rust clients ran over tungstenite
against the TypeScript Durable Object in `relay/src/index.ts`, under `wrangler dev` 4.149.0
on this Mac, with persisted local state.

| Scenario | Result |
|---|---|
| Online typing: 200 commits while connected | Converged. The 2.86 s total is mostly the test's own 5 ms poll timeouts; it is not latency. One push in flight batches commits at RTT pace |
| B offline while both edit, then reconnects | Converged |
| Relay killed mid-session (DO restart, sockets drop), edits continue, relay restarted | Converged, and relay heads equal every replica |
| Backlog: 1,000 offline commits, then reconnect | **48 ms** until the other client has them. The old single-record path took 363.6 s for 1,000 edits |
| One 3 MB commit (stored across 2 MB-capped rows) | 247 ms to the other client |
| A fresh replica joins from the seed and backfills everything | 131 ms |

Final relay state: 304 records, 3.03 MB. The relay log had no errors. Hosted latency and
real network conditions are not measured here; that stays a launch qualification item.

**Part 3, the engine that ships, end to end** (`tests/product_e2e.rs`, `--ignored`,
2026-10-09). The same Durable Object, scenarios and helpers as part 2, under `wrangler dev`
4.149.0 on this Mac. The client is the code in hitslop-core: `hitslop_core::sync::Session`
over `hitslop_core::Document` (built without storage), with remote changes imported by
`Document::import_remote`, so they pass the app's acceptance and publish like edits. Each
replica opens from the seed as a new session peer. Edits are row inserts and text typed into
a title with `from`, as the page sends them. Pass means every replica reads the same value,
every session reports `Synced`, and the relay's heads equal the replicas' version.

| Scenario | Result |
|---|---|
| Online typing: 200 edits while connected | Converged in 2.79 s, mostly the test's 5 ms poll timeouts |
| B offline while both edit, then reconnects | Converged |
| Relay killed mid-session, edits continue, relay restarted | Converged, and relay heads equal every replica |
| Backlog: 1,000 offline edits, then reconnect | **57 ms** until the other replica has them |
| One 3 MB commit (an agent's title, stored across 2 MB-capped rows) | 192 ms to the other replica |
| A fresh replica joins from the seed and backfills everything | 131 ms |

Final relay state: 303 records, 3.2 MB. The wire needed no change: the spike relay and the
shipped codec agree byte for byte. This closes the sync proof (the binary relay on
Cloudflare's runtime, with the engine that ships). The production relay and the clients are
deferred ([roadmap](../roadmap.md)).
