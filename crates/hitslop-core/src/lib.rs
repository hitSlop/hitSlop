//! Shared native/WASM document semantics.

#[rustfmt::skip]
#[path = "wire.generated.rs"]
mod wire;
use loro::{
    Container, ContainerID, ContainerTrait, ExportMode, Frontiers, Index, LoroDoc, ID, LoroMap, LoroMovableList,
    LoroText, ValueOrContainer, VersionVector,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
mod publication;
mod identity;
mod text;
pub mod theme;
pub mod shape;
mod descriptor;
mod execute;
mod project;
mod issues;
pub use descriptor::{schema_key, validate};
use descriptor::{Node, descriptor, valid_key, loro_scalar, unwrap_optional, utf16_len, is_scalar, holds_collections};
use execute::{fill, put, resolve, execute, Rows};
use identity::stored_id;
use project::{counter_sum, project, project_at};
use issues::{issues, container_issues, scalar_issue};
use publication::{Dirty, Events, ListState};
use std::sync::Arc;
pub use wire::{Code, STORAGE_BYTES, STORAGE_ROWS};
use wire::{valid_id, Anchor, Batch, Hunk, Intent, Segment, Issue, IssueCode, State, Publication, PatchOp};

const MAX_BYTES: usize = wire::STORAGE_BYTES;
const MAX_JSON: usize = 4 * 1024 * 1024;

#[derive(Debug, thiserror::Error)]
#[error("{code}: {message}")]
pub struct Error {
    pub code: Code,
    pub message: String,
    pub op_index: Option<usize>,
}
type Result<T> = std::result::Result<T, Error>;
fn err(code: Code, message: impl ToString) -> Error {
    Error {
        code,
        message: message.to_string(),
        op_index: None,
    }
}
fn engine(e: impl ToString) -> Error {
    err(Code::EngineError, e)
}
fn parse<T: serde::de::DeserializeOwned>(s: &str) -> Result<T> {
    if s.len() > MAX_JSON {
        return Err(err(Code::TooLarge, "JSON exceeds size limit"));
    }
    serde_json::from_str(s).map_err(|e| err(Code::InvalidRequest, e))
}
fn encode(v: &impl Serialize) -> Result<String> {
    serde_json::to_string(v).map_err(engine)
}
/// A materialized Loro value as JSON.
fn json(value: loro::LoroValue) -> Result<Value> {
    serde_json::to_value(value).map_err(engine)
}
fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push(DIGITS[(b >> 4) as usize] as char);
        out.push(DIGITS[(b & 15) as usize] as char);
    }
    out
}
/// Version tokens name at most 1,024 frontier IDs of 12 bytes each.
const MAX_TOKEN_BYTES: usize = 12 * 1024;
fn unhex(s: &str) -> Result<Vec<u8>> {
    if s.len() > 2 * MAX_TOKEN_BYTES || s.len() % 2 != 0 || !s.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(err(Code::InvalidVersion, "Expected an opaque version token"));
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).map_err(engine))
        .collect()
}
/// Version tokens are the document's frontiers: the IDs of its latest operations,
/// sorted, as 12-byte big-endian (peer, counter) records. They grow with concurrent
/// heads, not with every peer ever seen, and are stable across import and reopen until a
/// checkpoint trims the history they name.
fn version_token(frontiers: &Frontiers) -> String {
    let mut ids: Vec<ID> = frontiers.iter().collect();
    ids.sort();
    hex(&ids
        .iter()
        .flat_map(|id| id.peer.to_be_bytes().into_iter().chain(id.counter.to_be_bytes()))
        .collect::<Vec<_>>())
}
/// Decodes a token and proves every ID is in this document's history before any Loro
/// API sees it; unknown operations must never reach a panicking conversion.
fn decode_version(doc: &LoroDoc, s: &str) -> Result<(Frontiers, VersionVector)> {
    let bytes = unhex(s)?;
    if bytes.is_empty() || bytes.len() % 12 != 0 {
        return Err(err(Code::InvalidVersion, "Expected an opaque version token"));
    }
    let (known, trimmed) = (doc.oplog_vv(), doc.shallow_since_vv().to_vv());
    let mut ids = Vec::with_capacity(bytes.len() / 12);
    for record in bytes.chunks_exact(12) {
        let peer = u64::from_be_bytes(record[..8].try_into().expect("8 bytes"));
        let counter = i32::from_be_bytes(record[8..].try_into().expect("4 bytes"));
        if counter < 0 {
            return Err(err(Code::InvalidVersion, "Negative counter"));
        }
        let id = ID::new(peer, counter);
        if !known.includes_id(id) {
            return Err(err(Code::StaleBase, "Version names operations this document does not have"));
        }
        // Loro still resolves some trimmed IDs, but cannot branch or diff from them.
        if trimmed.includes_id(id) {
            return Err(err(Code::StaleBase, "Version precedes this document's retained history"));
        }
        ids.push(id);
    }
    let frontiers = Frontiers::from(ids);
    let vv = doc
        .frontiers_to_vv(&frontiers)
        .ok_or_else(|| err(Code::StaleBase, "Version is not in this document's history"))?;
    Ok((frontiers, vv))
}
const MAX_SAFE: i64 = 9_007_199_254_740_991;
fn safe(n: i64) -> bool {
    (-MAX_SAFE..=MAX_SAFE).contains(&n)
}
fn application_id() -> Result<String> {
    use identity::ALPHABET;
    let mut bytes = [0u8; 16];
    getrandom::getrandom(&mut bytes).map_err(engine)?;
    let mut buffer = 0u32;
    let mut bits = 0;
    let mut out = String::new();
    for byte in bytes {
        buffer = (buffer << 8) | u32::from(byte);
        bits += 8;
        while bits >= 5 {
            bits -= 5;
            out.push(ALPHABET[((buffer >> bits) & 31) as usize] as char);
        }
    }
    out.push(ALPHABET[((buffer << (5 - bits)) & 31) as usize] as char);
    Ok(out)
}

/// This replica's counter key: its peer ID.
/// Issues in one canonical order, by path then code, so every walk agrees.
fn sort_issues(issues: &mut [Issue]) {
    issues.sort_by(|a, b| (&a.path, a.code).cmp(&(&b.path, b.code)));
}
fn writer(doc: &LoroDoc) -> String {
    doc.peer_id().to_string()
}
fn raw(doc: &LoroDoc) -> Result<Value> {
    json(doc.get_map("data").get_deep_value())
}
fn subscribe(doc: &LoroDoc, events: &Events) {
    // The subscription lives exactly as long as this LoroDoc; a replaced doc drops it.
    publication::subscribe(doc, events).detach();
}

/// Identifies the core build (a hash of its sources and the lockfile), so a release can
/// prove the app and its helper embed the same core.
pub const BUILD_ID: &str = env!("HITSLOP_CORE_BUILD_ID");

/// A committed batch: its publication sequence, the IDs of inserted rows, and the
/// publication to deliver, absent when the batch changed nothing.
#[derive(Debug)]
pub struct Applied {
    pub sequence: u64,
    pub ids: Vec<String>,
    pub publication: Option<String>,
}
/// A stateless text edit. `authored` is the version right after this edit on its own
/// branch; the page sends it as the next `base`. Selections are UTF-16 offsets in the
/// merged text. A caret-only request publishes nothing.
#[derive(Debug)]
pub struct TextEdit {
    pub sequence: u64,
    pub authored: String,
    pub selection_start: usize,
    pub selection_end: usize,
    pub publication: Option<String>,
}

/// Exactly one host executor owns this value. Neither binding contains semantics.
pub struct Document {
    doc: LoroDoc,
    schema: Node,
    sequence: u64,
    /// Every movable list's order and row identities as of the last publication.
    lists: HashMap<ContainerID, ListState>,
    /// Issues as of the last publication. Empty is the common case and enables
    /// change-proportional validation; a document with anomalies rescans on publish.
    issues: Vec<Issue>,
    events: Events,
    /// Where the saved history starts once the latest checkpoint is written. A
    /// concurrent edit must not branch from before it: its saved operations would depend
    /// on history the checkpoint drops, and the package could not open again.
    floor: VersionVector,
}
impl Document {
    /// `scan` is false only for a document just built from validated input.
    fn from_doc(doc: LoroDoc, schema: Node, scan: bool) -> Result<Self> {
        let events = Events::default();
        subscribe(&doc, &events);
        let mut this = Self {
            lists: publication::index_all(&doc),
            doc,
            schema,
            sequence: 0,
            issues: vec![],
            events,
            floor: VersionVector::default(),
        };
        if scan {
            this.issues = this.scan_issues()?;
        }
        Ok(this)
    }
    pub fn create(schema: &str, initial: &str) -> Result<Self> {
        let schema = descriptor(schema)?;
        let initial: Value = parse(initial)?;
        schema.validate(&initial, false)?;
        let doc = LoroDoc::new();
        fill(&doc.get_map("data"), &schema, &initial, &writer(&doc))?;
        doc.commit();
        Self::from_doc(doc, schema, false)
    }
    pub fn open(schema: &str, checkpoint: &[u8], updates: &[Vec<u8>]) -> Result<Self> {
        let total = updates
            .iter()
            .try_fold(checkpoint.len(), |n, b| n.checked_add(b.len()))
            .ok_or_else(|| err(Code::TooLarge, "Input bytes"))?;
        if total > MAX_BYTES {
            return Err(err(Code::TooLarge, "Input bytes"));
        }
        Self::open_with(schema, checkpoint, |import| updates.iter().try_for_each(|bytes| import(bytes)))
    }
    /// Imports `checkpoint`, then each update `updates` passes to its import function, in
    /// order. Storage streams rows through it without copying them; callers bound the
    /// total size first.
    pub(crate) fn open_with(
        schema: &str,
        checkpoint: &[u8],
        updates: impl FnOnce(&mut dyn FnMut(&[u8]) -> Result<()>) -> Result<()>,
    ) -> Result<Self> {
        let schema = descriptor(schema)?;
        let doc = LoroDoc::new();
        // A checkpoint may start its history late; the updates after it never move that start.
        checked(doc.import(checkpoint))?;
        let trimmed = doc.shallow_since_vv();
        // One import per update measured faster than Loro's `import_batch` here.
        updates(&mut |bytes: &[u8]| {
            checked(doc.import(bytes))?;
            if doc.shallow_since_vv() != trimmed {
                return Err(err(Code::InvalidBytes, "Only a checkpoint may trim history"));
            }
            Ok(())
        })?;
        Self::from_doc(doc, schema, true)
    }
    /// Same result as `issues` over the full JSON value, without materializing it:
    /// containers are walked directly and only plain or unexpected values become JSON.
    fn scan_issues(&self) -> Result<Vec<Issue>> {
        let mut found = vec![];
        let root = ValueOrContainer::Container(Container::Map(self.doc.get_map("data")));
        container_issues(&self.schema, Some(root), &mut vec![], &mut found)?;
        sort_issues(&mut found);
        Ok(found)
    }
    /// Recomputes the issues under each changed place; every other issue is kept. A place
    /// whose rows have no unique ID widens to its list, as publications do. Returns
    /// whether the issues changed.
    fn refresh_issues(&mut self, dirty: &[Dirty]) -> Result<bool> {
        enum Place { Container(Container, Option<Node>), Entry(LoroMap, String) }
        let mut places: Vec<(Vec<Segment>, Place)> = vec![];
        let mut work: Vec<(ContainerID, Option<String>)> = dirty.iter().map(|d| match d {
            Dirty::Container(cid) => (cid.clone(), None),
            Dirty::Entry(cid, key) => (cid.clone(), Some(key.clone())),
        }).collect();
        while let Some((cid, key)) = work.pop() {
            // A detached container's issues leave with the place that removed it.
            let Some(loro_path) = self.doc.get_path_to_container(&cid) else { continue };
            let Some(container) = self.doc.get_container(cid.clone()) else { continue };
            let mut path = match publication::json_path(&self.lists, &loro_path) {
                Ok(path) => path,
                Err(list) => { work.push((list, None)); continue; }
            };
            let node = publication::node_at(&self.schema, &loro_path);
            match (key, node, container) {
                (Some(key), Some(Node::Object { .. } | Node::Record { .. }), Container::Map(map)) => {
                    path.push(Segment::Key(key.clone()));
                    places.push((path, Place::Entry(map, key)));
                }
                (_, Some(node), container) => places.push((path, Place::Container(container, Some(node.clone())))),
                // Undeclared: its parent reports it.
                (_, None, _) => match loro_path.len() {
                    0 | 1 => places.push((path, Place::Container(Container::Map(self.doc.get_map("data")), Some(self.schema.clone())))),
                    n => work.push((loro_path[n - 2].0.clone(), None)),
                },
            }
        }
        // Outermost places only, so no subtree is recomputed twice.
        places.sort_by_key(|(path, _)| path.len());
        let mut outer: Vec<(Vec<Segment>, Place)> = vec![];
        for (path, place) in places {
            if !outer.iter().any(|(p, _)| path.starts_with(p)) {
                outer.push((path, place));
            }
        }
        let before = std::mem::take(&mut self.issues);
        let mut issues: Vec<Issue> = before.iter().filter(|issue| !outer.iter().any(|(p, _)| issue.path.starts_with(p))).cloned().collect();
        for (path, place) in outer {
            let mut path = path;
            match place {
                Place::Container(container, node) => {
                    let node = node.expect("declared");
                    container_issues(&node, Some(ValueOrContainer::Container(container)), &mut path, &mut issues)?;
                }
                Place::Entry(map, key) => {
                    let loro_path = self.doc.get_path_to_container(&map.id()).unwrap_or_default();
                    let value = map.get(&key);
                    match publication::node_at(&self.schema, &loro_path) {
                        Some(Node::Object { properties }) => match (properties.get(&key), &value) {
                            (Some(Node::Optional { .. }), None) => {}
                            (Some(child), _) => container_issues(child, value, &mut path, &mut issues)?,
                            (None, Some(_)) if key != "$id" => issues.push(Issue { code: IssueCode::UnknownField, path }),
                            (None, _) => {}
                        },
                        Some(Node::Record { value: entry }) if value.is_some() => {
                            if valid_key(&key) {
                                container_issues(entry, value, &mut path, &mut issues)?;
                            } else {
                                issues.push(Issue { code: IssueCode::InvalidKey, path });
                            }
                        }
                        // An absent record entry has no issues.
                        _ => {}
                    }
                }
            }
        }
        sort_issues(&mut issues);
        let changed = issues != before;
        self.issues = issues;
        Ok(changed)
    }
    pub fn version(&self) -> String {
        version_token(&self.doc.oplog_frontiers())
    }
    /// The projected value. A document without issues stores every row's own unique
    /// `$id`, so its stored value projects exactly; anomalous rows need container
    /// identities for their effective IDs.
    fn projected(&self) -> Result<Value> {
        if self.issues.is_empty() {
            Ok(project(Some(&self.schema), raw(&self.doc)?))
        } else {
            project_at(&self.doc, Some(&self.schema), &self.doc.get_map("data").id())
        }
    }
    /// The application value as JSON.
    pub fn value(&self) -> Result<String> {
        encode(&self.projected()?)
    }
    /// `{sequence, version, value, issues}` for a page or a reader, with the issues the
    /// owner maintains. `snapshot` recomputes the same state from the full value.
    pub fn state(&self) -> Result<String> {
        encode(&State { version: self.version(), value: self.projected()?, issues: self.issues.clone(), sequence: self.sequence })
    }
    pub fn snapshot(&self) -> Result<String> {
        // Deliberately recomputed from the full value: this is the oracle that
        // incremental publications and issues are tested against.
        let raw = raw(&self.doc)?;
        let mut found = vec![];
        issues(&self.schema, &raw, None, &mut vec![], &mut found);
        // Without issues every row stores its own unique `$id`, so the stored value
        // projects exactly; anomalous rows need container identities for their IDs,
        // which also name them in issue paths.
        let value = if found.is_empty() {
            project(Some(&self.schema), raw)
        } else {
            let value = project_at(&self.doc, Some(&self.schema), &self.doc.get_map("data").id())?;
            found.clear();
            issues(&self.schema, &raw, Some(&value), &mut vec![], &mut found);
            sort_issues(&mut found);
            value
        };
        encode(
            &State { version: self.version(), value, issues: found, sequence: self.sequence },
        )
    }
    /// Loro transactions cannot be rolled back. Validation happens before each
    /// intent's first mutation, so a batch rejected at its first intent left nothing
    /// pending. When an earlier intent already mutated, rebuild the owner at the
    /// pre-batch frontiers (O(document), only on this rejection path). The pending
    /// operations were never exported; the rebuilt replica uses a fresh peer.
    fn abort(&mut self, before: &loro::Frontiers) -> Result<()> {
        if self.doc.get_pending_txn_len() == 0 {
            return Ok(());
        }
        let fresh = replica_at(&self.doc, before)?;
        self.events.lock().unwrap().clear();
        subscribe(&fresh, &self.events);
        // Container IDs survive the rebuild, so the published list indexes stay valid.
        self.doc = fresh;
        Ok(())
    }
    /// Merges another replica's updates; `None` when they changed nothing here.
    pub fn import(&mut self, bytes: &[u8]) -> Result<Option<String>> {
        if bytes.len() > MAX_BYTES {
            return Err(err(Code::TooLarge, "Import bytes"));
        }
        // Refuse a batch with missing dependencies before Loro buffers any of it.
        let meta = LoroDoc::decode_import_blob_meta(bytes, true)
            .map_err(|e| err(Code::InvalidBytes, e))?;
        // Refuse before mutation: a live owner must retain the full-history invariant.
        if meta.mode == loro::EncodedBlobMode::ShallowSnapshot {
            return Err(err(Code::InvalidBytes, "History-trimmed documents are not supported"));
        }
        let known = self.doc.oplog_vv();
        if meta
            .partial_start_vv
            .iter()
            .any(|(peer, start)| known.get(peer).copied().unwrap_or(0) < *start)
        {
            return Err(err(
                Code::MissingDependencies,
                "Durable pending-import buffering is not implemented",
            ));
        }
        match self.doc.import(bytes) {
            Ok(_) => self.publish(),
            // Loro applies the changes it can and refuses those that depend on history
            // this document trimmed; what landed is published before the refusal.
            Err(loro::LoroError::ImportUpdatesThatDependsOnOutdatedVersion) => {
                self.publish()?;
                Err(err(Code::StaleBase, "Updates depend on history this document trimmed"))
            }
            Err(e) => Err(err(Code::InvalidBytes, e)),
        }
    }
    /// The publication sequence: the number of published changes since open.
    pub fn sequence(&self) -> u64 {
        self.sequence
    }
    /// Applies a batch atomically; the result is a record so hosts never parse the reply.
    pub fn apply_batch(&mut self, batch: &str) -> Result<Applied> {
        let batch: Batch = parse(batch)?;
        if batch.intents.len() > 1000 {
            return Err(err(Code::TooLarge, "Batch exceeds 1000 intents"));
        }
        let before = self.doc.state_frontiers();
        let mut ids = vec![];
        let mut failure = None;
        {
            let mut rows = Rows::new(&self.lists);
            for (index, op) in batch.intents.iter().enumerate() {
                if let Err(mut e) = execute(&self.doc, &self.schema, op, &mut ids, &mut rows)
                {
                    e.op_index = Some(index);
                    failure = Some(e);
                    break;
                }
            }
        }
        if let Some(e) = failure {
            self.abort(&before)?;
            return Err(e);
        }
        self.doc.commit();
        let publication = self.publish()?;
        Ok(Applied { sequence: self.sequence, ids, publication })
    }
    /// Publishes the committed events as one change: `{previous, sequence, version, ops,
    /// issues?}`. Applying it to the previous snapshot yields a fresh snapshot. `None`
    /// when the document did not change: no publication, and the sequence stays.
    fn publish(&mut self) -> Result<Option<String>> {
        let events = std::mem::take(&mut *self.events.lock().unwrap());
        let Some(published) = publication::publish(&self.doc, &self.schema, &mut self.lists, events)? else {
            return Ok(None);
        };
        let changed = (published.rescan || !self.issues.is_empty()) && self.refresh_issues(&published.dirty)?;
        let next = self
            .sequence
            .checked_add(1)
            .ok_or_else(|| err(Code::TooLarge, "Publication sequence"))?;
        let issues = changed.then(|| self.issues.clone());
        let response = encode(&Publication { previous: self.sequence, sequence: next, version: self.version(), ops: published.ops, issues })?;
        self.sequence = next;
        Ok(Some(response))
    }
    /// Storage calls this when a checkpoint keeps only the history since `start`.
    #[cfg(feature = "storage")]
    pub(crate) fn retain_from(&mut self, start: &Frontiers) {
        self.floor = self.doc.frontiers_to_vv(start).unwrap_or_else(|| self.doc.oplog_vv());
    }
    pub fn checkpoint(&self) -> Result<Vec<u8>> {
        self.doc.export(ExportMode::Snapshot).map_err(engine)
    }
    pub fn export_since(&self, version: &str) -> Result<Vec<u8>> {
        let (_, vv) = decode_version(&self.doc, version)?;
        self.doc.export(ExportMode::updates(&vv)).map_err(engine)
    }
}
/// A new long-lived replica holding exactly the history up to `frontiers`, with its own
/// peer. Built by replaying the operations from where `doc`'s history starts (a trimmed
/// document's starting state, else nothing), never with `LoroDoc::fork_at`: a `fork_at`
/// replica of an older version can later resolve concurrent map writes differently from
/// every other replica (reproduced with plain Loro 1.16.2; see tests/replica.rs).
pub(crate) fn replica_at(doc: &LoroDoc, frontiers: &Frontiers) -> Result<LoroDoc> {
    // Measured before any export commits pending operations; those lie outside `vv`.
    let vv = doc.frontiers_to_vv(frontiers).ok_or_else(|| engine("Version is not in history"))?;
    let start = doc.shallow_since_vv().to_vv();
    let replica = LoroDoc::new();
    if doc.is_shallow() {
        let base = doc.export(ExportMode::state_only(Some(&doc.shallow_since_frontiers()))).map_err(engine)?;
        replica.import(&base).map_err(engine)?;
    }
    let spans: Vec<_> = vv.sub_iter(&start).collect();
    replica.import(&doc.export(ExportMode::updates_in_range(spans)).map_err(engine)?).map_err(engine)?;
    Ok(replica)
}
/// Callers bound the total input; a pending status means missing dependencies.
fn checked(imported: loro::LoroResult<loro::ImportStatus>) -> Result<()> {
    let status = imported.map_err(|e| err(Code::InvalidBytes, e))?;
    if status.pending.as_ref().is_some_and(|v| !v.is_empty()) {
        return Err(err(
            Code::MissingDependencies,
            "Durable pending-import buffering is not implemented",
        ));
    }
    Ok(())
}

#[cfg(all(feature = "schema-validation", not(target_arch = "wasm32")))]
pub mod envelope;
#[cfg(all(feature = "storage", not(target_arch = "wasm32")))]
pub mod store;
#[cfg(all(feature = "schema-validation", not(target_arch = "wasm32")))]
pub mod manifest;
