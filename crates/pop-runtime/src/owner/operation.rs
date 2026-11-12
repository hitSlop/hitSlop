//! Typed operation envelopes and their admission/barrier policy.
use super::*;
pub(super) struct Properties {
    pub control: bool,
    pub edits: bool,
    pub fence: bool,
    pub closes: bool,
    pub recover: bool,
}
pub(super) enum Operation {
    State { callback: Completion<Reading> },
    Version { callback: Completion<Version> },
    Apply { batch: pop_contracts::wire::Batch, origin: Origin, callback: Completion<Applied> },
    Undo { callback: Completion<Applied> },
    Redo { callback: Completion<Applied> },
    Command { name: String, args_json: String, origin: Origin, callback: Completion<CommandResult> },
    Flush { callback: Completion<()> },
    Discard { callback: Completion<()> },
    Close { preview: Option<Vec<u8>>, icon: Option<Vec<u8>>, callback: Completion<()> },
    Handoff { dest: PathBuf, preview: Option<Vec<u8>>, icon: Option<Vec<u8>>, callback: Completion<()> },
    Copy { destination: PathBuf, preview: Option<Vec<u8>>, icon: Option<Vec<u8>>, callback: Completion<()> },
    CaptureSource { destination: PathBuf, callback: Completion<()> },
    Backup { destination: PathBuf, callback: Completion<()> },
    StageSeed { destination: PathBuf, callback: Completion<()> },
    Artwork { name: Artwork, callback: Completion<Option<Vec<u8>>> },
    SetArtwork { source_version: String, preview: Option<Vec<u8>>, icon: Option<Vec<u8>>, callback: Completion<()> },
    Attachments { callback: Completion<Vec<store::Attachment>> },
    ReadAttachment { id: String, callback: Completion<Vec<u8>> },
    PutAttachment { bytes: Vec<u8>, callback: Completion<store::Attachment> },
    Share { share: store::Share, callback: Completion<()> },
    Unshare { callback: Completion<()> },
    Theme { callback: Completion<Theme> },
    ExportTheme { callback: Completion<String> },
}
impl Operation {
    pub(super) fn properties(&self) -> Properties {
        match self {
            Self::State { .. } => {
                Properties { control: false, edits: false, fence: false, closes: false, recover: false }
            }
            Self::Version { .. } => {
                Properties { control: false, edits: false, fence: false, closes: false, recover: false }
            }
            Self::Apply { .. } => {
                Properties { control: false, edits: true, fence: false, closes: false, recover: false }
            }
            Self::Undo { .. } => {
                Properties { control: false, edits: true, fence: false, closes: false, recover: false }
            }
            Self::Redo { .. } => {
                Properties { control: false, edits: true, fence: false, closes: false, recover: false }
            }
            Self::Command { .. } => {
                Properties { control: false, edits: true, fence: false, closes: false, recover: false }
            }
            Self::Flush { .. } => {
                Properties { control: true, edits: false, fence: true, closes: false, recover: false }
            }
            Self::Discard { .. } => {
                Properties { control: true, edits: false, fence: false, closes: false, recover: true }
            }
            Self::Close { .. } => Properties { control: true, edits: false, fence: true, closes: true, recover: false },
            Self::Handoff { .. } => {
                Properties { control: true, edits: false, fence: true, closes: true, recover: false }
            }
            Self::Copy { .. } => {
                Properties { control: false, edits: false, fence: true, closes: false, recover: false }
            }
            Self::CaptureSource { .. } => {
                Properties { control: false, edits: false, fence: true, closes: false, recover: false }
            }
            Self::Backup { .. } => {
                Properties { control: false, edits: false, fence: true, closes: false, recover: false }
            }
            Self::StageSeed { .. } => {
                Properties { control: false, edits: false, fence: false, closes: false, recover: false }
            }
            Self::Artwork { .. } => {
                Properties { control: false, edits: false, fence: false, closes: false, recover: false }
            }
            Self::SetArtwork { .. } => {
                Properties { control: false, edits: true, fence: true, closes: false, recover: false }
            }
            Self::Attachments { .. } => {
                Properties { control: false, edits: false, fence: false, closes: false, recover: false }
            }
            Self::ReadAttachment { .. } => {
                Properties { control: false, edits: false, fence: false, closes: false, recover: false }
            }
            Self::PutAttachment { .. } => {
                Properties { control: false, edits: true, fence: false, closes: false, recover: false }
            }
            Self::Share { .. } => {
                Properties { control: false, edits: false, fence: false, closes: false, recover: false }
            }
            Self::Unshare { .. } => {
                Properties { control: false, edits: false, fence: false, closes: false, recover: false }
            }
            Self::Theme { .. } => {
                Properties { control: false, edits: false, fence: false, closes: false, recover: false }
            }
            Self::ExportTheme { .. } => {
                Properties { control: false, edits: false, fence: true, closes: false, recover: false }
            }
        }
    }
    pub(super) fn payload_bytes(&self) -> usize {
        match self {
            Self::State { .. } => 0,
            Self::Version { .. } => 0,
            Self::Apply { batch, .. } => serialized_size(batch),
            Self::Undo { .. } => 0,
            Self::Redo { .. } => 0,
            Self::Command { name, args_json, .. } => name.len().saturating_add(args_json.len()),
            Self::Flush { .. } => 0,
            Self::Discard { .. } => 0,
            Self::Close { preview, icon, .. } => artwork_bytes(preview, icon),
            Self::Handoff { dest, preview, icon, .. } => {
                dest.as_os_str().len().saturating_add(artwork_bytes(preview, icon))
            }
            Self::Copy { destination, preview, icon, .. } => {
                destination.as_os_str().len().saturating_add(artwork_bytes(preview, icon))
            }
            Self::CaptureSource { destination, .. } => destination.as_os_str().len(),
            Self::Backup { destination, .. } => destination.as_os_str().len(),
            Self::StageSeed { destination, .. } => destination.as_os_str().len(),
            Self::Artwork { .. } => 0,
            Self::SetArtwork { source_version, preview, icon, .. } => {
                source_version.len().saturating_add(artwork_bytes(preview, icon))
            }
            Self::Attachments { .. } => 0,
            Self::ReadAttachment { id, .. } => id.len(),
            Self::PutAttachment { bytes, .. } => bytes.len(),
            Self::Share { share, .. } => share.endpoint.len(),
            Self::Unshare { .. } => 0,
            Self::Theme { .. } => 0,
            Self::ExportTheme { .. } => 0,
        }
    }
    pub(super) fn reject(self, failure: Failure) {
        match self {
            Self::State { callback, .. } => complete(callback, Err(failure)),
            Self::Version { callback, .. } => complete(callback, Err(failure)),
            Self::Apply { callback, .. } => complete(callback, Err(failure)),
            Self::Undo { callback, .. } => complete(callback, Err(failure)),
            Self::Redo { callback, .. } => complete(callback, Err(failure)),
            Self::Command { callback, .. } => complete(callback, Err(failure)),
            Self::Flush { callback, .. } => complete(callback, Err(failure)),
            Self::Discard { callback, .. } => complete(callback, Err(failure)),
            Self::Close { callback, .. } => complete(callback, Err(failure)),
            Self::Handoff { callback, .. } => complete(callback, Err(failure)),
            Self::Copy { callback, .. } => complete(callback, Err(failure)),
            Self::CaptureSource { callback, .. } => complete(callback, Err(failure)),
            Self::Backup { callback, .. } => complete(callback, Err(failure)),
            Self::StageSeed { callback, .. } => complete(callback, Err(failure)),
            Self::Artwork { callback, .. } => complete(callback, Err(failure)),
            Self::SetArtwork { callback, .. } => complete(callback, Err(failure)),
            Self::Attachments { callback, .. } => complete(callback, Err(failure)),
            Self::ReadAttachment { callback, .. } => complete(callback, Err(failure)),
            Self::PutAttachment { callback, .. } => complete(callback, Err(failure)),
            Self::Share { callback, .. } => complete(callback, Err(failure)),
            Self::Unshare { callback, .. } => complete(callback, Err(failure)),
            Self::Theme { callback, .. } => complete(callback, Err(failure)),
            Self::ExportTheme { callback, .. } => complete(callback, Err(failure)),
        }
    }
    pub(super) fn admitted(self, permit: Permit) -> Self {
        match self {
            Self::State { callback } => Self::State { callback: admitted_callback(callback, permit) },
            Self::Version { callback } => Self::Version { callback: admitted_callback(callback, permit) },
            Self::Apply { batch, origin, callback } => {
                Self::Apply { batch, origin, callback: admitted_callback(callback, permit) }
            }
            Self::Undo { callback } => Self::Undo { callback: admitted_callback(callback, permit) },
            Self::Redo { callback } => Self::Redo { callback: admitted_callback(callback, permit) },
            Self::Command { name, args_json, origin, callback } => {
                Self::Command { name, args_json, origin, callback: admitted_callback(callback, permit) }
            }
            Self::Flush { callback } => Self::Flush { callback: admitted_callback(callback, permit) },
            Self::Discard { callback } => Self::Discard { callback: admitted_callback(callback, permit) },
            Self::Close { preview, icon, callback } => {
                Self::Close { preview, icon, callback: admitted_callback(callback, permit) }
            }
            Self::Handoff { dest, preview, icon, callback } => {
                Self::Handoff { dest, preview, icon, callback: admitted_callback(callback, permit) }
            }
            Self::Copy { destination, preview, icon, callback } => {
                Self::Copy { destination, preview, icon, callback: admitted_callback(callback, permit) }
            }
            Self::CaptureSource { destination, callback } => {
                Self::CaptureSource { destination, callback: admitted_callback(callback, permit) }
            }
            Self::Backup { destination, callback } => {
                Self::Backup { destination, callback: admitted_callback(callback, permit) }
            }
            Self::StageSeed { destination, callback } => {
                Self::StageSeed { destination, callback: admitted_callback(callback, permit) }
            }
            Self::Artwork { name, callback } => Self::Artwork { name, callback: admitted_callback(callback, permit) },
            Self::SetArtwork { source_version, preview, icon, callback } => {
                Self::SetArtwork { source_version, preview, icon, callback: admitted_callback(callback, permit) }
            }
            Self::Attachments { callback } => Self::Attachments { callback: admitted_callback(callback, permit) },
            Self::ReadAttachment { id, callback } => {
                Self::ReadAttachment { id, callback: admitted_callback(callback, permit) }
            }
            Self::PutAttachment { bytes, callback } => {
                Self::PutAttachment { bytes, callback: admitted_callback(callback, permit) }
            }
            Self::Share { share, callback } => Self::Share { share, callback: admitted_callback(callback, permit) },
            Self::Unshare { callback } => Self::Unshare { callback: admitted_callback(callback, permit) },
            Self::Theme { callback } => Self::Theme { callback: admitted_callback(callback, permit) },
            Self::ExportTheme { callback } => Self::ExportTheme { callback: admitted_callback(callback, permit) },
        }
    }
}
fn artwork_bytes(preview: &Option<Vec<u8>>, icon: &Option<Vec<u8>>) -> usize {
    preview.as_ref().map_or(0, Vec::len).saturating_add(icon.as_ref().map_or(0, Vec::len))
}
impl Owner {
    /// Reads the document and the sequence ordering its page publications.
    pub fn state(&self, options: CallOptions, callback: Completion<Reading>) {
        self.enqueue(Operation::State { callback }, options);
    }
    /// Reads the current version token and publication sequence without constructing the document value.
    pub fn version(&self, options: CallOptions, callback: Completion<Version>) {
        self.enqueue(Operation::Version { callback }, options);
    }
    /// Applies one valid batch; completion follows publication of the updated snapshot.
    pub fn apply(
        &self,
        batch: pop_contracts::wire::Batch,
        origin: Origin,
        options: CallOptions,
        callback: Completion<Applied>,
    ) {
        self.enqueue(Operation::Apply { batch, origin, callback }, options);
    }
    /// Undoes the last local edit through the same publication and save path as a batch.
    pub fn undo(&self, options: CallOptions, callback: Completion<Applied>) {
        self.enqueue(Operation::Undo { callback }, options);
    }
    /// Reapplies the next local undo entry through the batch publication and save path.
    pub fn redo(&self, options: CallOptions, callback: Completion<Applied>) {
        self.enqueue(Operation::Redo { callback }, options);
    }
    /// Evaluates an authored command off the owner and admits its intents against the version it read.
    pub fn command(
        &self,
        name: String,
        args_json: String,
        origin: Origin,
        options: CallOptions,
        callback: Completion<CommandResult>,
    ) {
        self.enqueue(Operation::Command { name, args_json, origin, callback }, options);
    }
    /// Waits until storage covers every edit admitted before this operation.
    pub fn flush(&self, options: CallOptions, callback: Completion<()>) {
        self.enqueue(Operation::Flush { callback }, options);
    }
    /// Reloads durable state and fences the old page; ordinary shared documents refuse discard.
    pub fn discard(&self, options: CallOptions, callback: Completion<()>) {
        self.enqueue(Operation::Discard { callback }, options);
    }
    /// Flushes before releasing ownership; a failed save keeps the owner open for retry.
    pub fn close(
        &self,
        preview: Option<Vec<u8>>,
        icon: Option<Vec<u8>>,
        options: CallOptions,
        callback: Completion<()>,
    ) {
        self.enqueue(Operation::Close { preview, icon, callback }, options);
    }
    /// Publishes and verifies the accepted live state at a new path before releasing ownership without saving.
    /// The host must fence page edits first; any failure leaves the owner open with every edit.
    pub fn handoff(
        &self,
        dest: PathBuf,
        preview: Option<Vec<u8>>,
        icon: Option<Vec<u8>>,
        options: CallOptions,
        callback: Completion<()>,
    ) {
        self.enqueue(Operation::Handoff { dest, preview, icon, callback }, options);
    }
    /// Copies the accepted live state as an independent document, including the supplied rendered artwork.
    pub fn copy(
        &self,
        destination: PathBuf,
        preview: Option<Vec<u8>>,
        icon: Option<Vec<u8>>,
        options: CallOptions,
        callback: Completion<()>,
    ) {
        self.enqueue(Operation::Copy { destination, preview, icon, callback }, options);
    }
    /// Flushes and copies the saved document for a capture to render once.
    pub fn capture_source(&self, destination: PathBuf, options: CallOptions, callback: Completion<()>) {
        self.enqueue(Operation::CaptureSource { destination, callback }, options);
    }
    /// Flushes and copies the complete saved file, preserving its identity, history and artwork.
    pub fn backup(&self, destination: PathBuf, options: CallOptions, callback: Completion<()>) {
        self.enqueue(Operation::Backup { destination, callback }, options);
    }
    /// Flushes and stages sharing history with only referenced attachments and no artwork.
    pub fn stage_seed(&self, destination: PathBuf, options: CallOptions, callback: Completion<()>) {
        self.enqueue(Operation::StageSeed { destination, callback }, options);
    }
    /// Reads saved derived artwork, when present.
    pub fn artwork(&self, name: Artwork, options: CallOptions, callback: Completion<Option<Vec<u8>>>) {
        self.enqueue(Operation::Artwork { name, callback }, options);
    }
    /// Stores derived artwork only if its source version still matches the live document.
    pub fn set_artwork(
        &self,
        source_version: String,
        preview: Option<Vec<u8>>,
        icon: Option<Vec<u8>>,
        options: CallOptions,
        callback: Completion<()>,
    ) {
        self.enqueue(Operation::SetArtwork { source_version, preview, icon, callback }, options);
    }
    /// Lists immutable attachment metadata through the persistence worker.
    pub fn attachments(&self, options: CallOptions, callback: Completion<Vec<store::Attachment>>) {
        self.enqueue(Operation::Attachments { callback }, options);
    }
    /// Reads an immutable attachment by identity; missing attachments fail.
    pub fn read_attachment(&self, id: String, options: CallOptions, callback: Completion<Vec<u8>>) {
        self.enqueue(Operation::ReadAttachment { id, callback }, options);
    }
    /// Stores an immutable blob and returns its content identity.
    pub fn put_attachment(&self, bytes: Vec<u8>, options: CallOptions, callback: Completion<store::Attachment>) {
        self.enqueue(Operation::PutAttachment { bytes, callback }, options);
    }
    /// Persists relay membership so the owner retains shared history and attachments.
    pub fn set_share(&self, share: store::Share, options: CallOptions, callback: Completion<()>) {
        self.enqueue(Operation::Share { share, callback }, options);
    }
    /// Clears the saved relay membership.
    pub fn unshare(&self, options: CallOptions, callback: Completion<()>) {
        self.enqueue(Operation::Unshare { callback }, options);
    }
    /// Reads the palette and its publication sequence.
    pub fn theme(&self, options: CallOptions, callback: Completion<Theme>) {
        self.enqueue(Operation::Theme { callback }, options);
    }
    /// Exports a theme file after its palette has been saved.
    pub fn export_theme(&self, options: CallOptions, callback: Completion<String>) {
        self.enqueue(Operation::ExportTheme { callback }, options);
    }
}
