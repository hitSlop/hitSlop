//! Push admission mirrors the relay's per-socket window; control traffic stays independent.
use pop_sharing_protocol::limits::{SOCKET_PUSH_BYTES_PER_MINUTE, SOCKET_PUSHES_PER_MINUTE};
use serde::Serialize;

const WINDOW_MS: u64 = 60_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "action", rename_all = "camelCase")]
pub enum BudgetDecision {
    Ready,
    Wait { milliseconds: u64 },
    TooLarge,
}

/// One connection's successful reservations. A delayed frame must be retried after waiting.
#[derive(Debug)]
pub struct PushBudget {
    start_ms: u64,
    bytes: usize,
    pushes: usize,
    awaiting_first_ack: bool,
}
impl PushBudget {
    pub fn new(now_ms: u64) -> Self {
        Self { start_ms: now_ms, bytes: 0, pushes: 0, awaiting_first_ack: false }
    }
    /// Reserves only on Ready. Waiting never consumes the next window's allowance.
    pub fn admit(&mut self, now_ms: u64, bytes: usize) -> BudgetDecision {
        if bytes > SOCKET_PUSH_BYTES_PER_MINUTE {
            return BudgetDecision::TooLarge;
        }
        let elapsed = now_ms.saturating_sub(self.start_ms);
        if elapsed >= WINDOW_MS {
            self.start_ms = now_ms;
            self.bytes = 0;
            self.pushes = 0;
            self.awaiting_first_ack = false;
        }
        if self.bytes.saturating_add(bytes) > SOCKET_PUSH_BYTES_PER_MINUTE || self.pushes >= SOCKET_PUSHES_PER_MINUTE {
            return BudgetDecision::Wait {
                milliseconds: WINDOW_MS.saturating_sub(now_ms.saturating_sub(self.start_ms)),
            };
        }
        if self.pushes == 0 {
            self.awaiting_first_ack = true;
        }
        self.bytes += bytes;
        self.pushes += 1;
        BudgetDecision::Ready
    }
    /// Anchor a fresh window after its first push is acknowledged. The relay received
    /// that push before this clock sample, so a later send cannot reset too early.
    /// Drivers keep one push in flight and acknowledge it before reserving the next.
    pub fn acknowledge(&mut self, now_ms: u64) {
        if self.awaiting_first_ack {
            self.start_ms = now_ms;
            self.awaiting_first_ack = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn paced_backlog_waits_without_consuming_the_next_window() {
        let mut budget = PushBudget::new(100);
        assert_eq!(budget.admit(100, SOCKET_PUSH_BYTES_PER_MINUTE), BudgetDecision::Ready);
        budget.acknowledge(200);
        assert_eq!(budget.admit(200, 1), BudgetDecision::Wait { milliseconds: 60_000 });
        assert_eq!(budget.admit(60_199, 1), BudgetDecision::Wait { milliseconds: 1 });
        assert_eq!(budget.admit(60_200, 1), BudgetDecision::Ready);
        assert_eq!(budget.admit(60_200, SOCKET_PUSH_BYTES_PER_MINUTE + 1), BudgetDecision::TooLarge);
        assert_eq!(budget.admit(60_200, SOCKET_PUSH_BYTES_PER_MINUTE - 1), BudgetDecision::Ready);
    }
    #[test]
    fn tiny_pushes_also_obey_the_frame_budget() {
        let mut budget = PushBudget::new(0);
        for _ in 0..SOCKET_PUSHES_PER_MINUTE {
            assert_eq!(budget.admit(0, 1), BudgetDecision::Ready);
        }
        assert_eq!(budget.admit(0, 1), BudgetDecision::Wait { milliseconds: WINDOW_MS });
        assert_eq!(budget.admit(WINDOW_MS, 1), BudgetDecision::Ready);
    }
}
