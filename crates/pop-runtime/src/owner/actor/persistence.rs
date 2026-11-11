//! Serial storage effects, driven by the native persistence thread.
use super::*;

pub(in crate::owner) enum Control {
    Share(mpsc::Sender<Result<Option<store::Share>>>),
    Publish(String, mpsc::Sender<Result<()>>),
    Withdraw(mpsc::Sender<Result<()>>),
}

pub(super) enum Work {
    Control(Control),
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
    /// Publishes a verified copy of the live state at `dest`, then releases the store
    /// without saving.
    Handoff {
        generation: u64,
        dest: PathBuf,
        state: Vec<u8>,
        preview: Option<Vec<u8>>,
        icon: Option<Vec<u8>>,
        value: serde_json::Value,
        theme: std::collections::BTreeMap<String, String>,
        callback: Completion,
    },
}
/// The artwork a window rendered, by name, leaving out what it did not render.
fn named_artwork<'a>(preview: &'a Option<Vec<u8>>, icon: &'a Option<Vec<u8>>) -> Vec<(Artwork, &'a [u8])> {
    [(Artwork::Preview, preview), (Artwork::Icon, icon)]
        .into_iter()
        .filter_map(|(name, png)| Some((name, png.as_deref()?)))
        .collect()
}
pub(super) enum StorageAction {
    Copy { path: PathBuf, state: Vec<u8>, preview: Option<Vec<u8>>, icon: Option<Vec<u8>> },
    CaptureSource(PathBuf),
    Backup(PathBuf),
    StageSeed(PathBuf),
    Artwork(Artwork),
    SetArtwork { preview: Option<Vec<u8>>, icon: Option<Vec<u8>> },
    Attachments,
    ReadAttachment(String),
    PutAttachment(Vec<u8>),
    Share(store::Share),
    Unshare,
}
pub(in crate::owner) struct Persisted {
    pub generation: u64,
    pub state: store::SaveState,
    pub outcome: Outcome,
}
pub(in crate::owner) enum Outcome {
    Saved { target: u64, result: Result<()> },
    Restored { result: Result<Box<Document>>, callback: Completion },
    Stored { result: Result<Reply>, callback: Completion, closing: bool, refresh_status: bool },
}
pub(super) fn persistence(store: store::Store, work: mpsc::Receiver<Work>, sender: mpsc::Sender<Message>) {
    for work in work {
        if let Work::Control(control) = work {
            match control {
                Control::Share(reply) => {
                    let _ = reply.send(contained(|| Ok(store.share()?)));
                }
                Control::Publish(json, reply) => {
                    let _ = reply.send(contained(|| Ok(store.publish_discovery(&json)?)));
                }
                Control::Withdraw(reply) => {
                    store.withdraw_discovery();
                    let _ = reply.send(Ok(()));
                }
            }
            continue;
        }
        let done = perform(&store, work);
        if let Err(mpsc::SendError(Message::Persisted(Persisted {
            outcome: Outcome::Stored { callback, .. } | Outcome::Restored { callback, .. },
            ..
        }))) = sender.send(done)
        {
            complete(callback, Err(closed()));
        }
    }
}
/// One serial storage effect, driven by a native thread.
pub(super) fn perform(store: &store::Store, work: Work) -> Message {
    let generation = match &work {
        Work::Save { generation, .. }
        | Work::Restore { generation, .. }
        | Work::Store { generation, .. }
        | Work::Close { generation, .. }
        | Work::Handoff { generation, .. } => *generation,
        Work::Control(_) => unreachable!("controls are handled before persistence effects"),
    };
    let outcome = match work {
        Work::Control(_) => unreachable!("controls are handled before persistence effects"),
        Work::Save { target, job, .. } => {
            Outcome::Saved { target, result: contained(|| store.write(&job).map_err(Failure::from)) }
        }
        Work::Restore { callback, .. } => Outcome::Restored {
            callback,
            result: contained(|| store.open_document().map(Box::new).map_err(Failure::from)),
        },
        Work::Store { action, callback, .. } => {
            let refresh_status = matches!(action, StorageAction::Share(_) | StorageAction::Unshare);
            let result = contained(|| {
                Ok(match action {
                    StorageAction::Copy { path, state, preview, icon } => {
                        store.copy_clean(&path, &state, &named_artwork(&preview, &icon))?;
                        Reply::Unit
                    }
                    StorageAction::CaptureSource(path) => {
                        store.capture_source(&path)?;
                        Reply::Unit
                    }
                    StorageAction::Backup(path) => {
                        store.backup(&path)?;
                        Reply::Unit
                    }
                    StorageAction::StageSeed(path) => {
                        store.stage_seed(&path)?;
                        Reply::Unit
                    }
                    StorageAction::SetArtwork { preview, icon } => {
                        store.set_artwork(&named_artwork(&preview, &icon))?;
                        Reply::Unit
                    }
                    StorageAction::Artwork(name) => Reply::Bytes { bytes: store.artwork(name)? },
                    StorageAction::Attachments => Reply::Attachments { items: store.attachments()? },
                    StorageAction::ReadAttachment(id) => Reply::Bytes { bytes: Some(store.attachment(&id)?) },
                    StorageAction::PutAttachment(bytes) => Reply::Attachment { item: store.put_attachment(&bytes)? },
                    StorageAction::Share(share) => {
                        store.set_share(&share)?;
                        Reply::Unit
                    }
                    StorageAction::Unshare => {
                        store.clear_share()?;
                        Reply::Unit
                    }
                })
            });
            Outcome::Stored { result, callback, closing: false, refresh_status }
        }
        Work::Close { job, preview, icon, callback, .. } => {
            let result = contained(|| {
                // Housekeeping cannot fail a close whose final save succeeded.
                if let Some(job) = job {
                    let _ = store.write(&job);
                }
                let artwork = named_artwork(&preview, &icon);
                if !artwork.is_empty() {
                    let _ = store.set_artwork(&artwork);
                }
                // Only a confirmed close ends the undo session. Reclamation then takes
                // a fresh lease, so a failed close keeps its undo attachments intact.
                store.close_reclaiming()?;
                Ok(Reply::Unit)
            });
            Outcome::Stored { result, callback, closing: true, refresh_status: false }
        }
        Work::Handoff { dest, state, preview, icon, value, theme, callback, .. } => {
            let result = contained(|| {
                store.handoff_copy(&dest, &state, &named_artwork(&preview, &icon), &value, &theme)?;
                store.close()?;
                Ok(Reply::Unit)
            });
            Outcome::Stored { result, callback, closing: true, refresh_status: false }
        }
    };
    Message::Persisted(Persisted { generation, state: store.save_state(), outcome })
}
