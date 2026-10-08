//! Requests the owner admits later, in order, because work they must not interleave with is
//! still running: a history rebuild, or edits a shared document forwarded and has not yet
//! installed. One queue serves every holder, so `request` has one admission gate.
use super::*;
use std::collections::VecDeque;

/// How many requests may wait; past it a request is refused before admission (`Busy`).
const HELD_REQUESTS: usize = 128;

type Held = (Request, Option<String>, Option<Instant>, Completion);

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

    pub(super) fn precedes(&self, request: &Request) -> bool {
        !self.replaying && !self.held.is_empty() && (edits(request) || fence(request))
    }
}

impl Actor {
    /// Queues `request` behind the work that holds the barrier. Its callback answers when
    /// the request is admitted after `release_held`, or fails with the holder.
    pub(super) fn hold(
        &mut self,
        request: Request,
        view: Option<String>,
        deadline: Option<Instant>,
        callback: &mut Option<Completion>,
    ) -> Result<()> {
        if self.barrier.held.len() >= HELD_REQUESTS {
            return Err(Failure::new(FailureKind::Busy, "Too many requests are waiting; this one was not admitted"));
        }
        if matches!(request, Request::Close { .. }) {
            self.barrier.closing = true;
        }
        self.barrier.held.push_back((request, view, deadline, take(callback)));
        Ok(())
    }
    /// Admits the ready prefix in order. If a request must still wait (for example a
    /// fence after a command starts evaluating), it stays at the head of the queue.
    pub(super) fn release_held(&mut self) {
        if self.barrier.replaying {
            return;
        }
        self.barrier.replaying = true;
        while let Some((request, view, deadline, callback)) = self.barrier.held.pop_front() {
            let remaining = self.barrier.held.len();
            self.dispatch(request, view, deadline, callback);
            if self.barrier.held.len() > remaining {
                let waiting = self.barrier.held.pop_back().expect("request was held again");
                self.barrier.held.push_front(waiting);
                break;
            }
        }
        self.barrier.replaying = false;
        self.barrier.closing = self.barrier.held.iter().any(|(r, ..)| matches!(r, Request::Close { .. }));
    }
    /// Answers every held request with `error`, admitting none.
    pub(super) fn fail_held(&mut self, error: Failure) {
        self.barrier.closing = false;
        for (_, _, _, callback) in std::mem::take(&mut self.barrier.held) {
            complete(callback, Err(error.clone()));
        }
    }
}

/// Saved-state fences cover all earlier queued writes, including command evaluation.
pub(super) fn fence(request: &Request) -> bool {
    matches!(
        request,
        Request::Flush
            | Request::Copy { .. }
            | Request::CaptureSource { .. }
            | Request::Backup { .. }
            | Request::ExportTheme
            | Request::Close { .. }
    )
}

/// Requests that change the document.
pub(super) fn edits(request: &Request) -> bool {
    matches!(
        request,
        Request::Apply { .. } | Request::Command { .. } | Request::Undo { .. } | Request::PutAttachment { .. }
    )
}
