//! Owner event and completion channels used by runtime boundary tests.
use pop_runtime::owner::{CallOptions, Completion, Event, Failure, Owner};
use pop_storage::store::Mode;
use std::path::Path;
use std::sync::{Arc, mpsc};
use std::time::Duration;

/// Opens an owner and records its events through a channel.
pub fn open(path: &Path, mode: Mode) -> (Owner, mpsc::Receiver<Event>) {
    let (tx, rx) = mpsc::channel();
    (
        Owner::open(
            path,
            mode,
            Arc::new(move |event| {
                let _ = tx.send(event);
            }),
        )
        .unwrap(),
        rx,
    )
}

/// Admits a typed operation with an optional originating view.
pub fn submit<T: Send + 'static>(
    owner: &Owner,
    run: impl FnOnce(&Owner, CallOptions, Completion<T>),
    view: Option<&str>,
) -> mpsc::Receiver<Result<T, Failure>> {
    let (tx, rx) = mpsc::channel();
    run(
        owner,
        CallOptions { view: view.map(str::to_owned), deadline: None },
        Box::new(move |result| {
            let _ = tx.send(result);
        }),
    );
    rx
}
/// Waits at most thirty seconds for a typed owner completion.
pub fn call<T: Send + 'static>(
    owner: &Owner,
    run: impl FnOnce(&Owner, CallOptions, Completion<T>),
) -> Result<T, Failure> {
    submit(owner, run, None).recv_timeout(Duration::from_secs(30)).expect("owner did not complete within 30s")
}
/// Flushes and closes the owner without replacing artwork.
pub fn close(owner: &Owner) {
    call(owner, |owner, options, callback| owner.close(None, None, options, callback)).unwrap();
}
