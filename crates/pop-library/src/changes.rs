//! Snapshot invalidation callbacks carry no authority or mutable document state.
use super::*;

pub(super) type Notice = Arc<Mutex<Option<Arc<dyn Fn() + Send + Sync>>>>;

pub(super) fn notify(notice: &Notice) {
    let callback = notice.lock().unwrap().clone();
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
        *self.notice.lock().unwrap() = Some(callback);
    }
    pub(super) fn changing(&self) -> Change<'_> {
        Change(self)
    }
    pub(super) fn notify(&self) {
        notify(&self.notice);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn snapshot_callbacks_can_read_committed_rows_after_projection_locks_are_released() {
        let (directory, template) = crate::test_support::fixture();
        let library = Library::open(&directory.path().join("library"), "account-a").unwrap();
        let weak = Arc::downgrade(&library);
        let snapshots = Arc::new(Mutex::new(Vec::new()));
        let observed = snapshots.clone();
        let locked = Arc::new(AtomicBool::new(false));
        let blocked = locked.clone();
        library.listen(Arc::new(move || {
            let library = weak.upgrade().unwrap();
            if library.inner.try_lock().is_err() || library.projections.try_lock().is_err() {
                blocked.store(true, Ordering::Release);
                return;
            }
            let titles = library.entries().unwrap().into_iter().map(|entry| entry.title).collect::<Vec<_>>();
            observed.lock().unwrap().push(titles);
        }));
        let entry = library.create(&template, "First").unwrap();
        library.rename(&entry.id, "Renamed").unwrap();
        assert!(!locked.load(Ordering::Acquire), "callbacks must run outside index and projection locks");
        assert_eq!(snapshots.lock().unwrap().last().unwrap(), &["Renamed"]);
        let before = snapshots.lock().unwrap().len();
        library.entries().unwrap();
        library.reconcile().unwrap();
        assert_eq!(snapshots.lock().unwrap().len(), before, "unchanged reads must not generate refresh loops");
    }
}
