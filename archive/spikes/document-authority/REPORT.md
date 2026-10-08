# JavaScriptCore authority: performance decision

**Proceed with a checklist-scale host-owned prototype using a separate
JSVirtualMachine, JSContext and serial queue per open document. Do not approve
the production rewrite or the current full-copy reducer for large documents yet.**
These are separate VMs inside one process, not a process per document.

The measured memory cost is reasonable at checklist scale. Sharing a VM saves
memory but lets a large edit in one document block a small edit in another.
The simple reducer becomes expensive at 5,000 rows and unsuitable at 40,000 rows.
This experiment changes no production runtime, SDK, storage format or template.

## Results

Measured on an Apple M1 with 16 GiB RAM, macOS 26.6.2 (25G83), Swift 6.4 and
Bun 1.4.2. The complete matrix passed 436 executions: 195 headless cases,
15 contention cases, 225 window cases and one native fixture execution.
Each performance configuration ran in five fresh processes with candidate order
rotated. Latencies below are **medians of per-process p95s**, not pooled p95s;
memory is the median physical footprint in MiB. Full ranges remain in the evidence.

### How much memory does a JSC per document cost?

These are headless process totals with separate VMs, including the reducer and
loaded documents. The pre-JSC baseline is about 2–3 MiB at these sizes.

| Rows per document | 1 document | 10 documents | 25 documents |
|---|---:|---:|---:|
| 100 | 5.3 MiB | 16.6 MiB | 35.1 MiB |
| 1,000 | 6.6 MiB | 24.2 MiB | 53.0 MiB |
| 5,000 | 12.1 MiB | 55.2 MiB | 129.5 MiB |

Loaded memory is not a steady-state or peak budget. At 5,000 rows, one document
reached a median lifetime peak of 78.0 MiB; 25 documents reached 242.5 MiB
(221.4–342.2 MiB across trials). The full-copy reducer allocates heavily.
Five close/reopen cycles were generally stable at smaller sizes, but the
25-document, 5,000-row curves were variable and sometimes grew on later cycles.
This does not establish leak freedom or a reliable long-session memory ceiling.
Investigate allocation/retention before supporting that workload.

Creating one separate VM, evaluating the bundle and loading 1,000 rows took
7.7 ms median (7.1–8.3 ms); 5,000 rows took 27.1 ms (25.0–29.2 ms).
These are in-process timings, not end-to-end CLI cold starts.

### Why separate VMs?

A foreground 100-row edit arrived about 1 ms after another document started
a 40,000-row, 1,024-operation batch:

| Allocation | Foreground idle p95 | Foreground contended p95 | Contended p95 range |
|---|---:|---:|---:|
| Separate contexts, shared VM | 0.7 ms | 287.3 ms | 248.0–374.0 ms |
| Separate VM/context per document | 1.0 ms | 0.7 ms | 0.6–0.7 ms |
| Shared context | 0.7 ms | 354.3 ms | 251.7–447.3 ms |

Separate VMs prevented the large cross-document stall in this workload.
The small difference between idle and busy measurements is noise, not a speedup.
Use lazy allocation for active documents and release idle owners; do not keep a
VM alive for every document in the catalog. Exact ownership lifetime remains an
integration decision.

### Same Svelte view, different authorities

These totals include the harness host plus its identified WebContent processes.
They are not measurements of the full production app. Only 40 rows are rendered.
The current-WASM control uses Loro 1.16.1; the native-Loro spike uses 1.16.2 and
its partial interpreter. Neither control establishes full SDK equivalence.

| Rows | Windows | Candidate | Memory | Checkbox acceptance | Move acceptance |
|---|---:|---|---:|---:|---:|
| 1,000 | 1 | JSC, separate VMs | 101.3 MiB | 8 ms | 8 ms |
| 1,000 | 1 | Current WASM | 147.1 MiB | 7 ms | 8 ms |
| 1,000 | 1 | Native Loro | 102.2 MiB | 2 ms | 3 ms |
| 1,000 | 10 | JSC, separate VMs | 386.1 MiB | 8 ms | 8 ms |
| 1,000 | 10 | Current WASM | 725.6 MiB | 6 ms | 8 ms |
| 1,000 | 10 | Native Loro | 379.1 MiB | 3 ms | 3 ms |
| 5,000 | 1 | JSC, separate VMs | 125.1 MiB | 30 ms | 30 ms |
| 5,000 | 1 | Current WASM | 211.9 MiB | 15 ms | 35 ms |
| 5,000 | 1 | Native Loro | 120.1 MiB | 5 ms | 6 ms |
| 5,000 | 10 | JSC, separate VMs | 428.9 MiB | 33 ms | 30 ms |
| 5,000 | 10 | Current WASM | 916.4 MiB | 14 ms | 34 ms |
| 5,000 | 10 | Native Loro | 439.8 MiB | 5 ms | 6 ms |

At 100 rows, separate-JSC checkbox acceptance was 2 ms with one window and
96.4 MiB total memory. Across every separate-JSC trial at 1/5/10 windows,
the worst per-run checkbox/move p95s were 3/5 ms at 100 rows, 10/8 ms at
1,000 rows, and 44/33 ms at 5,000 rows. The 5,000-row paste p95 reached 69 ms;
do not interpret the checkbox result as an all-operations 50 ms guarantee.

All sustained scripted typing assertions passed. Separate-JSC trials completed
11–12 periodic saves during typing and drained pending work in at most about
1 ms. This does not exercise real keyboard input, IME or selection behavior.
At 5,000 rows, checkbox render-opportunity p95 medians were 51–54 ms, versus
34 ms for native Loro; individual separate-JSC trials reached 67 ms.

### Decision gates and remaining work

The original proposal in `docs/NextPhasePlan.md` is more ambitious than this
performance slice. Its strict S1 apply-plus-serialization budgets **fail**:
separate-JSC checkbox apply alone was 27.6 ms at 5,000 rows and 232.8 ms at
40,000 rows, above the proposed 5/30 ms limits. At 40,000 rows the median lifetime
peak was 818.3 MiB. The 5,000-row context/load budget passes, but memory is not
uniformly lower than native Loro. S2's one-frame render allowance is not
consistently met at 5,000 rows, although its typing-drain target passes.

Consequently this is a **conditional feasibility result**, not a pass of every
original gate. Full-copy staging, whole-document validation and capacity
serialization need profiling before changing languages or adding patch transport.
Patches alone would not remove those reducer costs. Headless separate-JSC move
p95 also had outliers: 47.3 ms with one 5,000-row document and 70.6 ms with 25.
Shared-VM 5,000-row checkbox p95 reached 90.4 ms even with one document, so that
outlier cannot be blamed on cross-document contention.

The next integration should prove the on-demand owner service and the same
UI/CLI request path, durable receipts/retries, close/export barriers and text drafts
on Quick Checklist. Atomic JSON-file saving, CLI process cold start, authored SDK
cutover and a Durable Object deployment were not implemented or benchmarked here.
The SQLite scratch store is only a controlled durability-cost measurement.
No production contracts or existing tests were removed.

## What the experiment establishes

The candidate is one TypeScript reducer running in a native JSContext. It supports
the dedicated checklist schema, stable-ID operations, field/list revision conflicts,
atomic rejection and explicit JSON snapshots. The exact same fixture script runs in
Bun and JavaScriptCore. The window experiment reuses the placement spike's Svelte
component, workloads, rendered-row count and periodic-save assertions.

Three allocation strategies are compared: one VM with independent contexts, one
VM/context per document, and one context containing multiple documents. Only one
document is actively edited in the ordinary window matrix; a separate contention
workload tests a small edit while another document processes a large batch.

## Interpretation limits

- This reducer copies the whole document, validates the whole result, and serializes
  it for capacity accounting on each batch. Its timings include those decisions.
  They are not a JavaScriptCore microbenchmark or an optimized lower bound.
- Full snapshots cross the native/view boundary. JSC adds structural sharing in the
  view; the native-Loro control uses existing incremental patches. Different semantics
  and adapters mean observed differences cannot be attributed solely to engine placement.
- All documents of a given size start with identical content and IDs. Shared VMs may
  benefit from string interning and GC behavior; memory is not a fixed per-document fee.
- Loaded footprint, post-workload footprint and lifetime peaks are different. Peaks
  include staging, serialization, SQLite and repeated allocation cycles. Context release
  does not guarantee immediate allocator/VM memory return to the OS.
- Window memory includes the host and identified WebContent processes, excluding
  GPU/network helpers. Headless baselines include loaded source/seed buffers but allocate
  no NSApplication or WKWebView. Two animation frames measure render opportunity, not paint.
- Window acceptance precedes coalesced persistence, matching the existing control's
  first-unsaved 200ms schedule. Separate headless durable-edit measurements include SQLite
  commit with the same full-sync policy; successful reopen checks compare saved bytes.
- Ad-hoc hardened runtime with empty entitlements matches the current entitlement
  policy, but does not prove a JIT tier or substitute for Developer ID/notarized app testing.
- A VM per document isolates execution scheduling, not native process crashes. The
  benchmark process watchdog is not an in-process cancellation mechanism.
- The fixture core is not a complete untrusted-input protocol, author SDK, draft/IME
  binding, export path, request-receipt system or persistent document-owner service.
  Performance feasibility does not retire those integration requirements.

## Reproduction and evidence

See [README.md](README.md) for commands and measurement boundaries. The generated
[table](evidence/table.md) and [structured results](evidence/results.json) retain
trial counts, ranges, raw latency samples, memory curves, configurations and frozen
asset/executable hashes. Smoke runs are diagnostics and are excluded from final statistics.

Final matrix evidence was captured in
`.hitslop/v1-evidence/document-authority/2026-09-28T22-53-28.320Z`.
Every JSC window run recorded zero WASM resource requests. Every headless
save/reopen comparison passed. Both literal fixture scenarios passed in JSC,
including the additional shared-context document-isolation check.

The spike's TypeScript check, four Bun tests and native release build passed.
Repository `bun run check` passed, including zero Svelte errors or warnings.
Repository `bun run test` passed: 154 tests, 115 compatibility replay cases
and 52 bundled-template open/reopen checks.
