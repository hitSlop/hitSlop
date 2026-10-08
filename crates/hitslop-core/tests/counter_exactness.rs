//! Why `s.counter()` stays an exact integer under one authority instead of a Loro counter:
//! the pinned Loro adds f64 deltas when it replays a batch, so once a partial sum passes
//! 2^53 two replicas at the same version vector can disagree. Ignored because it asserts
//! upstream behavior, not hitSlop's: run it with `--ignored` after a Loro upgrade, and
//! when it fails, native counters can be reconsidered (after the corpus passes).
use loro::{ExportMode, LoroDoc};

#[test]
#[ignore = "documents upstream Loro behavior; run after upgrading Loro"]
fn loro_counter_replay_is_inexact_past_two_to_the_53() {
    let source = LoroDoc::new();
    let counter = source.get_counter("count");
    counter.increment(-9_007_199_254_740_990.0).unwrap();
    source.commit();
    let replica = LoroDoc::new();
    replica.import(&source.export(ExportMode::Snapshot).unwrap()).unwrap();
    let base = replica.oplog_vv();
    counter.increment(9_007_199_254_740_990.0).unwrap();
    source.commit();
    counter.increment(9_007_199_254_740_991.0).unwrap();
    source.commit();
    replica.import(&source.export(ExportMode::updates(&base)).unwrap()).unwrap();
    assert_eq!(source.oplog_vv(), replica.oplog_vv());
    assert_ne!(counter.get_value(), replica.get_counter("count").get_value(), "Loro now replays counters exactly");
}
