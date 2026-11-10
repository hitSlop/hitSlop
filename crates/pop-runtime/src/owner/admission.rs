//! Outstanding public requests retain their budget until completion, including while
//! commands or persistence are pending. Ordinary requests have 128 slots and 56 MiB;
//! Flush, Discard, Close and Handoff have eight separate slots and may use the remaining
//! space up to the shared 64 MiB ceiling. Large control payloads can still be refused.
//! Host dispatches share ordinary admission. Two independent-copy producers may be
//! outstanding; generated snapshot bytes also retain a lease through their completion.
use super::Request;
use std::io;
use std::io::Write;
use std::sync::{Arc, Mutex};

const ORDINARY_REQUESTS: usize = 128;
const CONTROL_REQUESTS: usize = 8;
const ORDINARY_BYTES: usize = 56 * 1024 * 1024;
const TOTAL_BYTES: usize = 64 * 1024 * 1024;
const COPY_PRODUCERS: usize = 2;

#[derive(Default)]
pub(super) struct Admission {
    usage: Mutex<Usage>,
}
#[derive(Default)]
struct Usage {
    ordinary: usize,
    control: usize,
    ordinary_bytes: usize,
    bytes: usize,
    copies: usize,
}
pub(crate) struct Permit {
    admission: Arc<Admission>,
    control: bool,
    bytes: usize,
    copy: bool,
}
pub(super) struct ByteLease {
    admission: Arc<Admission>,
    control: bool,
    bytes: usize,
}
impl Admission {
    pub(super) fn reserve(self: &Arc<Self>, request: &Request, view: &Option<String>) -> Option<Permit> {
        let control =
            matches!(request, Request::Flush | Request::Discard | Request::Close { .. } | Request::Handoff { .. });
        let bytes = retained_bytes(request).saturating_add(view.as_ref().map_or(0, String::capacity));
        let copy = matches!(request, Request::Copy { .. });
        self.reserve_request(bytes, control, copy)
    }
    pub(super) fn reserve_page(self: &Arc<Self>, view_bytes: usize, payload_bytes: usize) -> Option<Permit> {
        let bytes = std::mem::size_of::<Request>().saturating_add(view_bytes).saturating_add(payload_bytes);
        self.reserve_request(bytes, false, false)
    }
    fn reserve_request(self: &Arc<Self>, bytes: usize, control: bool, copy: bool) -> Option<Permit> {
        let mut usage = self.usage.lock().expect("owner request admission");
        if (copy && usage.copies >= COPY_PRODUCERS)
            || bytes > TOTAL_BYTES.saturating_sub(usage.bytes)
            || if control {
                usage.control >= CONTROL_REQUESTS
            } else {
                usage.ordinary >= ORDINARY_REQUESTS || bytes > ORDINARY_BYTES.saturating_sub(usage.ordinary_bytes)
            }
        {
            return None;
        }
        if control {
            usage.control += 1;
        } else {
            usage.ordinary += 1;
            usage.ordinary_bytes += bytes;
        }
        usage.bytes += bytes;
        usage.copies += usize::from(copy);
        Some(Permit { admission: Arc::clone(self), control, bytes, copy })
    }
    pub(super) fn reserve_bytes(self: &Arc<Self>, bytes: usize, control: bool) -> Option<ByteLease> {
        let mut usage = self.usage.lock().expect("owner request admission");
        if bytes > TOTAL_BYTES.saturating_sub(usage.bytes)
            || (!control && bytes > ORDINARY_BYTES.saturating_sub(usage.ordinary_bytes))
        {
            return None;
        }
        usage.bytes += bytes;
        if !control {
            usage.ordinary_bytes += bytes;
        }
        Some(ByteLease { admission: Arc::clone(self), control, bytes })
    }
}
impl Drop for Permit {
    fn drop(&mut self) {
        let mut usage = self.admission.usage.lock().expect("owner request admission");
        if self.control {
            usage.control -= 1;
        } else {
            usage.ordinary -= 1;
            usage.ordinary_bytes -= self.bytes;
        }
        usage.bytes -= self.bytes;
        usage.copies -= usize::from(self.copy);
    }
}
impl Drop for ByteLease {
    fn drop(&mut self) {
        let mut usage = self.admission.usage.lock().expect("owner request admission");
        usage.bytes -= self.bytes;
        if !self.control {
            usage.ordinary_bytes -= self.bytes;
        }
    }
}

fn artwork_bytes(preview: &Option<Vec<u8>>, icon: &Option<Vec<u8>>) -> usize {
    preview.as_ref().map_or(0, Vec::capacity).saturating_add(icon.as_ref().map_or(0, Vec::capacity))
}
fn retained_bytes(request: &Request) -> usize {
    let payload = match request {
        Request::State
        | Request::Undo { .. }
        | Request::Flush
        | Request::Discard
        | Request::Artwork { .. }
        | Request::Attachments
        | Request::Theme
        | Request::ExportTheme => 0,
        Request::Unshare => 0,
        Request::PutAttachment { bytes } => bytes.capacity(),
        Request::Apply { batch, .. } => {
            let mut writer = SizeWriter(0);
            if serde_json::to_writer(&mut writer, batch).is_err() {
                return usize::MAX;
            }
            writer.0.max(batch_capacity(batch))
        }
        Request::Command { name, args_json, .. } => name.capacity().saturating_add(args_json.capacity()),
        Request::Close { preview, icon } => artwork_bytes(preview, icon),
        Request::Handoff { dest, preview, icon } => dest.capacity().saturating_add(artwork_bytes(preview, icon)),
        Request::Copy { destination, preview, icon } => {
            destination.capacity().saturating_add(artwork_bytes(preview, icon))
        }
        Request::CaptureSource { destination }
        | Request::Backup { destination }
        | Request::StageSeed { destination } => destination.capacity(),
        Request::SetArtwork { source_version, preview, icon } => {
            source_version.capacity().saturating_add(artwork_bytes(preview, icon))
        }
        Request::ReadAttachment { id } => id.capacity(),
        Request::Share { share } => share.endpoint.capacity(),
    };
    std::mem::size_of::<Request>().saturating_add(payload)
}
// Capacity accounting estimates memory retained by the request, not total process
// memory. Keep the serialized-size charge too: JSON escaping can cost more than storage.
fn batch_capacity(batch: &pop_contracts::wire::Batch) -> usize {
    let mut bytes = vector_capacity(&batch.intents).saturating_add(optional_string(&batch.if_version));
    for intent in &batch.intents {
        bytes = bytes.saturating_add(intent_capacity(intent));
    }
    bytes
}
fn vector_capacity<T>(items: &Vec<T>) -> usize {
    items.capacity().saturating_mul(std::mem::size_of::<T>())
}
fn optional_string(value: &Option<String>) -> usize {
    value.as_ref().map_or(0, String::capacity)
}
fn path_capacity(path: &Vec<pop_contracts::wire::Segment>) -> usize {
    path.iter().fold(vector_capacity(path), |bytes, segment| {
        bytes.saturating_add(match segment {
            pop_contracts::wire::Segment::Key(key) => key.capacity(),
            pop_contracts::wire::Segment::Id { id } => id.capacity(),
            pop_contracts::wire::Segment::Index { .. } => 0,
        })
    })
}
fn anchor_capacity(anchor: &Option<pop_contracts::wire::Anchor>) -> usize {
    match anchor {
        Some(pop_contracts::wire::Anchor::Before { before }) => before.capacity(),
        Some(pop_contracts::wire::Anchor::After { after }) => after.capacity(),
        None => 0,
    }
}
// BTreeMap exposes no allocation capacity. Charge twice each entry's inline size
// plus six pointers, approximating spare node slots and links; owned key/value
// allocations are counted separately. This is an admission estimate, not an allocator census.
fn map_capacity<K, V>(len: usize) -> usize {
    len.saturating_mul(
        std::mem::size_of::<(K, V)>().saturating_mul(2).saturating_add(std::mem::size_of::<usize>().saturating_mul(6)),
    )
}
pub(super) fn value_capacity(value: &serde_json::Value) -> usize {
    match value {
        serde_json::Value::String(text) => text.capacity(),
        serde_json::Value::Array(values) => {
            values.iter().fold(vector_capacity(values), |bytes, value| bytes.saturating_add(value_capacity(value)))
        }
        serde_json::Value::Object(values) => {
            values.iter().fold(map_capacity::<String, serde_json::Value>(values.len()), |bytes, (key, value)| {
                bytes.saturating_add(key.capacity()).saturating_add(value_capacity(value))
            })
        }
        serde_json::Value::Null | serde_json::Value::Bool(_) | serde_json::Value::Number(_) => 0,
    }
}
fn intent_capacity(intent: &pop_contracts::wire::Intent) -> usize {
    use pop_contracts::wire::Intent;
    match intent {
        Intent::Set { path, value, from, .. } => {
            path_capacity(path).saturating_add(value_capacity(value)).saturating_add(optional_string(from))
        }
        Intent::Insert { path, value, id, at, .. } => path_capacity(path)
            .saturating_add(value_capacity(value))
            .saturating_add(optional_string(id))
            .saturating_add(anchor_capacity(at)),
        Intent::Remove { path, id, .. } => path_capacity(path).saturating_add(optional_string(id)),
        Intent::Move { path, id, at } => {
            path_capacity(path).saturating_add(id.capacity()).saturating_add(anchor_capacity(at))
        }
        Intent::Clear { path } | Intent::Increment { path, .. } => path_capacity(path),
        Intent::Replace { path, value } => path_capacity(path).saturating_add(value_capacity(value)),
        Intent::SetTheme { values, .. } => {
            values.iter().fold(map_capacity::<String, Option<String>>(values.len()), |bytes, (key, value)| {
                bytes.saturating_add(key.capacity()).saturating_add(optional_string(value))
            })
        }
        Intent::ImportTheme { file } => file.capacity(),
    }
}

struct SizeWriter(usize);
pub(crate) fn serialized_size(value: &impl serde::Serialize) -> usize {
    let mut writer = SizeWriter(0);
    if serde_json::to_writer(&mut writer, value).is_err() { usize::MAX } else { writer.0 }
}
impl Write for SizeWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0 = self.0.saturating_add(bytes.len());
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
