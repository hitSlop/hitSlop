# Retain the existing template cache

The full-corpus Nx pilot failed the macOS adoption gate. Production keeps the
existing validated per-template cache and Actions cache transport. No Nx
dependencies or task adapters are adopted.

| Measured path | Existing/direct | Nx |
|---|---:|---:|
| macOS warm corpus, including cache transfer | 6.9s | 55.4s |
| macOS one-slop edit, including cache transfer | 13.5s | 64.2s |
| Linux portable compilation and verification | 94.5s uncached | 3.8s remote |

These are medians from three fresh-runner attempts, not whole-release timings.
The [pilot report](https://github.com/hitSlop/hitslop/blob/0a795b375f4a4cfabc408621f94b4d75d82fddc2/docs/nx-pilot.md)
and [machine evidence](https://github.com/hitSlop/hitslop/blob/0a795b375f4a4cfabc408621f94b4d75d82fddc2/docs/evidence/nx-corpus-2026-09-25.json)
remain on the unmerged experiment branch. The pilot workflow is disabled and its
dedicated GitHub Actions secret has been removed. Historical Cloud data is retained.

Useful fixes are independent of Nx: Svelte style hashes now depend on component
content, the three date-dependent sample seeds use fixed dates, and shared cache
inputs include the copied document skill and inherited root TypeScript config.
The fixed dates describe sample content; they do not disable live date features.

`bundled.json` still selects shipped templates; per-template input hashes select
what rebuilds. Build packages and artwork remain generated. No historical runtime,
preserved package, or release record is regenerated to increase cache hits.

Fresh initialization and preserved saved-state loading remain separate render
cases. Ignoring `state/` does not establish equivalent coverage. The render runner
now records stage timings before considering fewer helper launches. A portable
compile cache and combined PNG/PDF export are follow-ups requiring measurements;
neither is part of this change. Signing, notarization and ZIP/DMG distribution keep
their existing behavior.

## Validation after removing the pilot (2026-09-25, before contract 3)

The [GitHub validation run](https://github.com/hitSlop/hitslop/actions/runs/36175211719)
tested code commit `6dc7a9b`. Its Linux job completed in 2m 30s and its everyday
native job in 6m 55s. It passed 107 Bun tests, 55 compatibility replay cases,
51 bundled template open/reopen checks, 126 Swift tests and seven native CLI
tests, along with the native helper, storage and crash checks. The everyday
render smoke passed all four packages.

These timings describe that run and its fixture-based native scope; they are
not a claimed speedup for the complete release gate. Full-corpus profiling is
an explicit manual option and does not sign or publish anything.

The full sweep passed all **104** cases in **669.6s** (11m 10s). Initial reads
took 134.6s, PNG exports 212.9s, PDF exports 186.9s, and final reads 133.9s.
These stage times include startup and execution; they do not establish how much
batching would save. No cases were skipped.

The first template build had no cache entries and built all 51 in 265.8s.
The second build restored all 51, rebuilt none, and took **1.04s** on the same
runner. These command times exclude Actions cache transfer and helper setup;
they are not comparable directly to the pilot's transfer-inclusive table above.
The complete profiling job, including setup, both builds and the full render,
took 17m 6s. [Per-package timings and build evidence](evidence/render-profile-2026-09-25.json)
are retained with the code; the GitHub artifact also contains every PNG/PDF.
