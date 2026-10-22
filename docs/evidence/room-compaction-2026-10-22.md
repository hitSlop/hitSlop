# Room compaction measurements — 2026-10-22

Measured with the real core document, owner-style commits/checkpoints and session
pushes. Bytes below sum the opaque Loro payload in every pushed record, excluding
frame/spans overhead. Base exports retain operations in **updates** format, not snapshots.

| Workload | Pushes | Record bytes | Base bytes | Records / base |
| --- | ---: | ---: | ---: | ---: |
| Type 20,000 characters, save/push every 5 | 4,000 | 383,751 | 20,721 | 18.52× |
| Toggle checklist rows and reorder each commit | 4,000 | 419,892 | 41,330 | 10.16× |
| Replace a 20 KB text field entirely each commit | 1,000 | 20,198,706 | 20,177,062 | 1.00× |

The checklist uses the two-row infrastructure fixture, alternating row identity and
completion state. Replacement alternates the entire text's character, retaining
roughly 20 MB of inserted history despite a small current value. This deliberately
measures past room capacity; a real room would reject pushes earlier.

The 4 MiB / 8,192-row hint provides useful headroom before the 16 MiB / 32,768-row
retained-update limits for small interactive edits. Neither typing nor checklist
work hits 4 MiB here; both reach the row hint after roughly twice this run. There is
no evidence to change those thresholds from these workloads. Full replacement barely
shrinks: compaction cannot make indefinite history fit a 16 MiB base. Once the base
outgrows that bound, clients must offer a clean, unshared copy and a new room, rather
than retry forever. A lower hint would not solve that history-growth limit.

Reproduce (the test is gated; keep it for threshold decisions):

```sh
HITSLOP_NIGHTLY=1 NEXTEST_SUCCESS_OUTPUT=immediate bun run verify rust sync::room_compaction_measurements
```

The test also verifies that an oversized base returns the typed `TooLarge` core
outcome (mapped to `Outgrown` by the session). The measured run passed in 89.9 seconds
in the unoptimized test profile. Small byte differences across peers/runs are expected.
