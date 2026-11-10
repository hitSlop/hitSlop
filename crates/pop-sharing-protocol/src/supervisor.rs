//! I/O-free connection scheduling and maintenance shared by native drivers.
pub mod budget;
pub mod maintenance;
use crate::limits;

/// An observed connection outcome. A live session resets the reconnect delay.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event {
    Connect,
    Live,
    Retry,
    Stop,
}
/// The host executes this action using its own clock and cancellation mechanism.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Continue,
    RetryAfter { milliseconds: u64 },
    Stop,
}
/// Retry policy has no runtime, timers or global state.
#[derive(Debug)]
pub struct Supervisor {
    stopped: bool,
    delay_ms: u64,
}
impl Default for Supervisor {
    fn default() -> Self {
        Self::new()
    }
}
impl Supervisor {
    pub fn new() -> Self {
        Self { stopped: false, delay_ms: limits::BACKOFF_MIN_MS }
    }
    pub fn step(&mut self, event: Event) -> Action {
        if self.stopped {
            return Action::Stop;
        }
        match event {
            Event::Connect => Action::Continue,
            Event::Live => {
                self.delay_ms = limits::BACKOFF_MIN_MS;
                Action::Continue
            }
            Event::Retry => {
                let milliseconds = self.delay_ms;
                self.delay_ms = self.delay_ms.saturating_mul(2).min(limits::BACKOFF_MAX_MS);
                Action::RetryAfter { milliseconds }
            }
            Event::Stop => {
                self.stopped = true;
                Action::Stop
            }
        }
    }
}

/// Independent timers are serviced in this order when deadlines coincide.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Wake {
    Ping,
    Probe,
    Compact,
}
/// Driver-local monotonic millisecond deadlines; no polling interval is required.
#[derive(Debug)]
pub struct Schedule {
    ping_at: u64,
    probe_at: u64,
    compact_at: Option<u64>,
}
impl Schedule {
    pub fn new(now_ms: u64) -> Self {
        Self { ping_at: now_ms.saturating_add(limits::PING_MS), probe_at: now_ms, compact_at: None }
    }
    pub fn next_at_ms(&self) -> u64 {
        self.ping_at.min(self.probe_at).min(self.compact_at.unwrap_or(u64::MAX))
    }
    pub fn due(&self, now_ms: u64) -> Option<Wake> {
        if self.ping_at <= now_ms {
            Some(Wake::Ping)
        } else if self.probe_at <= now_ms {
            Some(Wake::Probe)
        } else if self.compact_at.is_some_and(|at| at <= now_ms) {
            Some(Wake::Compact)
        } else {
            None
        }
    }
    pub fn completed(&mut self, wake: Wake, now_ms: u64) {
        match wake {
            Wake::Ping => self.ping_at = now_ms.saturating_add(limits::PING_MS),
            Wake::Probe => self.probe_at = now_ms.saturating_add(limits::ROOM_RECHECK_MS),
            Wake::Compact => self.compact_at = None,
        }
    }
    pub fn compact_after(&mut self, now_ms: u64, delay_ms: u64) {
        self.compact_at = Some(now_ms.saturating_add(delay_ms));
    }
    pub fn probe_now(&mut self, now_ms: u64) {
        self.probe_at = now_ms;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn idle_connections_sleep_until_real_work_and_pressure_wakes_the_probe() {
        let mut schedule = Schedule::new(100);
        assert_eq!(schedule.due(100), Some(Wake::Probe));
        schedule.completed(Wake::Probe, 100);
        assert_eq!(schedule.due(101), None);
        assert_eq!(schedule.next_at_ms(), 100 + limits::PING_MS);
        schedule.probe_now(101);
        assert_eq!(schedule.next_at_ms(), 101);
        schedule.completed(Wake::Probe, 101);
        schedule.compact_after(101, 10);
        assert_eq!(schedule.due(111), Some(Wake::Compact));
        schedule.completed(Wake::Compact, 111);
        assert_eq!(schedule.due(111), None);
    }

    #[test]
    fn reconnect_delay_resets_after_catchup_and_stopped_sessions_never_restart() {
        let mut supervisor = Supervisor::new();
        assert_eq!(supervisor.step(Event::Retry), Action::RetryAfter { milliseconds: limits::BACKOFF_MIN_MS });
        assert_eq!(supervisor.step(Event::Retry), Action::RetryAfter { milliseconds: 2 * limits::BACKOFF_MIN_MS });
        supervisor.step(Event::Live);
        assert_eq!(supervisor.step(Event::Retry), Action::RetryAfter { milliseconds: limits::BACKOFF_MIN_MS });
        supervisor.step(Event::Stop);
        assert_eq!(supervisor.step(Event::Live), Action::Stop);
        assert_eq!(supervisor.step(Event::Retry), Action::Stop);
    }
}
