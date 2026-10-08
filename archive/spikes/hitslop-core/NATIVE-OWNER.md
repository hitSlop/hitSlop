# Rust owner in a real WKWebView host

Isolated experiment, 2026-09-28 local / 2026-09-29 UTC. **No production cutover.**
The binding foundation is recorded separately in [REPORT.md](REPORT.md).

## Current status (2026-09-29)

| Gate | Status | Evidence |
|---|---|---|
| S1 latency / render / drain / 50 ms | Pass, fastest candidate in every cell | Second iteration below |
| S1 memory (+10% vs Swift native) | Total misses by 0.4% at 1k × 1 window; attributed to WebContent, host is smaller | Second iteration, open items |
| Publication correctness | Pass, including other roots, plain lists, replaced containers, nested lists | Third iteration |
| S3 one CLI edit path + cold start | Pass: 9/9 race and kill scenarios; 1k cold apply 20 ms p95 | Third iteration |
| S4 counters | Pass: exact convergence where Loro Counter fails | Third iteration |
| S2 system IME | **Open** (manual; needs a Japanese/Chinese input source) | — |
| End-to-end 2 ms publication budget | Unproven: core 0.002 ms, remaining ~2.5 ms is bridge + renderer | Production-SDK measurement |

## Third iteration: hardening, one CLI edit path, counters (2026-09-29)

### Publication hardening

An external review found two bugs, both reproduced before fixing:
- **Other Loro roots leaked into the projection.** An imported `unrelated.done = true`
  published `set ["done"]`, while the snapshot still read `done: false`. Publications
  now keep only events under the `data` root.
- **A plain `LoroList` panicked on its next update** (`get_movable_list` on a List ID).
  Only movable lists carry row identity; any other list now publishes exactly
  through the fallback.

New coverage, each case named with an independent oracle (patch consumer + fresh
snapshot values and issues):
- `core/tests/chaos.rs`: a raw `LoroDoc` peer makes edits that local writes refuse:
  deleted fields, wrong types, replaced `rows` and `text` containers, plain values
  and plain lists, writes to other roots, duplicate and rewritten `$id`s, concurrent
  move/remove. 300 rounds × 40 imports.
- `core/tests/nested.rs` with `fixtures/nested.json`: rows with a nested list and an
  object field, local and multi-edit remote imports. A deterministic case shows that
  a nested edit plus removal of its row in one import publishes exactly one
  `deleteRow`: Loro omits events for containers removed by the same change. The
  earlier "deepest list first" event sort was therefore not load-bearing (reversing
  it changed nothing) and was removed.
- `conformance.rs` `owner_keeps_working_after_a_late_rejection`: after a late
  rejection rebuilds the owner, local edits, remote imports, both draft paths,
  incremental replay and checkpoint reopen all still work. It fails if the rebuilt
  owner is not resubscribed.

### S3: one CLI edit path (`Sources/OwnerService`, `slop-spike`, `owner-host`)

Whoever holds `writer.lock` runs the Rust owner. A live owner (`owner-host`, the app
stand-in) listens on a Unix socket and publishes a discovery file only once it is
accepting. The CLI (`slop-spike get|apply`) first tries to take the lock:
- If it succeeds, it owns the closed document in-process: open, apply, durable flush,
  release. No socket, no WebKit.
- If the lock is busy, it forwards to the discovered socket.
- While an owner is starting or closing (no discovery file, refused connection, or a
  `closing` reply), it retries with bounded backoff for up to 2 s.
- If the owner fails to reply after a request was sent, the CLI reports
  `unknown_outcome` ("run get before another edit") and never resends.

Close order is: stop accepting → mark closing on the owner queue → flush → remove
discovery → release the lock. `bun spikes/hitslop-core/cli-races.ts` uses counter
increments, so "applied exactly once" is checkable. Evidence is in
`.hitslop/v1-evidence/hitslop-core/cli-races.json`.

| Scenario | Result |
|---|---|
| Closed document, 20 sequential applies | All owned on the first attempt; count 20; no discovery file left behind |
| Open document, 30 applies (5 concurrent at a time) while the host edits every 5 ms | All forwarded; count 30; host edits durable |
| Host holds the lock before listening | CLI retried (≈12 attempts), then forwarded |
| Host shutting down (slow release) | CLI retried (≈11 attempts), then owned after release; nothing lost |
| Two CLIs at once, 20 rounds | 40/40 applied: 20 owned first, 20 owned after retry |
| SIGKILL the CLI owner paused before COMMIT, 100× | Lock released by the OS every time; no trace of the uncommitted save |
| SIGKILL the CLI owner at random times, 100× | Always old or new state (≈25 landed, ≈75 killed first), never torn |
| SIGKILL the host mid-edit | Stale discovery ignored; next CLI owns on its first attempt; durable host edits kept |
| SIGKILL the host after applying, before replying | CLI reports `unknown_outcome`; `get` shows the applied edit exactly once |

`writer.lock` keeps the same inode in every scenario: it is never unlinked or
bypassed. Sensitivity: making a lost reply resend instead of reporting
`unknown_outcome` applies the edit twice, and the last scenario fails.

Cold apply on a closed document: process spawn → lock → load → open → apply →
durable flush (`synchronous=EXTRA`, `fullfsync`) → exit, 20 fresh processes each:

| Rows | Wall p50 | Wall p95 | Lock | Open | Apply | Flush |
|---:|---:|---:|---:|---:|---:|---:|
| 1 | 13.8 ms | 15.7 ms | 3.5 | 0.8 | 0.06 | 2.3 |
| 1,000 | **18.3 ms** | **20.3 ms** | 3.6 | 4.1 | 0.06 | 3.0 |
| 5,000 | 41.5 ms | 62.5 ms | 3.8 | 24.6 | 0.06 | 4.5 |
| 40,000 | 289.7 ms | 409.8 ms | 4.1 | 264.0 | 0.06 | 6.9 |

The 150 ms target at 1k rows passes by about 7×. Large documents are open-bound: at
40k rows the cost is Loro's first full decode plus the index and issue walks.

### S4: counters

`kind: "counter"` is stored as a Loro map of writer key → `i64` contribution and
projects to the sum. The writer key is the session's Loro peer ID, so no key ever has
concurrent writers and contributions merge without loss or float rounding.
`increment {path, by}` requires a nonzero safe integer and rejects any result
outside ±(2^53−1). There is no `set`: a UI reset is an increment by −current.
Anomalous contributions, and concurrent sums that leave the safe range, are
preserved raw and flagged `type_mismatch`; local increments on them reject. The
TypeBox wire gained `increment` (regenerated); both binding fixtures include counter
scenarios.

`core/tests/counters.rs`:
- **Loro Counter reproduces** the regrouping failure (accepted 1, replayed 0 for
  `(1e16 − 1e16) + 1`). It also produces a wrong value from individually safe
  increments whose running float sum crosses 2^53.
- **The hitSlop counter stays exact.** It rejects the increment that would leave the
  safe range, and live, replayed and reopened values agree at the boundary.
- **200 rounds with three replicas** under random, duplicated and delayed delivery
  converge to the exact integer sum. Every publication reconstructs the snapshot.
- **Restored checkpoints and Finder copies** keep every increment after merge.
- Sensitivity: a shared writer key (what a persisted per-device key does to Finder
  copies) fails the convergence, copy and overflow tests.

Known limit: the map grows by one key per session that increments. Compaction is
deferred.

## Second iteration: change-proportional core (2026-09-29)

**Result: every latency, render, drain and 50 ms gate passes; the Rust candidate is
now the fastest in every cell, and all three 40k stress cells succeed.** One gate
still fails: memory at 1k rows × 1 window is 109.6 MiB versus 99.3 MiB Swift native
(+10.4% against a +10% limit). S1 therefore remains formally open on that gate.

Frozen matrix `.hitslop/v1-evidence/engine-placement/2026-09-29T04-21-52.104Z`,
99 runs (5 trials per ordinary cell, 1 per stress cell), same seeds, view, periodic
saves and rotation as before. Medians of per-process acceptance p95:

| Rows | Windows | Checkbox Rust / Swift / WASM | Move Rust / Swift / WASM | Memory MiB Rust / Swift / WASM |
|---:|---:|---|---|---|
| 1,000 | 1 | 3 / 5 / 8 | 2 / 4 / 11 | 109.6 / 99.3 / 152.0 |
| 1,000 | 10 | 3 / 5 / 7 | 3 / 4 / 12 | 413.3 / 390.0 / 743.7 |
| 5,000 | 1 | 3 / 10 / 13 | 3 / 10 / 32 | 114.0 / 104.3 / 207.8 |
| 5,000 | 10 | 3 / 8 / 14 | 3 / 10 / 36 | 459.6 / 423.1 / 926.3 |
| 40,000 | 1 | 5 / 27 / 65 | 6 / 32 / 233 | 159.7 / 178.3 / 702.5 |
| 40,000 | 10 | 4 / 28 / 73 | 7 / 32 / 246 | 983.2 / 976.4 / 2192.3 |

The previous Rust run measured 39/45 ms (checkbox/move) at 5k and failed every 40k
cell. Full table: [evidence/native-table.md](evidence/native-table.md).

### What changed

Step-1 attribution on the old core (`examples/cost_attribution.rs` at that time):
one 5k checkbox cost 36.6 ms p95, of which full JSON materialization was ~9 ms and
ran twice; `fork()` was 1.7 ms at 5k and 15 ms at 40k; the 40k checkbox took 543 ms.

- **No per-edit fork.** Intents are fully validated before their first mutation and
  apply to the owner directly. A rejection after earlier intents mutated rebuilds the
  owner with `fork_at(pre-batch frontiers)` and a fresh peer (O(document), only on
  that path). `sensitivity.ts` now removes that recovery; `atomic_rejection` fails.
- **Publications from Loro events.** `subscribe_root` events (synchronous after
  commit/import) name each changed container and a typed diff. Map updates → `set`,
  text → `set` of that text, movable-list deltas → `deleteRow`/`insertRow`/`moveRow`.
  Changed rows are placed after their final predecessor, which provably reproduces
  the final order without a list diff; a defensive check falls back to an exact
  list `set` and never fired in 100k randomized steps.
- **Persistent list indexes** (order + `$id` lookup) are updated from the same
  deltas; lookups within a batch scan only lists that batch already changed.
- **Incremental validation** while the document has no issues; documents with issues
  rescan on publish (anomalies only arise from merges). Anomalous lists publish
  exactly via fallback.
- **Typing fast path.** A draft edits the owner directly while the owner is still at
  the draft's last accepted version; the authored branch is materialized with
  `fork_at` only when other edits intervene.
- **Imports** check dependencies from the blob header before Loro buffers anything.

Native cost per operation (`examples/cost_attribution.rs`, p95, excludes FFI/bridge):

| Operation | 1k | 5k | 40k |
|---|---:|---:|---:|
| Checkbox | 0.025 ms | 0.025 ms | 0.029 ms |
| Title / row text splice | 0.02 ms | 0.02 ms | 0.02 ms |
| Insert / remove / move row | 0.05–0.07 ms | 0.18–0.22 ms | 1.1–1.3 ms |
| Remote checkbox import | 0.13 ms | 0.35 ms | 2.5 ms |
| Remote 100-edit import | 0.64 ms | 1.0 ms | 1.3 ms |
| Full snapshot (allowed O(n)) | 1.7 ms | 10.4 ms | 196 ms |
| Open (index + issue scan) | 14 ms | 26 ms | 333 ms |

Field and text edits are flat. Structural edits are linear in list length (vector
bookkeeping and predecessor scans) but small. Publication construction alone is
0.002 ms p95 at 5k (was 30.6 ms). The window harness's `patchUpperBound` diagnostic
is now 2.5–3.2 ms at 5k, which is bridge round trip and renderer application, not
the core; the 2 ms diagnostic therefore still reads `false`.

### Correctness evidence for this iteration

- All Rust tests pass. The randomized test now also imports one to four remote
  edits per publication and asserts clean lists never publish a whole-list `set`.
  Deliberately placing rows at their final index instead of after their predecessor
  fails it; removing fallback publication fails the new merged-duplicate-ID test.
- Swift/UniFFI and Bun/WASM fixtures, native↔WASM byte replay, renderer tests,
  renderer sensitivity, strict TypeScript and the real-WebView correctness gate pass.

### Open items from this iteration

- **Memory gate (1k × 1 window): attributed, not the core.** The harness now also
  records host and WebContent footprints separately (`hostMiB`, `contentMiB`; the gate
  still uses the total). Five fresh processes per candidate at 1 window
  (`.hitslop/v1-evidence/engine-placement/memory-attribution-20260929T080342`):

  | Rows | Host MiB, Rust / Swift (median) | WebContent MiB, Rust / Swift (median, range) |
  |---:|---|---|
  | 1,000 | **25.0 / 27.0** | 80.1 (73.1–101.3) / 73.7 (73.2–74.0) |
  | 5,000 | 38.8 / 36.3 (ranges overlap) | 76.9 (76.4–81.9) / 72.9 (56.3–77.8) |

  The Rust host is no larger than the Swift-native host; the difference sits in the
  WebContent process and swings ~28 MiB between identical runs. The Rust renderer
  adapter (`renderer.ts`, `text-binding.ts`, `patch.ts`) keeps no growing state, so
  this reads as WebKit heap/GC timing rather than retention. The production SDK
  replaces this adapter; re-measure it there rather than tuning the spike adapter.
  `examples/memory_probe.rs` separately showed the Loro document is not the cause.
- **Open cost.** The open-time issue scan now walks Loro containers instead of
  materializing JSON: 40k open 333 → 228 ms (1k 14 ms, 5k 19 ms). A breakdown at 40k:
  import 10 ms (lazy), first row walk ~93 ms (Loro decodes state on first access),
  each further walk ~60 ms. Open does two walks (list index, issues); merging them
  would approach the ~100 ms decode floor. Not worth the complexity at this size yet.
- Unchanged: system-IME manual gate, full descriptor/derived-ID parity, CLI
  forwarding/races, counters, durable receipts.

## Historical: first integration (2026-09-28)

Everything below records the first WKWebView integration and its matrix
(`2026-09-29T03-19-50.285Z`). Its performance numbers and "does not pass" verdicts
describe the superseded fork-per-edit core, not the current one.

## Architecture implemented

The Rust-only executable links `hitslop-core` through generated UniFFI bindings.
Its WKWebView runs the shared Svelte view, an immutable patch projection and DOM
text drafts. Its asset handler refuses Loro/WASM requests. Native and WASM are
separate deployment targets; there is no renderer CRDT replica in this candidate.

The Swift-native control and Rust candidate compile the same harness source into
separate executables. SQLite storage and JSON utilities live in the engine-neutral
`placement-support` package. The Rust Swift package does not depend on `loro-swift`.
Swift serializes access, holds the OS writer lock, persists opaque bytes in the
existing format-2 envelope and supplies a native save-error/Retry Save control.
Acceptance is in memory; periodic 200 ms autosave and explicit flush establish
SQLite durability. No tests open user packages or execute authored application code.

Rust retains staged atomicity, adds cached row-ID indexes and publishes field,
insert, delete and move patches. The renderer preserves unaffected row objects;
missing sequences and changed owner sessions obtain a fresh snapshot. Anomalous
lists fall back to exact replacement rather than modifying stored data. Full
production derived-ID projection and issue vocabulary remain unported.

Text requests carry owner session, draft ID, sequence, base, parent, UTF-16 splice
and selection. Each draft retains its authored branch separately from the merged
owner. Rust merges only its authored delta into the latest staged owner and maps
Loro Unicode cursors back to DOM UTF-16 selection offsets. The renderer sends one
request at a time per binding and sends nothing during composition. Failed
submissions retain their DOM draft; no blind retry occurs after an owner change.
An explicit `command_current` call interprets offsets at owner-queue execution;
it refuses a supplied authored base. This is a spike command entry, not production
CLI socket forwarding. Renderer detach retires its active draft identities.

## Correctness evidence

- Twelve Rust tests, including 100,000 deterministic valid local/import steps,
  independent literal semantics, exact patch reconstruction and issue comparison.
- Twelve fixtures through Swift/UniFFI and twelve through Bun/WASM; native/WASM
  checkpoint and incremental-update bytes replay in both directions.
- Six renderer tests cover structural sharing, publication gaps/session changes,
  surrogate boundaries, retained failed composition, input during draft release and
  delayed resynchronization that must not overwrite newer publications.
- Three Rust-owner Swift tests exercise real SQLite failure/retry, lock retention,
  ambiguous committed writes and native reopening. Ten existing Swift-native
  control tests remain intact after sharing the Store.
- The WebView gate covers 0/20/100/500 ms reply delays, concurrent owner commands,
  exact text/caret outcomes, Unicode edits, synthetic composition, focused-row
  removal, exactly one affected row component and save failure/retry. It also
  exercises real AppKit keyboard input and focused responder undo/redo.
- Normal renderer replacement drains and flushes. Forced replacement occurs after
  native acceptance but before the delayed reply. Both require the old WebView's
  weak reference to clear and the new view to display the retained owner's state.
- Atomicity sensitivity bypasses fork staging in a disposable crate; the owner
  regression detects partial mutation. A separate renderer sensitivity removes
  the release-gap pump and detects stalled input. Neither changes actual source.
- Repository check passes; the standard suite passes 154 tests, 115 compatibility
  replay cases and 52 bundled-template open/reopen checks. Production native and
  release suites are not claimed because their source/targets remain unchanged.

The actual Japanese/Chinese system-IME gate remains **unverified**. Enabled sources
on this Mac were Dvorak, U.S. and the character palette. Synthetic composition and
AppKit keyboard/undo tests do not establish system-language IME behavior.

## Findings caught during integration

1. The first Rust milestone recognized only its 32-character hex fixture IDs.
   Production mints 26-character Crockford IDs and accepts safe 1–64 character
   stored IDs. That mismatch forced full-list publications. Validation/minting now
   follows `packages/document/src/identity.ts`; derived anomaly IDs remain deferred.
2. Loro version-vector encoding serializes a hash map. The randomized reopen/import
   test exposed unequal byte tokens for equivalent histories. The spike now emits
   canonical version tokens rather than treating raw encoded map order as identity.
3. Reading a base in one bridge call and applying a command in another races pending
   text acceptance. The current-state command entry chooses its base and stages the
   edit within the same serialized owner call. Drafts still use authored ancestry.
4. Input can arrive while a completed draft's release call is pending. The binding
   now pumps that new draft after release and drains through the entire queue.
5. A delayed gap-recovery snapshot could overwrite a newer publication. The
   regression first reproduced `two` instead of the independently expected `three`;
   recovery now checks owner session and sequence before replacing its projection.
6. Renderer teardown clears the focused native responder and undo actions before
   replacing the WebView; the old view must actually deallocate.
7. The sensitivity crate originally shared Cargo's target cache with normal tests,
   allowing a deliberately broken test artifact to be reused. Its target directory
   is now isolated; normal spike artifacts were cleaned and rebuilt.

## Measurements and decision

**Result: native ownership works for the tested flows; the Rust prototype does not
pass the performance gates.** The final matrix contains **99 recorded cells**:
90 ordinary-size successes, six control stress successes and three Rust stress
failures. See [the table](evidence/native-table.md) and [per-process evidence](evidence/native-results.json).

At one window, medians of the five per-process acceptance p95s are:

| Rows | Checkbox: Rust / Swift native / matched WASM | Move: Rust / Swift native / matched WASM |
|---:|---:|---:|
| 1,000 | 9 / 2 / 6 ms | 10 / 3 / 8 ms |
| 5,000 | 39 / 5 / 14 ms | 45 / 5 / 32 ms |

- Relative acceptance fails against the freshly measured Swift-native control in
  every ordinary scenario/cell. Most 5k render-opportunity comparisons also miss
  matched WASM + 16.7 ms.
- The every-run 50 ms checkbox/move gate fails: one 5k/single-window move run has
  an **82 ms p95**, despite its five-run median being 45 ms.
- Memory passes the +10% Swift-native limit in all six ordinary cells. At 5k/10
  windows, median host + WebContent footprints are **425.5 MiB Rust, 425.8 MiB Swift
  native and 918.4 MiB matched WASM**. Sustained typing drains within 15 ms in every
  ordinary Rust run and meets the periodic-save assertion there.
- All three 40k Rust stress cells fail the requirement for at least eight periodic
  saves during sustained typing. Failed processes are retained as failures; their
  missing timings/memory are not averaged into successful runs.
- A separate 20-edit native-core probe measures **publication construction alone**
  at **7.57 ms p95 for 1k rows and 30.61 ms p95 for 5k**. It excludes response
  encoding, FFI, bridge transport and renderer application. This lower bound alone
  fails the 2 ms publication budget and identifies work inside the core.

**Next step:** retain UniFFI and the Rust candidate, and replace full-value
materialization/subtree scanning in publication with updates driven directly by
Loro container diffs. Keep the independent patch/issue property test and literal
fixtures as the guard. Then rerun these frozen workloads before extending the
production integration. These results compare implementations, not intrinsic
Rust-versus-Swift language performance.

Raw evidence is in
`.hitslop/v1-evidence/engine-placement/2026-09-29T03-19-50.285Z/`.
Incomplete calibration runs are marked discarded. Two interrupted attempts were
archived and repeated from fresh SQLite directories with identical frozen inputs;
resuming no longer reuses a partially edited database. The compact evidence records
those reruns and the individual process samples. The source commit in environment
metadata is informational because the spike was in the working tree; frozen binary,
renderer and seed hashes identify the actual measured inputs.
The matrix freezes executable and browser bytes, rotates matched WASM, Swift-native
and Rust-native processes, and runs five trials at each 1k/5k × 1/5/10-window
ordinary cell, plus one exploratory run per candidate at each 40k × 1/5/10-window
stress cell. All candidates render the same 40-row view and assert at least eight periodic
saves during sustained typing. Two animation frames measure render opportunity,
not compositor paint. Memory includes the host and identified WebContent processes,
excluding GPU/network helpers. The 40k cells are stress results.

The field named `patchUpperBound` estimates native patch construction plus bridge
round trip minus total native apply time, plus projection/application. It includes
request transport and queueing but excludes native response encoding counted
inside apply time. Treat it as a diagnostic estimate, not a precise isolated
transfer measurement or proof that the 2 ms target passes. Native patch construction itself still materializes full
JSON values and scans changed subtrees; small patches alone do not prove cheap
publication.

S1 and S5 are **not passed**. S2 has automated checklist evidence but its system-IME
gate remains open. Full
semantics, derived IDs/issues, resource bounds, durable request outcomes, real CLI
forwarding/races, process-kill recovery and the system-IME gate remain explicit work.
The SQLite schema, production synchronous API, released runtime bytes, templates,
client, counters and deferred sync remain unchanged.
