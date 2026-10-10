//! Serial storage effects, driven by the native thread or browser worker.
use super::*;

pub(super) enum Work {
    Save {
        generation: u64,
        target: u64,
        job: store::SaveJob,
    },
    Restore {
        generation: u64,
        callback: Completion,
    },
    Store {
        generation: u64,
        action: StorageAction,
        callback: Completion,
    },
    Close {
        generation: u64,
        job: Option<store::SaveJob>,
        preview: Option<Vec<u8>>,
        icon: Option<Vec<u8>>,
        callback: Completion,
    },
}
/// The artwork a window rendered, by name, leaving out what it did not render.
#[cfg(not(target_arch = "wasm32"))]
fn named_artwork<'a>(preview: &'a Option<Vec<u8>>, icon: &'a Option<Vec<u8>>) -> Vec<(Artwork, &'a [u8])> {
    [(Artwork::Preview, preview), (Artwork::Icon, icon)]
        .into_iter()
        .filter_map(|(name, png)| Some((name, png.as_deref()?)))
        .collect()
}
#[cfg_attr(
    target_arch = "wasm32",
    expect(dead_code, reason = "native path payloads reach the persistence queue but are refused in the browser")
)]
pub(super) enum StorageAction {
    Copy { path: PathBuf, preview: Option<Vec<u8>>, icon: Option<Vec<u8>> },
    CaptureSource(PathBuf),
    Backup(PathBuf),
    Artwork(Artwork),
    Attachments,
    ReadAttachment(String),
    PutAttachment(Vec<u8>),
    Share(store::Share),
}
#[cfg(not(target_arch = "wasm32"))]
pub(super) fn persistence(store: Arc<store::Store>, work: mpsc::Receiver<Work>, sender: mpsc::Sender<Message>) {
    for work in work {
        let done = perform(&store, work);
        if let Err(mpsc::SendError(Message::Stored { callback, .. } | Message::Restored { callback, .. })) =
            sender.send(done)
        {
            complete(callback, Err(closed()))
        }
    }
}
/// One serial storage effect, driven by a native thread or a browser worker.
pub(super) fn perform(store: &store::Store, work: Work) -> Message {
    match work {
        Work::Save { generation, target, job } => {
            Message::Saved { generation, target, result: contained(|| store.write(&job).map_err(Failure::from)) }
        }
        Work::Restore { generation, callback } => Message::Restored {
            generation,
            callback,
            result: contained(|| store.document().map(Box::new).map_err(Failure::from)),
        },
        Work::Store { generation, action, callback } => {
            let result = contained(|| {
                Ok(match action {
                    #[cfg(not(target_arch = "wasm32"))]
                    StorageAction::Copy { path, preview, icon } => {
                        store.copy_clean(&path, &named_artwork(&preview, &icon))?;
                        Reply::Unit
                    }
                    #[cfg(not(target_arch = "wasm32"))]
                    StorageAction::CaptureSource(path) => {
                        store.capture_source(&path)?;
                        Reply::Unit
                    }
                    #[cfg(not(target_arch = "wasm32"))]
                    StorageAction::Backup(path) => {
                        store.backup(&path)?;
                        Reply::Unit
                    }
                    #[cfg(target_arch = "wasm32")]
                    StorageAction::Copy { .. } | StorageAction::CaptureSource(_) | StorageAction::Backup(_) => {
                        return Err(Failure::rejected(
                            Code::InvalidRequest,
                            "Native path operation is unavailable in the browser",
                        ));
                    }
                    StorageAction::Artwork(name) => Reply::Bytes { bytes: store.artwork(name)? },
                    StorageAction::Attachments => Reply::Attachments { items: store.attachments()? },
                    StorageAction::ReadAttachment(id) => Reply::Bytes { bytes: Some(store.attachment(&id)?) },
                    StorageAction::PutAttachment(bytes) => Reply::Attachment { item: store.put_attachment(&bytes)? },
                    StorageAction::Share(share) => {
                        store.set_share(&share)?;
                        Reply::Unit
                    }
                })
            });
            Message::Stored { generation, result, callback, closing: false }
        }
        Work::Close { generation, job, preview, icon, callback } => {
            let result = contained(|| {
                // Housekeeping cannot fail a close whose final save succeeded.
                if let Some(job) = job {
                    let _ = store.write(&job);
                }
                #[cfg(not(target_arch = "wasm32"))]
                {
                    let artwork = named_artwork(&preview, &icon);
                    if !artwork.is_empty() {
                        let _ = store.set_artwork(&artwork);
                    }
                }
                #[cfg(target_arch = "wasm32")]
                let _ = (preview, icon);
                // After the final save, when no import can be waiting for its
                // reference: the page's barrier drained its imports, and an agent's
                // blobs arrive in the batch that references them.
                let _ = store.reclaim_attachments();
                store.close()?;
                Ok(Reply::Unit)
            });
            Message::Stored { generation, result, callback, closing: true }
        }
    }
}
