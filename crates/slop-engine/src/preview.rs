//! A disposable development page runs against the production owner and command path.
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
    io::{BufRead, Read, Write},
    path::Path,
    process::ExitCode,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
        mpsc,
    },
    time::Duration,
};

fn failure(error: impl ToString) -> Value {
    json!({"ok":false,"code":"rejected","reason":"invalid_request","error":error.to_string()})
}
fn close(owner: &Owner) -> Result<(), String> {
    let (sender, receiver) = mpsc::channel();
    owner.submit(
        Request::Close { preview: None, icon: None },
        None,
        Box::new(move |reply| {
            let _ = sender.send(reply);
        }),
    );
    match receiver.recv_timeout(Duration::from_secs(10)) {
        Ok(Ok(Reply::Unit)) => Ok(()),
        Ok(Err(e)) => Err(e.message),
        _ => Err("Preview close timed out".into()),
    }
}
pub fn serve(path: &Path) -> ExitCode {
    let (send, receive) = mpsc::sync_channel::<Value>(128);
    let overflow = Arc::new(AtomicBool::new(false));
    let writer_overflow = overflow.clone();
    let writer = std::thread::spawn(move || {
        let mut stdout = std::io::stdout().lock();
        for value in receive {
            if writeln!(stdout, "{value}").and_then(|_| stdout.flush()).is_err() {
                break;
            }
            if writer_overflow.swap(false, Ordering::SeqCst) {
                let _ = writeln!(stdout, "{}", json!({"type":"push","pushes":[{"type":"resync"}]}));
                let _ = stdout.flush();
            }
        }
    });
    let events = send.clone();
    let result = (|| -> Result<(), String> {
        let evaluator = hitslop_runner::Evaluator::new(
            std::env::current_exe().map_err(|e| e.to_string())?,
            vec!["--evaluate-command".into()],
        )?;
        let owner = Owner::open_with_evaluator(path, Mode::Document, Arc::new(move |event| {
            let message = match event {
                Event::Publication { json: publication } => json!({"type":"push","pushes":[{"type":"publication","publication":serde_json::from_str::<Value>(&publication).expect("publication")}]}),
                Event::SaveStatus { status, failure } => json!({"type":"save","status":format!("{status:?}"),"error":failure.map(|f| f.message)}),
                _ => return,
            };
            if events.try_send(message).is_err() { overflow.store(true, Ordering::SeqCst); }
        }), Some(evaluator)).map_err(|e| e.to_string())?;
        let resources = owner.resource_reader().map_err(|e| e.to_string())?;
        let view = "native-preview";
        owner.attach(view.into());
        let _ = send.send(json!({"type":"ready","pid":std::process::id()}));
        let pending = Arc::new(AtomicUsize::new(0));
        let result = (|| -> Result<(), String> {
            let mut stdin = std::io::stdin().lock();
            loop {
                let mut line = Vec::new();
                let count = (&mut stdin)
                    .take(command::MAX_REQUEST_BYTES as u64 + 1)
                    .read_until(b'\n', &mut line)
                    .map_err(|e| e.to_string())?;
                if count == 0 {
                    break;
                }
                if count > command::MAX_REQUEST_BYTES {
                    return Err("Oversized preview frame".into());
                }
                let request = match serde_json::from_slice::<PreviewRequest>(&line) {
                    Ok(request) => request,
                    // A page request the owner does not know is refused like the native
                    // host refuses it; a frame without its ID is a broken transport.
                    Err(error) => {
                        let Some(id) = serde_json::from_slice::<Value>(&line).ok().and_then(|v| v["id"].as_u64())
                        else {
                            return Err(error.to_string());
                        };
                        let _ = send.send(json!({"type":"reply","id":id,"reply":failure(error).to_string()}));
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
                        let _ = send.send(resource);
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
                                let _ = output.send(json!({"type":"reply","id":id,"reply":reply}));
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
        let saved = close(&owner);
        result.and(saved)
    })();
    let failed = result.is_err();
    if let Err(error) = result {
        let _ = send.send(json!({"type":"fatal","error":failure(error)}));
    }
    drop(send);
    let _ = writer.join();
    if failed { ExitCode::FAILURE } else { ExitCode::SUCCESS }
}
