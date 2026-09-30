# Loro `fork_at` replica divergence — 2026-09-30

Loro 1.16.2, `crates/hitslop-core`, Apple M1. Found by the descriptor-driven model test
(`tests/model.rs`, `scalars_model`, seed 3, step 146), which reported one peer
diverging from a fresh merge of the same operations.

## Failure

When a batch mutated and was then refused, the owner rolled back by replacing its
`LoroDoc` with `doc.fork_at(batch_start)`. The fork held the same operations as every
other replica, but after later imports it resolved concurrent writes to one map key
(last writer wins) differently from a replica that imported the same updates into a
fresh document. The replicas never converged again.

The investigation reproduced this with plain Loro calls: fork at an older frontier,
import concurrent map updates, and compare with a fresh import of the same updates. That
ad hoc reproduction is not kept in the tree. The retained regression is
`crates/hitslop-core/tests/replica.rs`: three peers apply `set`, `clear` and a
mutate-then-refuse batch to one optional integer, and after every sync each peer must
equal a fresh replica opened from the seed plus all updates. It fails with `fork_at`
and passes with the fix.

## Fix

`replica_at` builds the rollback replica by replaying history: it exports
`ExportMode::updates_till(vv)` up to the batch start and imports it into a new
`LoroDoc`. The rebuilt owner then merges like any other replica.

The text slow path (`text.rs`) keeps `fork_at`: its branch lives for one edit, checks
that its text equals `from`, authors only text operations and never imports later
changes. Replay there measured 127–264 ms p95 against about 47 ms for `fork_at`.

## Follow-up

Worth reporting upstream to Loro with a minimal plain-Loro reproduction. Until it is
fixed, never use `fork_at` for a replica that outlives one edit or imports later
updates.
