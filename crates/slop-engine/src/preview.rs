//! A disposable development page runs against the production owner and command path.
mod transport;
use hitslop_core::{
    command::{self, PageDispatch},
    file::ResourceRoute,
    owner::{Event, Owner, Reply, Request},
    page_wire::{HostAction, HostReply},
    preview::PreviewRequest,
    store::Mode,
};
use serde_json::{Value, json};
use std::{
    path::Path,
    process::ExitCode,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};

fn close(owner: &Owner, deadline: Instant) -> Result<(), String> {
    let (sender, receiver) = mpsc::channel();
    owner.submit(
        Request::Close { preview: None, icon: None },
        None,
        Box::new(move |reply| {
            let _ = sender.send(reply);
        }),
    );
    match receiver.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
        Ok(Ok(Reply::Unit)) => Ok(()),
        Ok(Err(e)) => Err(e.message),
        _ => Err("Preview close timed out".into()),
    }
}
pub fn serve(path: &Path) -> ExitCode {
    let Ok((mut transport, send)) = transport::Transport::new() else {
        return ExitCode::FAILURE;
    };
    let mut shutdown = None;
    let events = send.clone();
    let result = (|| -> Result<(), String> {
        let evaluator = super::evaluator()?;
        let owner = Owner::open_with_evaluator(path, Mode::Document, Arc::new(move |event| {
            let message = match event {
                Event::Publication { json: publication } => json!({"type":"push","pushes":[{"type":"publication","publication":serde_json::from_str::<Value>(&publication).expect("publication")}]}),
                Event::SaveStatus { status, failure } => json!({"type":"save","status":format!("{status:?}"),"error":failure.map(|f| f.message)}),
                _ => return,
            };
            events.send(message);
        }), Some(evaluator)).map_err(|e| e.to_string())?;
        let resources = owner.resource_reader().map_err(|e| e.to_string())?;
        let view = "preview";
        owner.attach(view.into());
        send.send(json!({"type":"ready","pid":std::process::id()}));
        let pending = Arc::new(AtomicUsize::new(0));
        let result = (|| -> Result<(), String> {
            while let Some(line) = transport.next()? {
                let request = match serde_json::from_slice::<PreviewRequest>(&line) {
                    Ok(request) => request,
                    // A page request the owner does not know is refused like the native
                    // host refuses it; a frame without its ID is a broken transport.
                    Err(error) => {
                        let Some(id) = serde_json::from_slice::<Value>(&line).ok().and_then(|v| v["id"].as_u64())
                        else {
                            return Err(error.to_string());
                        };
                        send.send(json!({"type":"reply","id":id,"reply":super::rejected("invalid_request", error)}));
                        continue;
                    }
                };
                match request {
                    PreviewRequest::Resource { id, attachment_id, offset, length } => {
                        let resource = (|| {
                            let info = resources.info(ResourceRoute::Attachment, &attachment_id)?;
                            let bytes = resources.read_range(ResourceRoute::Attachment, &attachment_id, offset, length)?;
                            Ok::<_,hitslop_core::store::Error>(json!({"type":"resource","id":id,"info":info.map(|i| json!({"size":i.size,"mimeType":i.media_type})),"bytes":bytes.map(|b| data_encoding::BASE64.encode(&b))}))
                        })().unwrap_or_else(|e| json!({"type":"resource","id":id,"error":e.to_string()}));
                        send.send(resource);
                    }
                    PreviewRequest::Page { id, request } => {
                        if pending.fetch_add(1, Ordering::SeqCst) >= 64 {
                            pending.fetch_sub(1, Ordering::SeqCst);
                            return Err("Too many preview requests".into());
                        }
                        let output = send.clone();
                        let pending = pending.clone();
                        command::page(
                            &owner,
                            view.into(),
                            &serde_json::to_string(&request).expect("page request"),
                            move |result| {
                                let reply = match result {
                                    PageDispatch::Reply { json, .. } => json,
                                    PageDispatch::Host { action } => match action {
                                        HostAction::WindowResize { width, height } => {
                                            HostReply::WindowResize { width: width.into(), height: height.into() }
                                        }
                                        HostAction::Ready => HostReply::Ready,
                                        HostAction::PageRecovered => HostReply::PageRecovered,
                                        HostAction::Failed { .. } => HostReply::Failed,
                                        HostAction::PageError { .. } => HostReply::PageError,
                                    }
                                    .to_json(),
                                };
                                output.send(json!({"type":"reply","id":id,"reply":reply}));
                                pending.fetch_sub(1, Ordering::SeqCst);
                            },
                        );
                    }
                }
            }
            Ok(())
        })();
        // FIFO fence prevents late evaluations from mutating an abandoned preview.
        owner.attach("preview-closed".into());
        let deadline = Instant::now() + Duration::from_secs(10);
        shutdown = Some(deadline);
        let saved = close(&owner, deadline);
        result.and(saved)
    })();
    let mut failed = result.is_err();
    if let Err(error) = result {
        let error = serde_json::value::RawValue::from_string(super::rejected("invalid_request", error))
            .expect("serialized failure");
        send.send(json!({"type":"fatal","error":error}));
    }
    drop(send);
    failed |= transport.finish(shutdown.unwrap_or_else(|| Instant::now() + Duration::from_secs(10))).is_err();
    if failed { ExitCode::FAILURE } else { ExitCode::SUCCESS }
}
