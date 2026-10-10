# Native authority proof and foundation continuation

> **Superseded 2026-10-09.** The authority proof (`owner/sync.rs`, `slop-room`, `dev-sync`) and live history maintenance were removed; their commands no longer run. Sync is now local owners and a byte relay: see [sync spikes](sync-spikes-2026-10.md).

This continues the [first foundation qualification](launch-foundation-2026-10-07.md)
on `feat/launch-foundation`, based on master `8dec1847`. Changes are uncommitted.
It implements the local authority proof and measured local history maintenance. It
does not enable production sharing or freeze a release.

## What runs

`bun scripts/dev/live-sync.ts examples/slops/quick-checklist` builds the opt-in Rust
engine, creates an authority and two durable SQLite replicas, and prints two Vite
URLs. Each page uses the ordinary SDK and preview owner bridge. Page and CLI mutations
forward to the authority; accepted Loro updates return through each replica's serial
owner, validation, publications and persistence. No CRDT bytes reach JavaScript or
Swift. The regular engine refuses the development entry points.

The private transport checks a session credential, document UUID, app/assets digest
and layout. Large payloads use bounded chunks with a digest and deadline. Callbacks
enqueue output without waiting for sockets; both frame count and retained output bytes
are bounded. Bootstrap backups preserve identity and history through the owner's
durable save barrier, rather than the temporary capture-copy path.

The two-view test exercises alternating commands, concurrent CLI increments and
commands, concurrent text, theme changes, row identity, duplicate and delayed delivery,
a dropped update followed by automatic checkpoint recovery, explicit snapshot
replacement, disconnected writes and DOM draft retention. All three files must agree
in value and version and independently reopen after their owners close. Separate
native rendering evidence covers a keyed 4,000-row Svelte list and command publications.

The expanded Svelte conformance fixture covers keyed text deletion during composition,
recreation and detached input events; simultaneous row submissions and moves; nested
row lists in exports; frame-by-frame previews followed by one undoable gesture; and
an atomic counter command with a companion field. Existing Swift and compatibility
tests execute it through the public SDK.

## Corrections found during integration

- An eight-request CLI burst overwhelmed the initial eight-frame output queue. It now
  admits up to 128 frames within a 64 MiB byte budget, including headers and the frame
  currently being written. Presend refusal is distinguishable from an unknown outcome.
- A replica that cannot install an already accepted authority update must report an
  unknown outcome and fence writes, rather than claim the authority rejected it.
- Shared-role configuration is refused while local maintenance owns the barrier.
- RetryFlush can recover an earlier accepted save that fails before maintenance has
  prepared a candidate. Otherwise the retry itself could wait forever behind that
  barrier. The regression failed before the admission fix.
- Preview output previously waited for a 50 ms poll interval when an owner callback
  enqueued a reply. A nonblocking wake socket now wakes that poll immediately; slow
  readers retain the existing output deadline and bounded queue.

## History maintenance

The [isolated owner benchmark](owner-history-2026-10-08.md) includes 20 checkpoint
cycles and 5,140 durable saves with constant live content. Retained history fell from
86.0 MB to 2.18 MB; peak RSS fell from 1.03 GB to 404 MB. Ordinary checkpoint p95 fell
from 482 ms to 307 ms. Maintenance separately holds later mutations for roughly one
second. These are measured outcomes, not a general RSS bound or a zero-pause claim.

Only a persisted, validated candidate replaces the live document. A failed or
ambiguously acknowledged replacement retains the original owner, candidate and exact
save job for retry. Sequence, writer peer, view and visible identities survive;
supported undo remains usable. Shared owners do not run the local retention policy.

## Live latency and rendering measurements

Focused native run `2026-10-08T07-20-28-112Z-59685` passed both tests and 25 assertions.
On arm64 macOS 26.6.2, using release native engines and development Vite pages:

| Workload | p50 | p95 | Samples |
|---|---:|---:|---:|
| Shared command and durable flush | 38 ms | 48 ms | 10 after one warmup |
| Shared bound text and durable flush | 15 ms | 43 ms | 10 after one warmup |
| Local 4,000-row command, flush and two animation frames | 99 ms | 101 ms | 5 after one warmup |

Both shared round trips meet the proof's p95-under-100-ms target in this run.
An earlier run before the preview wake change measured command p50/p95 106/136 ms
and text 107/115 ms. This is a small loopback sample, not a WAN or service-level claim.
One of two concurrent command calls was accepted; the other was definitely refused
by the existing command admission rule. Eight simultaneous increment batches all
counted. A subsequent edit proved the definite refusal did not fence the replica.

The keyed list's first rendered frame took 325 ms, measured from Svelte component
creation through two animation frames, excluding Vite compilation. Six additions
left 4,006 rows and retained the first row's DOM identity. The test records timings
without flaky performance assertions.

Raw measurements: [live round trips](live-sync-timings-2026-10-08.json) and
[large page](large-preview-timings-2026-10-08.json). Two-view PNGs are retained in
`.hitslop/evidence/runs/2026-10-08T07-20-28-112Z-59685/live-sync-{a,b}.png`.

The final full-suite run repeated these tests on the final source. Shared command
p50/p95 measured 25/28 ms and text 12/14 ms. The large page's first frame took 281 ms;
command-through-frame p50/p95 was 100/150 ms. The median agrees with the focused run,
but five samples do not establish a stable tail. The 101–150 ms p95 spread warrants
a larger, phase-separated projection/evaluator/render profile before optimizing this
path. No representative latency assertion was weakened. The ordinary native-preview
test measured 40 fresh evaluations at p50/p95 14/21 ms, versus 53/54 ms in the first
foundation checkpoint. [Final timing samples](foundation-native-timings-2026-10-08.json).

## Final qualification

`bun run verify` passed in `2026-10-08T07-31-15-155Z-69656`. The development corpus
was refreshed once more after the mounted-draft fix, including all 13 page scenarios.
`bun run verify --native` then passed in `2026-10-08T07-41-51-144Z-82906`; its report
has `certifiesDefaultRun: true`. The source remains uncommitted at HEAD `8dec1847`.

| Boundary | Final qualification |
|---|---|
| Ordinary Rust workspace | 312 passed; 4 child helpers ignored as standalone tests |
| Core/engine with `dev-sync` | 329 passed; same 4 child helpers; overlaps the ordinary suite |
| SDK/shell | 86 passed |
| CLI | 88 passed |
| Swift | 198 passed, all three shards complete |
| Native | 84 passed, 1 existing host-process-kill skip |
| Packed packages | 4 passed; native/global-install case skipped in the default run |
| Types, contracts, landing, corpus integrity | Passed |
| Tooling | Unchanged 22-test qualification reused by the verifier |

The four ignored Rust entries are subprocess helpers invoked by passing parent tests
for lock and crash behavior; they are not missing crash coverage. The native host-kill
case requires the release app binary. This was not `release:check` or a public release
freeze. Only replaceable `tests/compat/dev` was regenerated. Final logs, screenshots and
the complete report are in `.hitslop/evidence/runs/2026-10-08T07-41-51-144Z-82906/`.

## Deliberate limits

Room binding and credentials exist only during the process lifetime. Stopped harness
files are unsupported shared artifacts. There is no durable request receipt, restart
fence, automatic mutation replay, offline merge, attachment transfer or personal shared
undo. Duplicate accepted updates are idempotent; that does not make commands exactly
once. Production sharing remains Phase 4b and needs its storage-2 design.

Disconnected mounted text drafts are tested separately from expired history bases.
A new shell regression failed because flush succeeded after silently replacing an
expired-base draft. The fixed binding keeps the mounted draft, stops replay, refuses
flush/close, and offers Escape to discard it and adopt current accepted text. The
37-case owner-shell suite passed, including IME Escape protection, remote changes,
newer unsent typing and a deliberate new edit after discard. Unresolved drafts can
still be lost when their control unmounts; recovery outside that control remains open
in the roadmap.
