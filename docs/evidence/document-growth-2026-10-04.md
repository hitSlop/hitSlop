# Production-owner storage growth

`HITSLOP_GROWTH_DAYS=30 bun run bench:growth` now uses the active conformance fixture,
without archived app dependencies. Each comparison made 3,120 commits and flushed
3,120 times over 30 simulated days: 100 serialized 2,048-point paths per day, removing
each group of 25. Each workload generated 84,663,645 bytes of geometry. Every day's
saved value matched the live value through a separate read-only snapshot.

| Writer lifetime | Highest sampled saved bytes before close | Final saved bytes after close | Mean flush |
| --- | ---: | ---: | ---: |
| Daily close/reopen | 8,199,519 | 1,136 | 3.73 ms |
| One continuously open writer | 16,372,722 | 682,355 | 6.63 ms |

The continuously open writer crossed the normal history threshold between the fourth
and fifth samples. Its saved bytes fell from 16,372,722 to 2,005,487 without a writer
close or forced checkpoint. The existing automatic retention policy remained enabled
and unchanged. No save failed. Both workloads completed in 66 seconds combined.

These are sampled sizes for synthetic heavy use, not a measured upper bound for every
app or an estimate of lifetime capacity. The full measurements and build identity are
in `document-growth-2026-10-04.json`. No JSON copy of document state was persisted; JSON
values were used only transiently to check the core's saved-state round trip.
