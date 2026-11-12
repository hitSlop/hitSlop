//! Requests the owner admits later, in order, because work they must not interleave with is
//! still running: a command being evaluated. One queue serves every holder, so `request`
//! has one admission gate.
use super::*;
use std::collections::VecDeque;

type Held = (Operation, Option<String>, Option<Duration>);

#[derive(Default)]
pub(super) struct Barrier {
    held: VecDeque<Held>,
    closing: bool,
    replaying: bool,
}
impl Barrier {
    /// A close is waiting, so new edits are refused rather than queued behind it.
    pub(super) fn closing(&self) -> bool {
        self.closing && !self.replaying
    }

    pub(super) fn precedes(&self, request: &Operation) -> bool {
        !self.replaying && !self.held.is_empty() && (request.properties().edits || request.properties().fence)
    }
}

impl Actor {
    /// Queues `request` behind the work that holds the barrier. Its callback answers when
    /// the request is admitted after `release_held`, or fails with the holder.
    pub(super) fn hold(&mut self, operation: Operation, view: Option<String>, deadline: Option<Duration>) {
        if operation.properties().closes {
            self.barrier.closing = true;
        }
        self.barrier.held.push_back((operation, view, deadline));
    }
    /// Admits the ready prefix in order. If a request must still wait (for example a
    /// fence after a command starts evaluating), it stays at the head of the queue.
    pub(super) fn release_held(&mut self) {
        if self.barrier.replaying {
            return;
        }
        self.barrier.replaying = true;
        while let Some((request, view, deadline)) = self.barrier.held.pop_front() {
            let remaining = self.barrier.held.len();
            self.dispatch(request, view, deadline);
            if self.barrier.held.len() > remaining {
                let waiting = self.barrier.held.pop_back().expect("request was held again");
                self.barrier.held.push_front(waiting);
                break;
            }
        }
        self.barrier.replaying = false;
        self.barrier.closing = self.barrier.held.iter().any(|(r, ..)| r.properties().closes);
    }
    /// Answers every held request with `error`, admitting none.
    pub(super) fn fail_held(&mut self, error: Failure) {
        self.barrier.closing = false;
        for (operation, _, _) in std::mem::take(&mut self.barrier.held) {
            operation.reject(error.clone());
        }
    }
}
