# JavaScriptCore document-authority experiment

An isolated, deliberately narrow comparison of ordinary JSON operations in JavaScriptCore
with the existing WASM and native-Loro placement controls. It does not change the Mac
product, its SDK, package format, templates, sealed artifacts or release records.

The reducer supports the dedicated checklist fixture: strings, booleans, objects and
ordered object rows. It validates input, uses stable IDs, rejects revision conflicts,
stages an atomic batch, validates the result and checks a 16 MiB UTF-8 bound.
IDs are supplied in fixture/operation data, so the reducer is deterministic. This is
not a proposed final public API or a complete host service.

## Run

From the repository root, with existing workspace and engine-placement dependencies:

```sh
bun test spikes/document-authority/core.test.ts
bun node_modules/typescript/bin/tsc -p spikes/document-authority/tsconfig.json
bun spikes/document-authority/prepare.ts
bun spikes/document-authority/run.ts --headless
bun spikes/document-authority/run.ts --web
# Run these sequentially, on an otherwise quiet interactive Mac:
bun spikes/document-authority/run.ts --matrix --headless
bun spikes/document-authority/run.ts --matrix --web
bun spikes/document-authority/summarize.ts HEADLESS_EVIDENCE_DIR WEB_EVIDENCE_DIR
```

Each runner prints its evidence directory under `.hitslop/v1-evidence/document-authority`.
It builds and copies the executables and assets, signs disposable copies with ad-hoc
hardened runtime and no entitlements, records hashes, and rotates candidate order.
It never signs the application or publishes anything. A 180-second per-process deadline
terminates the benchmark process; this is not a claim of safe in-process JS cancellation.

No flag means headless smoke. `--matrix` means five fresh processes per cell.
Headless tests cover 100/1,000/5,000 rows × 1/5/10/25 documents, plus one 40,000-row
stress document and a two-document contention workload. Window tests cover
100/1,000/5,000 rows × 1/5/10 windows for three JSC configurations and both controls.
The complete window matrix takes substantially longer than the smoke run.

## Configurations and ownership

- `shared-vm`: one VM, a separate context/serial queue per document.
- `separate-vm`: a VM/context/serial queue per document.
- `shared-context`: one context and serial queue with a Map of independent documents.

Only platform reducer code runs in JSC; no authored code, native capabilities or
JavaScript timers are exposed. Document state stays inside its context. Calls cross
the Swift boundary as JSON strings. Mutable JSValue graphs never leave their owner.
The prototype has no CLI ownership service or user-package editing path.

## Measurement boundaries

Headless runs separate process baseline, VM allocation, context+bundle evaluation,
document import, operation application, snapshot serialization and SQLite commits.
`ri_phys_footprint` and `ri_lifetime_max_phys_footprint` match the existing probe's
physical-footprint method. The lifetime peak includes all phases and allocation cycles;
it is not the peak of just one edit. Releasing contexts need not immediately return
all VM/allocator memory to the OS. Open/close cycles do not force garbage collection.

The reducer intentionally uses a full JSON copy per batch, full validation and a full
serialization to enforce the size cap. These costs are included in `apply`, and another
serialization supplies the outgoing snapshot. This measures a simple complete candidate,
not JavaScriptCore's theoretical minimum or an optimized persistent data structure.

Contention uses an ordinary 100-row document and a 40,000-row document applying 1,024
changes. It submits the foreground edit about 1ms after background work begins. The
semaphore observes work entry, not the private VM-lock acquisition point. Actual
arrival delays and background durations are retained, so scheduling uncertainty is visible.

Window runs compile the exact existing `engine-placement/web/App.svelte` and workload.
Only adapter selection is extended in generated assets. The workload's positional move
and splice descriptions are converted to versioned ID operations at submission. The
new reducer itself has no positional mutation API and no automatic text merge.
Forty rows are rendered, including the last row; this does not benchmark 5,000 DOM rows.

JSC publishes full snapshots with structural sharing in the Svelte adapter; native Loro
publishes its existing patches. Results therefore compare viable designs, not placement
alone. Engine acceptance, two-rAF render opportunity and durable completion are distinct.
The window comparison keeps the control's periodic 200ms save schedule, and asserts
at least eight saves during sustained scripted typing. Headless `durableEdit` separately
measures apply + serialization + SQLite commit, with reopen verification.

SQLite uses the placement harness's DELETE journal, synchronous EXTRA and fullfsync ON
policy. These are scratch databases, not a production persistence implementation.
Power loss, request deduplication, receipts and failure/retry UI remain outside this spike.

Signing tests the current empty-entitlement hardened configuration; it neither proves a
particular JIT tier nor substitutes for Developer ID signing, notarization or deployment
target testing. A performance conclusion must not infer JIT availability from signing alone.

## Correctness

The literal fixtures are an independent oracle for late atomic rejection, Unicode,
stale fields, unrelated-field edits, row deletion conflicts, identity after moves,
duplicate insertion and missing anchors. The same fixtures run through Bun and JSC.
The native fixture mode also verifies separation between documents in one context.
Additional Bun cases reject prototype paths and oversized operation batches.
Existing production coverage is neither removed nor treated as proof of this new model.

See `REPORT.md` for the measured outcome and `evidence/table.md` for the generated matrix.
