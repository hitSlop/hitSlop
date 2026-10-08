# Local-owner history maintenance, 2026-10-08

The actual Owner/Store workload crossed the measure-first gate: its durable file
remained small while its live Loro history and process memory kept growing. The
implemented local-owner maintenance bounds retained history in this workload, keeps
usable undo, and survives durable reopen. It adds a periodic admission pause; these
results do not claim interruption-free editing or rendered-page responsiveness.

## Method and raw results

Run `cargo run --locked --release -p hitslop-core --features dev-sync --example
bench_owner_history` on arm64 macOS 26.6.2. The diagnostic feature exposes measurements;
the maintenance behavior itself is enabled for ordinary local owners. Other builds,
tests and browser work were paused during sampling. These are single runs of the
uncommitted foundation tree, not a statistical comparison across machines.

The fixture has 4,000 rows with nested lists and text, a record and optional text.
Each of 2,570 deterministic rounds creates and deletes 16 KiB of random ASCII while
keeping live content constant. Every mutation is explicitly flushed: 5,140 durable
saves spanning 20 normal checkpoint cycles. Fixture creation and semantic reopen
validation run in separate child processes. RSS includes temporary export, fork and
candidate buffers in the measured owner process. The seed is 42.

- [Before maintenance: raw 20-cycle results](owner-history-baseline-2026-10-08.json)
- [Final implementation: raw 20-cycle results](owner-history-after-2026-10-08.json),
  core build `6843ff4be6175f22`
- [Eight-cycle worker-fork experiment](owner-history-fork-experiment-2026-10-08.json)
- Harness: `crates/hitslop-core/examples/bench_owner_history.rs`

| Measurement | Before | Final implementation |
|---|---:|---:|
| Final full retained snapshot | 86,045,753 B | 2,177,718 B |
| Final current-state snapshot | 511,887 B | 511,941 B |
| Final saved checkpoint at cycle 20 | 511,887 B | 2,177,718 B |
| Peak owner process RSS | 1,028,210,688 B | 404,422,656 B |
| Normal checkpoint flush p95 / max | 482.38 / 512.94 ms | 307.45 / 332.39 ms |
| All ordinary flushes p95 | 4.81 ms | 5.28 ms |
| Edit completion p95 | 4.48 ms | 3.74 ms |
| Measured loop elapsed | 42.94 s | 43.75 s |

The saved checkpoint now retains the supported undo window rather than only the
current state. Tiny current-state encoding differences do not represent a content
change: every replacement compares native values with container IDs, version vectors
and theme; child processes compare the final application value before and after close.

The final run rebuilt at cycles 4, 8, 12, 16 and 20. After the first rebuild, full
retained snapshots repeat approximately 2.18, 6.47, 10.72 and 15.00 MB, then return to
2.18 MB. Peak RSS reached 404.42 MB at cycle 12 and remained there through cycle 20.
This demonstrates a plateau over the measured run, not a general memory bound for
arbitrary documents. Real Undo and Redo each emitted a publication after the final
maintenance and restored the original live value.

## Admission pauses and read availability

Normal checkpoint-flush numbers above exclude the separate maintenance fence. New
mutations remain queued behind maintenance until its candidate is persisted and
installed. An immediate State request reads the old authoritative document while
the worker prepares; the following Flush measures the remaining maintenance wait.

| Maintenance cycle | Immediate State request | Remaining maintenance fence |
|---|---:|---:|
| 4 | 24.19 ms | 837.48 ms |
| 8 | 73.56 ms | 1,014.96 ms |
| 12 | 77.86 ms | 1,032.23 ms |
| 16 | 88.15 ms | 1,025.83 ms |
| 20 | 76.37 ms | 1,047.30 ms |

These samples measure owner projection, not WebKit frames. They do not establish the
worst possible read latency at every instant during preparation. The worker first
forks the immutable native source so historical export uses independent Loro locks;
the initial fork still briefly shares the source locks. In the eight-cycle diagnostic
run, actor seed creation took 0.016–0.018 ms and worker forks took 10.60 and 88.54 ms.
The preceding reference-clone-only experiment stalled its sampled reads for about
420–435 ms. Keeping the worker fork improved those samples to 26 and 83 ms, at the
cost of temporary memory and approximately 280 ms more total maintenance in cycle 8.

An earlier implementation also spent about 3.4 seconds exporting an intermediate
session-only checkpoint after previous rebuilds. A local-owner job now reuses its
already-exported full snapshot when it crosses the soft maintenance threshold and
fits the unchanged hard storage limit. The worker then persists the trimmed candidate.
The final 20-cycle run includes repeated rebuilds and no recurrence of that checkpoint
spike. Direct Store clients and development shared owners keep their conservative
checkpoint policy.

## Policy and correctness coverage

The normal save path measures full retained size only when it already exports a
checkpoint. Append jobs do not export full history for this trigger. Maintenance starts
above 16 MiB, or above the last rebuilt size plus 4 MiB when that is larger. A candidate
retains the supported undo/redo window if it fits; otherwise it expires redo first,
then halves the oldest undo prefix until it fits. Current state may exceed this soft
target but must fit the existing hard storage bound. Expired text bases remain stale;
maintenance never loosens the previously accepted text floor.

The actor freezes mutations and sends a private read-only seed to the existing
persistence worker. It retains the old authoritative Document throughout preparation
and persistence. Full stored validation and state/container-ID/theme equality precede
the forced checkpoint. Only a confirmed successful write installs the candidate and
resets Store accounting; an ambiguous write retains the exact save job for retry.
Writer peer, view, publication sequence, document UUID and app remain unchanged.
Development shared owners skip this local retention policy.

Regression coverage includes preserved peer/state/sequence/undo/redo; expired oversized
history; retained undo after a stricter saved text floor; same-version forced checkpoint
and exact-job retry; pending import rollback; bounded queued edits; failed maintenance
saves; and RetryFlush while admitted work fails before candidate preparation. The
combined core/owner run passed 57 tests before this final benchmark, including all
three maintenance unit tests. The root verification run remains the authority for
whole-repository qualification.
