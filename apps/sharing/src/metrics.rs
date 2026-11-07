//! Fixed event names only: no user, document, room, IP or credential data.
use worker::{AnalyticsEngineDataPointBuilder, Env};

pub(crate) enum Metric {
    DocumentCreated,
    DocumentReady,
    DocumentDeleted,
    GuestJoin,
    Limited,
}
pub(crate) fn record(env: &Env, event: Metric) {
    let name = match event {
        Metric::DocumentCreated => "document_created",
        Metric::DocumentReady => "document_ready",
        Metric::DocumentDeleted => "document_deleted",
        Metric::GuestJoin => "guest_join",
        Metric::Limited => "limited",
    };
    // Metrics must never turn an accepted lifecycle operation into a failure.
    if let Ok(dataset) = env.analytics_engine("METRICS") {
        let _ = AnalyticsEngineDataPointBuilder::new().add_blob(name).add_double(1).write_to(&dataset);
    }
}
