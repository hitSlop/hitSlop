//! Fixed event names only: no user, document, room, IP or credential data.
use worker::{AnalyticsEngineDataPointBuilder, Env};

pub(crate) enum Metric {
    RoomCreated,
    RoomOpened,
    RoomEndedStop,
    RoomEndedAlarm,
    RoomEndedIdle,
    AccountDeleted,
    GuestJoin,
    Limited,
}
pub(crate) fn record(env: &Env, event: Metric) {
    let name = match event {
        Metric::RoomCreated => "room_created",
        Metric::RoomOpened => "room_opened",
        Metric::RoomEndedStop => "room_ended_stop",
        Metric::RoomEndedIdle => "room_ended_idle",
        Metric::RoomEndedAlarm => "room_ended_alarm",
        Metric::AccountDeleted => "account_deleted",
        Metric::GuestJoin => "guest_join",
        Metric::Limited => "limited",
    };
    // Metrics must never turn an accepted lifecycle operation into a failure.
    if let Ok(dataset) = env.analytics_engine("METRICS") {
        let _ = AnalyticsEngineDataPointBuilder::new().add_blob(name).add_double(1).write_to(&dataset);
    }
}
