# Engine placement spike

An isolated experiment comparing the current document runtime in WebKit with a
native Loro owner and an asynchronous authoring boundary. It is **not a replacement
runtime**, and its native adapter does not yet implement the complete SDK contract.
See [the decision and coverage report](REPORT.md) before using the measurements.

## Reproduce

From the repository root, with the root Bun dependencies already installed:

```sh
bun install --cwd spikes/engine-placement --frozen-lockfile
bun spikes/engine-placement/prepare.ts
bun spikes/engine-placement/generate.ts --check
bunx tsc -p spikes/engine-placement/tsconfig.json
bunx svelte-check --workspace spikes/engine-placement --tsconfig ./tsconfig.json
bun test spikes/engine-placement/sdk.test.ts
swift test --package-path spikes/engine-placement -c release
swift build --package-path spikes/engine-placement -c release
bun spikes/engine-placement/audit.ts
bun spikes/engine-placement/counter-probe.ts
bun spikes/engine-placement/replay.ts
bun spikes/engine-placement/run.ts --matrix
bun spikes/engine-placement/transport.ts
bun spikes/engine-placement/summarize.ts
```

The conformance audit intentionally exits **2** while gaps remain. The counter probe
also exits **2** when the upstream numeric replay failure is reproduced. Do not mistake a
successful benchmark or the focused Swift tests for full native-runtime conformance.
`run.ts` without `--matrix` runs a small three-candidate smoke comparison. The full
matrix opens temporary Mac windows and needs an interactive desktop. Keep the
machine awake and avoid other CPU-heavy work. It makes no network deployment.

`bun spikes/engine-placement/sensitivity.ts` temporarily replaces native fork staging
with live mutation, verifies that the atomicity owner test fails, and restores the
source in `finally`. Run it without another build or benchmark in flight, then
rebuild. Its log is retained with the results.

Results and temporary SQLite documents go to `.hitslop/v1-evidence/engine-placement`.
The matrix freezes its executable and assets, records their hashes, rotates candidate
order, and starts a fresh process for every cell. The `baseline` uses production
runtime source with Loro 1.16.1; `matched` uses the same source with Loro 1.16.2;
`native` uses Swift bindings whose Cargo.lock pins Loro core 1.16.2. Package locks are
committed. No engine dependency or release record in the application is changed.

## What is measured

- Engine/view opening in a fresh process, engine-ready time, and host plus identified WebContent
  physical footprint. This is not a cold filesystem-cache measurement and excludes
  GPU/network helper processes. Opening starts after seed creation and SQLite handle
  acquisition; it includes reading/importing state and creating the views, not process
  launch or production catalog startup.
- Identical Svelte views, rendering 40 rows including the final row at all dataset
  sizes. This is an engine/bridge comparison, not a benchmark of rendering 40,000 DOM
  rows. The native page requests no Loro/WASM resources.
- Accepted checkbox, move, text splice and paste latency; two animation frames after
  Svelte's update as a **render-opportunity proxy**, not a compositor paint timestamp.
- Sustained scripted typing at 20 characters/second, pending queue size, drain time,
  exact final text, frame intervals and flush completion. Every run must perform at
  least eight periodic saves during that sequence; both adapters schedule from the
  first unscheduled edit with the same 200 ms interval.
- A separate exact-payload WebSocket echo test at 0/50/150 ms simulated RTT. It
  isolates the native transport hop. It does not model Cloudflare storage, auth,
  CRDT merging or remote-user paint.
- A separate durable-log component replay: native SQLite owners and WASM converge,
  lost acknowledgements deduplicate, a server restart preserves the log, and 1,000
  offline edits replay in bounded pages. Delivery is controlled by the test runner;
  this does not claim a complete production synchronization protocol.

The `native-naive` diagnostic remains available directly through the harness: it
scans row handles and sends whole-list projections after moves. The improved native
variant uses a container-ID index and confirmed move patches. Both still stage on a
fork, so placement results do not include an unproven transaction rewrite.

## SDK experiment

`contract.ts` owns the operation envelope and generates the Swift tagged union;
`JSONValue` represents dynamic application data. `sdk.ts` demonstrates typed field
references with synchronous intent collection and asynchronous acceptance:

```ts
const fields = checklist.fields;
await document.edit(edit => {
  edit.splice(fields.title, 0, 0, 'Today: ');
});
await document.flush();
```

A callback runs once; a throw submits nothing. A retained batch cannot accept later
writes. The host serializes atomic batches and persistence. The small SDK is a
proposal, not a compatibility wrapper or complete replacement for all handles.
The experimental view's draft queue is local-only: remote ancestry and real IME
behavior remain explicit blockers.
