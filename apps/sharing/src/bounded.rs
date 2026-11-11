//! Cross-object calls have bounded waits; a timeout leaves their outcome unknown.
use futures_util::future::{Either, select};
use std::{future::Future, time::Duration};

pub(crate) async fn bounded<T>(call: impl Future<Output = worker::Result<T>>, limit: Duration) -> worker::Result<T> {
    let call = std::pin::pin!(call);
    let deadline = std::pin::pin!(worker::Delay::from(limit));
    match select(call, deadline).await {
        Either::Left((reply, _)) => reply,
        Either::Right(_) => Err(worker::Error::RustError("Internal request timed out".into())),
    }
}
