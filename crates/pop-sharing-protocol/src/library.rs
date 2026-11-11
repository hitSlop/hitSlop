//! Account library metadata. Entries describe discovery and never authorize document access.
use crate::{CONTROL_BYTES, document::DocumentRole, limits};
use pop_contracts::{Code, Error, Result};
use serde::{Deserialize, Serialize};

/// A server-maintained projection of one document into an account's library.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export_to = "sharing.generated.ts"))]
pub struct AccountEntry {
    pub document: String,
    pub role: DocumentRole,
    pub title: String,
    pub template_id: String,
    pub sharing: bool,
    /// Monotonic metadata revision, independent of CRDT progress.
    pub revision: u64,
}

/// Server frames; only a final snapshot page establishes absence from a library.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export_to = "sharing.generated.ts"))]
pub enum LibraryFrame {
    Snapshot { entries: Vec<AccountEntry>, last: bool },
    Upsert { entry: AccountEntry },
    Remove { document: String },
}

impl AccountEntry {
    /// Check the bounds shared by library publishers and readers.
    ///
    /// # Errors
    /// Returns an error for invalid identity, revision or oversized metadata.
    pub fn validate(&self) -> Result<()> {
        if !pop_contracts::identity::valid_document_id(&self.document) || self.revision == 0 {
            return Err(Error::new(Code::InvalidBytes, "Invalid library document or revision"));
        }
        if self.title.len() > limits::TITLE_BYTES || self.template_id.len() > limits::TEMPLATE_ID_BYTES {
            return Err(Error::new(Code::TooLarge, "Library metadata exceeds its limit"));
        }
        Ok(())
    }
}

impl LibraryFrame {
    /// Encode one bounded, validated control frame.
    ///
    /// # Errors
    /// Returns an error for invalid entries or a frame exceeding the control budget.
    pub fn encode(&self) -> Result<String> {
        self.validate()?;
        let json = serde_json::to_string(self).map_err(|e| Error::new(Code::InvalidBytes, e.to_string()))?;
        if json.len() > CONTROL_BYTES {
            return Err(Error::new(Code::TooLarge, "Library frame exceeds its limit"));
        }
        Ok(json)
    }

    /// Decode one control frame, bounding bytes before parsing JSON.
    ///
    /// # Errors
    /// Returns an error for malformed, oversized or invalid frames.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() > CONTROL_BYTES {
            return Err(Error::new(Code::TooLarge, "Library frame exceeds its limit"));
        }
        let frame: Self = serde_json::from_slice(bytes).map_err(|e| Error::new(Code::InvalidBytes, e.to_string()))?;
        frame.validate()?;
        Ok(frame)
    }

    fn validate(&self) -> Result<()> {
        match self {
            Self::Snapshot { entries, .. } => {
                if entries.len() > limits::ACCOUNT_SNAPSHOT_PAGE {
                    return Err(Error::new(Code::TooLarge, "Too many library entries in one page"));
                }
                entries.iter().try_for_each(AccountEntry::validate)
            }
            Self::Upsert { entry } => entry.validate(),
            Self::Remove { document } => {
                if !pop_contracts::identity::valid_document_id(document) {
                    return Err(Error::new(Code::InvalidBytes, "Invalid library document"));
                }
                Ok(())
            }
        }
    }
}

/// Encode a coherent snapshot with both byte and entry-count limits.
///
/// The caller must send these pages before any later changes on the connection.
/// An empty library still produces a final page.
///
/// # Errors
/// Returns an error for invalid metadata or an entry that cannot fit in a frame.
pub fn snapshot_pages(entries: impl IntoIterator<Item = AccountEntry>) -> Result<Vec<String>> {
    let mut pages = Vec::new();
    let mut page = Vec::new();
    // A false final marker is one byte larger than true; reserve its exact envelope.
    let envelope = serde_json::to_string(&LibraryFrame::Snapshot { entries: vec![], last: false })
        .map_err(|e| Error::new(Code::InvalidBytes, e.to_string()))?
        .len();
    let mut size = envelope;
    for entry in entries {
        entry.validate()?;
        let bytes = serde_json::to_vec(&entry).map_err(|e| Error::new(Code::InvalidBytes, e.to_string()))?.len();
        if envelope + bytes > CONTROL_BYTES {
            return Err(Error::new(Code::TooLarge, "Library entry cannot fit in a frame"));
        }
        let comma = usize::from(!page.is_empty());
        if page.len() == limits::ACCOUNT_SNAPSHOT_PAGE || size + comma + bytes > CONTROL_BYTES {
            pages.push(LibraryFrame::Snapshot { entries: std::mem::take(&mut page), last: false }.encode()?);
            size = envelope;
        }
        size += usize::from(!page.is_empty()) + bytes;
        page.push(entry);
    }
    pages.push(LibraryFrame::Snapshot { entries: page, last: true }.encode()?);
    Ok(pages)
}
