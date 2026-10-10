//! Read-only inspection of a local attachment before a batch imports it.
use hitslop_core::{ATTACHMENT_FILE_BYTES, ATTACHMENT_NAME_BYTES, Code, engine::AttachmentRef, store};
use std::fs::OpenOptions;
use std::io::Read;
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;

pub(super) fn reference(path: &Path) -> store::Result<AttachmentRef> {
    let rejected = |code, message: String| {
        store::Error::Rejected(hitslop_core::Error { code, message, op_index: None, path: vec![] })
    };
    // Never follow a link, and do not wait on a FIFO before checking its file type.
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)
        .map_err(|e| rejected(Code::InvalidRequest, format!("Cannot read attachment: {e}")))?;
    let info = file.metadata().map_err(|e| store::Error::Failed(e.to_string()))?;
    if !info.is_file() {
        return Err(rejected(Code::InvalidRequest, "Attachment must be a regular file".into()));
    }
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| rejected(Code::InvalidRequest, "Attachment needs a UTF-8 file name".into()))?;
    if name.len() > ATTACHMENT_NAME_BYTES {
        return Err(rejected(Code::TooLarge, format!("File name exceeds {ATTACHMENT_NAME_BYTES} bytes")));
    }
    if info.len() > ATTACHMENT_FILE_BYTES as u64 {
        return Err(rejected(Code::TooLarge, format!("Attachment exceeds {} MiB", ATTACHMENT_FILE_BYTES >> 20)));
    }
    // The file can grow after stat; limit the read and let the core refuse the extra byte.
    let mut bytes = Vec::new();
    file.take(ATTACHMENT_FILE_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| store::Error::Failed(e.to_string()))?;
    let item = store::Attachment::from_bytes(&bytes)?;
    Ok(AttachmentRef { id: item.id, byte_length: item.bytes, name: name.into(), mime_type: item.media_type })
}
