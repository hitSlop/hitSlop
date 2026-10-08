# Boundary simplification experiment

This harness measures alternatives without changing production source. It preserves
Loro, full history, the native owner, live CLI editing and the WASM authoring path.
The report is `plans/boundary-simplification-spike.md`. The accepted candidates have
since been adopted in the main tree; production verification is recorded in
`plans/boundary-simplification-adoption.md`. This harness still freezes the tree at
invocation time; use the retained original baseline to reproduce the original comparison.

## Layout and isolation

`harness.ts prepare` freezes tracked and non-ignored working-tree files, including
staged and unstaged edits, and records a SHA-256 inventory and toolchain versions.
It never resets the user's checkout. Candidate source trees live under
`generated/boundary-simplification/`; `candidates.py` applies bounded transformations
to those trees. Every candidate starts from the same frozen source.

Build caches are APFS clones of the local caches, independent of production.
Installed JavaScript dependencies are cloned once and shared read-only by candidates.
Run builds sequentially when they share a compiler cache. Native comparisons use
`activate NAME` to copy only changed inputs into the stable `A` execution path;
the instrumented baseline is preserved as `A-frozen`. This avoids recompiling every
Swift dependency merely because a source directory was relocated. Candidate source
trees are retained independently of that execution path.

Native commands need ordinary Swift compiler-cache access. They may also need a
UI-capable macOS session for WebView, window and export suites.

## Candidates

| Name | Experiment |
|---|---|
| A | Existing persistence with measurement/test hooks only |
| B | Update log and checkpoints; conservative retry without generation/attempt tokens |
| C | Full snapshots in one SQLite document row; separate owner and persistence queues |
| D | C with one serial executor for edits and saves |
| validation | Native descriptor/initial/theme acceptance and cached descriptor identity |
| shape | `svgtypes` parser adapter, pinned to 0.16.1 |
| sqlite-module | Move database-specific duplication beside storage |
| session | Remove the page-session facade; keep one callback channel |

The session prototype deliberately retains `DocumentSession`'s existing callbacks
and connects Host directly. This avoids introducing a delegate protocol solely to
replace callbacks already used by native consumers and tests.

## Reproduction

Start in a fresh output directory; existing workspaces are not overwritten by
`workspace`. Use the repository's Bun, Rust and Xcode toolchains.

```sh
bun archive/spikes/boundary-simplification/harness.ts workspace A
bun archive/spikes/boundary-simplification/install.ts A
python3 archive/spikes/boundary-simplification/candidates.py A
bun archive/spikes/boundary-simplification/harness.ts run A build-native bun run core:build:native
bun archive/spikes/boundary-simplification/harness.ts run A boundary swift test --package-path apps/apple/Packages/HitSlopApple --no-parallel --filter BoundarySpike
bun archive/spikes/boundary-simplification/harness.ts run A package-regressions swift test --package-path apps/apple/Packages/HitSlopApple --no-parallel --filter BoundaryPackageRegression
```

The package regressions intentionally fail on A. Keep that result as evidence.
For B, C and D, create the workspace, install the same test hooks, and apply its
candidate once. Create these while A is still the active baseline (or activate A
again first), so they receive its matching generated bindings. Then activate each
candidate and run the same boundary tests. The hooks include
a native update-import adapter; that adapter is experiment-only, not a production
collaboration API. `install.ts` can refresh test instrumentation without reapplying
a candidate.

```sh
bun archive/spikes/boundary-simplification/harness.ts workspace B
bun archive/spikes/boundary-simplification/install.ts B
python3 archive/spikes/boundary-simplification/candidates.py B
bun archive/spikes/boundary-simplification/harness.ts activate B
bun archive/spikes/boundary-simplification/harness.ts run B boundary swift test --package-path apps/apple/Packages/HitSlopApple --no-parallel --filter BoundarySpike
bun archive/spikes/boundary-simplification/harness.ts run B small-limit env SPIKE_STORAGE_BYTES=65536 swift test --package-path apps/apple/Packages/HitSlopApple --no-parallel --filter retryAtScaledStorageLimit
bun archive/spikes/boundary-simplification/harness.ts run B helper swift build --package-path apps/apple/Packages/HitSlopApple --product hitslop-native
bun archive/spikes/boundary-simplification/harness.ts run B crash bun scripts/crash-matrix.ts
```

The 64 KiB run accelerates repeated-retry boundary testing. It is not evidence that
a workload filled the normal 32 MiB cap. Normal workload runs use the production cap.
The existing tests that deliberately gate a save while awaiting another edit apply
to A/B/C. D intentionally serializes those operations, so those responsiveness tests
are not an acceptance gate for D; its measured stall is the experiment's result.

For `validation`, `shape`, `sqlite-module`, and `session`, apply `candidates.py` to a
fresh workspace without installing persistence hooks. Regenerate native bindings
for validation and copy `PackageRegression.swift` into its `HitSlopCoreTests`
directory; shape also updates the candidate Cargo lockfile. Run relevant
native, Rust and authoring checks as listed in the report. The rejected shape probe
is in `shape-probe.rs`; copy it to the candidate core's `tests/boundary_shape.rs`.

`harness.ts patch NAME` writes the source delta under `patches/`. Persistence patches
strip measurement hooks from both sides before comparison, including diff context.
Generated bindings are regenerated by the usual build, not hand-edited or committed
in a candidate patch.

The retained `combined.patch` contains B, validation, SQLite cleanup and session
facade removal together. To reproduce that result on the frozen source:

```sh
bun archive/spikes/boundary-simplification/harness.ts workspace combined
# Run from generated/boundary-simplification/combined:
patch -p1 -i ../../../archive/spikes/boundary-simplification/patches/combined.patch
# Return to the repository root:
bun archive/spikes/boundary-simplification/harness.ts activate combined
bun archive/spikes/boundary-simplification/harness.ts run combined build bun run build
bun archive/spikes/boundary-simplification/verify.ts combined
bun archive/spikes/boundary-simplification/summarize.ts
```

Activation restores matching generated artifacts and invalidates only this
experiment's cached FFI precompiled header. The subsequent normal build regenerates
bindings from candidate source. `candidates.py C C-repro` also supports generating
a candidate into another frozen copy; B/C/D were regenerated this way and their
tracked source compared byte-for-byte with the tested variants.

## Measurement boundaries

The Swift probe records raw samples for owner edit acceptance, full-snapshot export,
flush, close/reopen, SQLite write duration, payload bytes submitted to committed
transactions, logical database size and the test process's peak resident memory.
SQLite payload bytes are not physical device writes. Peak RSS is the process high
water mark, not an isolated allocation measurement for one document. Publication
delivery and native window behavior are validated by existing native suites; the
owner timings are not an end-to-end typing benchmark.

The two-replica probe shares a durable seed, makes independent text/list/counter edits,
tests missing-dependency rejection and duplicate imports, saves and restarts both
replicas, then compares with an independent core reconstructed from seed and updates.
It adds no network, permissions, accounts, presence or durable inbound update queue.

Raw command logs and measurements stay under `generated/boundary-simplification`.
The final report and compact evidence retained here state which checks completed and
which results are inconclusive; an unrun check is never recorded as a pass.
