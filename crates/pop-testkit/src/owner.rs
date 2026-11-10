//! Owner event and completion channels used by runtime boundary tests.
use pop_runtime::owner::{Event, Failure, Owner, Reply, Request};
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

/// Admits a request with its optional originating view and returns its completion channel.
pub fn submit(owner: &Owner, request: Request, view: Option<&str>) -> mpsc::Receiver<Result<Reply, Failure>> {
    let (tx, rx) = mpsc::channel();
    owner.submit(
        request,
        view.map(str::to_owned),
        Box::new(move |result| {
            let _ = tx.send(result);
        }),
    );
    rx
}

/// Waits at most thirty seconds for an owner request to complete.
pub fn call(owner: &Owner, request: Request) -> Result<Reply, Failure> {
    let operation = match &request {
        Request::Flush => "flush",
        Request::ExportTheme => "export theme",
        Request::Close { .. } => "close",
        Request::Discard => "discard",
        Request::State => "read state",
        Request::Apply { .. } => "apply",
        _ => "owner request",
    };
    submit(owner, request, None)
        .recv_timeout(Duration::from_secs(30))
        .unwrap_or_else(|error| panic!("owner did not complete {operation} within 30s: {error}"))
}

/// Flushes and closes the owner without replacing artwork.
pub fn close(owner: &Owner) {
    call(owner, Request::Close { preview: None, icon: None }).unwrap();
}
