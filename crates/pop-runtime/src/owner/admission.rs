//! One outstanding-work budget, retained through callback completion. Control work
//! can use the queue's reserved tail so ordinary floods cannot prevent saving or close.
use std::io::{self, Write};
use std::sync::{Arc, Mutex};

/// Bounds outstanding work, including held requests and pending persistence.
const QUEUED_MESSAGES: usize = 136;
/// Leave lifecycle operations room when ordinary producers fill the queue.
const CONTROL_RESERVE: usize = 8;
/// Bound carried payloads and derived buffers until their completions settle.
const PAYLOAD_BYTES: usize = 64 * 1024 * 1024;
/// Preserve save and close capacity beneath the common byte ceiling.
const CONTROL_BYTES: usize = 8 * 1024 * 1024;

#[derive(Default)]
pub(super) struct Admission {
    usage: Mutex<Usage>,
}
#[derive(Default)]
struct Usage {
    messages: usize,
    bytes: usize,
}
pub(crate) struct Permit {
    admission: Arc<Admission>,
    bytes: usize,
}
pub(super) struct ByteLease {
    admission: Arc<Admission>,
    bytes: usize,
}
impl Admission {
    pub(super) fn reserve(self: &Arc<Self>, bytes: usize, control: bool) -> Option<Permit> {
        let mut usage = crate::lock(&self.usage);
        let count = if control { QUEUED_MESSAGES } else { QUEUED_MESSAGES - CONTROL_RESERVE };
        let limit = if control { PAYLOAD_BYTES } else { PAYLOAD_BYTES - CONTROL_BYTES };
        if usage.messages >= count || bytes > limit.saturating_sub(usage.bytes) {
            return None;
        }
        usage.messages += 1;
        usage.bytes += bytes;
        Some(Permit { admission: self.clone(), bytes })
    }
    pub(super) fn reserve_page(self: &Arc<Self>, view_bytes: usize, payload_bytes: usize) -> Option<Permit> {
        self.reserve(view_bytes.saturating_add(payload_bytes), false)
    }
    pub(super) fn reserve_bytes(self: &Arc<Self>, bytes: usize, control: bool) -> Option<ByteLease> {
        let mut usage = crate::lock(&self.usage);
        let limit = if control { PAYLOAD_BYTES } else { PAYLOAD_BYTES - CONTROL_BYTES };
        if bytes > limit.saturating_sub(usage.bytes) {
            return None;
        }
        usage.bytes += bytes;
        Some(ByteLease { admission: self.clone(), bytes })
    }
}
impl Drop for Permit {
    fn drop(&mut self) {
        let mut usage = crate::lock(&self.admission.usage);
        usage.messages -= 1;
        usage.bytes -= self.bytes;
    }
}
impl Drop for ByteLease {
    fn drop(&mut self) {
        crate::lock(&self.admission.usage).bytes -= self.bytes;
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
