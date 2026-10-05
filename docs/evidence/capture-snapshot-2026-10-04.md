# Saved-snapshot capture gate

Baseline `aa82b548`, macOS native WebKit, 5,000 Quick Checklist rows. One warm-up
and 20 paired measurements, live preview first then saved-snapshot preview. The saved
path includes draining/saving the source page, core SQLite backup, opening a fresh
read-only page, readiness, rendering, teardown and temporary-file cleanup. Both paths
render the complete 5,000-row dedicated Export and use the same capped preview image.
The gate is the nearest-rank p95 of each pair's `snapshotMS - liveMS`, at most 1,000 ms.

The first attempt failed: **3,122 ms** paired added p95. Phase measurements showed
that laying out the invisible App and restoring it at teardown dominated the cost.
Hiding the App root before layout, only when a dedicated Export exists, reduced the
full repeated run to **741 ms** paired added p95 (maximum pair: 1,041 ms). No row reduction, smaller export,
process-pool reuse, retention-policy change or frozen page reuse was used.

Across the optimized run the host footprint ended at 59 MiB versus 88 MiB initially.
The retiring capture WebContent process was 445–500 MiB; at most one remained at each
immediate post-close sample, and none remained after two seconds at the end. These are
observations of this workload, not a general memory ceiling or a leak proof.

Raw results:

- `capture-snapshot-spike-2026-10-04.json`: unoptimized gate failure.
- `capture-snapshot-breakdown-2026-10-04.json`: phase diagnosis, six runs per variant.
- `capture-snapshot-optimized-2026-10-04.json`: 20-pair optimized gate and memory.

The baseline ran in an isolated harness linked against the preserved `aa82b548` build,
so concurrent owner changes could not contaminate it. Its startup stylesheet is what
production now does in Root.svelte: a renderer page hides the App when Export is present.
The production path also drops live-editor focus, selection, scroll and input restoration.

Production integration checks passed after replacement: 12 native tests covering
pending input, saved default view, editor resize independence, theme isolation,
attachment reads after editor close, source cleanup, save failure, capture failure,
missing Export/Icon, PNG/PDF color and text, long-document preview limits, closing
artwork and the live/closed CLI paths. `bun run check` and `bun run hygiene` also passed.
