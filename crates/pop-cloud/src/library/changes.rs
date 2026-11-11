//! Snapshot invalidation callbacks carry no authority or mutable document state.
use super::*;

pub(super) type Notice = Arc<Mutex<Option<Arc<dyn Fn() + Send + Sync>>>>;

pub(crate) fn notify(notice: &Notice) {
    let callback = pop_core::lock(notice).clone();
    if let Some(callback) = callback {
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| callback()));
    }
}

pub(super) struct Change<'a>(&'a Library);
impl Drop for Change<'_> {
    fn drop(&mut self) {
        self.0.notify();
    }
}
impl Library {
    /// Install a snapshot invalidation callback. Callbacks may read the library;
    /// they run after the mutation's index/projection locks have been released.
    pub fn listen(&self, callback: Arc<dyn Fn() + Send + Sync>) {
        *pop_core::lock(&self.notice) = Some(callback);
    }
    pub(super) fn changing(&self) -> Change<'_> {
        Change(self)
    }
    pub(crate) fn notify(&self) {
        notify(&self.notice);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn snapshot_callbacks_can_read_committed_rows_after_projection_locks_are_released() {
        let (directory, template) = crate::test_support::template_fixture();
        let library = Library::open(&directory.path().join("library"), "account-a").unwrap();
        let weak = Arc::downgrade(&library);
        let snapshots = Arc::new(Mutex::new(Vec::new()));
        let observed = snapshots.clone();
        let locked = Arc::new(AtomicBool::new(false));
        let blocked = locked.clone();
        library.listen(Arc::new(move || {
            let library = weak.upgrade().unwrap();
            if library.inner.try_lock().is_err() {
                blocked.store(true, Ordering::Release);
                return;
            }
            let titles = library.entries().unwrap().into_iter().map(|entry| entry.title).collect::<Vec<_>>();
            pop_core::lock(&observed).push(titles);
        }));
        library.create(&template, "First").unwrap();
        library.create(&template, "Second").unwrap();
        assert!(!locked.load(Ordering::Acquire), "callbacks must run outside index and projection locks");
        assert_eq!(pop_core::lock(&snapshots).last().unwrap().len(), 2);
        let before = pop_core::lock(&snapshots).len();
        library.entries().unwrap();
        library.reconcile().unwrap();
        assert_eq!(pop_core::lock(&snapshots).len(), before, "unchanged reads must not generate refresh loops");
    }
}
