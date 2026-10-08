# Loro foundation evidence

The restored frameless window-controller harness was measured on 2026-09-19:

| Rows per window | Windows | Host + WebContent MiB | Durable title edit p95 ms |
| --- | --- | --- | --- |
| 100 | 1 | 168 | 7 |
| 100 | 10 | 900 | 9 |
| 100 | 20 | 1,406 | 13 |
| 1,000 | 1 | 230 | 66 |
| 1,000 | 10 | 1,520 | 70 |
| 1,000 | 20 | 2,933 | 82 |

This uses the real Host window controllers and hover panels in a test harness. It excludes the catalog/Firebase application baseline and GPU/network processes. These are single warm-machine samples with other development activity, not isolated performance guarantees. Raw data: [restored-client-windows.json](restored-client-windows.json). Host footprint after closing was 32–54 MiB.

The restored client passed all native test targets, native socket and closed-file WASM editing, failed-save retry, renderer death recovery, PNG/PDF capture, and native/Bun commit-phase crash probes. The packaged Xcode app was opened with an isolated local template catalog; its embedded Swift helper also read documents with Node/Bun excluded from PATH.

Run `bun run bench:windows` for the current opt-in window matrix. The full correctness gate is `bun run test:local`. These measurements are historical observations, not performance guarantees.

Storage/crash evidence: [storage-v1.json](storage-v1.json) and [native-crash.json](native-crash.json). Superseded minimal-host results are retained under `deferred/docs/benchmarks/minimal-host`.

## PNG optimizer comparison (2026-09-22)

Keep the existing lossless optimizer. A small in-memory `swift-png` 4.5.0 adapter did not meet the replacement criteria: no larger aggregate output and no slower median processing. Both implementations were compiled with SwiftPM's release configuration; times are medians of three runs with other local tests active, not isolated latency guarantees. Output size alone rules out this replacement.

Inputs were the two generated template previews and the checked-in transparent washer, decoded and re-encoded with `NSBitmapImageRep` to obtain native PNG baselines, plus synthetic opaque gradients at 480×620 and 2400×2000. The adapter decoded with `PNG.Image`, retained metadata, packed fully opaque pixels as RGB when significant-bits metadata was absent, and compressed at levels 6 and 9. All tested outputs retained identical decoded RGBA pixels. Metadata and cancellation checks were not pursued because size already failed.

| Input | Native bytes | Current bytes | Library level 6 bytes | Library level 9 bytes |
| --- | ---: | ---: | ---: | ---: |
| Quick Checklist | 78,354 | 48,841 | 62,464 | 74,976 |
| Small Expenses | 63,789 | 42,483 | 55,841 | 59,385 |
| Transparent washer | 5,403 | 1,935 | 2,897 | 4,093 |
| Gradient 480×620 | 7,473 | 1,884 | 2,393 | 5,911 |
| Gradient 2400×2000 | 91,129 | 18,635 | 18,934 | 82,251 |

Aggregate output was 113,778 bytes for the current optimizer, 142,529 at library level 6 (25.3% larger), and 226,616 at level 9 (99.2% larger). A level-13 attempt was stopped after more than 90 seconds without completing the first image comparison. No library dependency or export behavior changed. Raw sizes, pixel comparisons, and timings: [png-optimization.json](png-optimization.json).
