//! Shared native/WASM document semantics.

#[cfg(feature = "ts")]
pub mod bindings;
mod replication;
mod wire;
use loro::{
    Container, ContainerID, ContainerTrait, ExportMode, Frontiers, ID, Index, LoroDoc, LoroMap, LoroMovableList,
    LoroText, ValueOrContainer, VersionVector,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque};
#[cfg(feature = "storage")]
pub use wire::build;
#[cfg(feature = "storage")]
pub use wire::engine;
#[cfg(feature = "storage")]
pub use wire::engine::{EngineReply, EngineRequest, EngineSuccess};
#[cfg(feature = "storage")]
pub use wire::{OutcomeCode, native, page as page_wire, preview, socket as socket_wire};
#[cfg(feature = "storage")]
pub mod app;
#[cfg(feature = "storage")]
pub use wire::{HELPER_PROTOCOL, HostLimits, NATIVE_RESOURCE_POLICY, host_limits};
pub mod arguments;
mod check;
mod descriptor;
mod execute;
mod identity;
#[cfg(feature = "storage")]
pub mod images;
#[cfg(feature = "storage")]
mod maintenance;
#[cfg(feature = "storage")]
pub mod media;
mod project;
mod publication;
mod replace;
pub mod shape;
#[cfg(all(test, feature = "storage"))]
mod testing;
mod text;
pub mod theme;
pub use descriptor::validate;
use descriptor::{Node, descriptor, holds_collections, is_scalar, loro_scalar, unwrap_optional, valid_key};
use execute::{Rows, execute, fill, put, resolve};
use identity::stored_id;
use project::project;
use publication::{Events, ListState};
use std::sync::Arc;
use wire::valid_id;
pub use wire::{
    ASSET_BYTES, ASSET_COUNT, ASSET_FILE_BYTES, ATTACHMENT_BYTES, ATTACHMENT_COUNT, ATTACHMENT_FILE_BYTES, Code,
    IMAGE_PIXELS, IMAGE_SIDE, PACKAGE_FORMAT, RUNTIME_ABI, STORAGE_BYTES, STORAGE_ROWS,
};
pub use wire::{Anchor, Batch, Hunk, Intent, OwnerState, PatchOp, Publication, Reading, Segment, Selection, ThemeFile};

/// The largest JSON text the core parses: a page request, or an app's initial values.
const MAX_JSON: usize =
    if wire::APP_TEXT_BYTES > wire::PAGE_PAYLOAD { wire::APP_TEXT_BYTES } else { wire::PAGE_PAYLOAD };

#[derive(Debug, thiserror::Error)]
#[error("{code}: {message}")]
pub struct Error {
    pub code: Code,
    pub message: String,
    pub op_index: Option<usize>,
    /// Location within a checked value; independent of the batch's operation index.
    pub path: Vec<String>,
}
impl Error {
    fn at(mut self, segment: impl ToString) -> Self {
        self.path.insert(0, segment.to_string());
        self
    }
    /// RFC 6901 pointer; an empty string identifies the checked value itself.
    pub fn pointer(&self) -> String {
        self.path.iter().map(|s| format!("/{}", s.replace('~', "~0").replace('/', "~1"))).collect()
    }
}
type Result<T> = std::result::Result<T, Error>;
fn err(code: Code, message: impl ToString) -> Error {
    Error { code, message: message.to_string(), op_index: None, path: vec![] }
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
/// JSON text of a value the core built: string keys and finite or null numbers only.
fn encode(v: &impl Serialize) -> String {
    serde_json::to_string(v).expect("core values encode as JSON")
}
/// A materialized Loro value as JSON.
fn json(value: loro::LoroValue) -> Value {
    serde_json::to_value(value).expect("Loro values encode as JSON")
}
/// `mutex`'s value, also after a thread panicked holding it: no value guarded here is left
/// half-changed by a panic.
pub(crate) fn lock<T>(mutex: &std::sync::Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|e| e.into_inner())
}
/// Fills `buffer` from the platform's randomness, without which no identity can be minted.
fn random(buffer: &mut [u8]) {
    getrandom::getrandom(buffer).expect("random bytes");
}
/// `bytes` random bytes in lowercase hex.
#[cfg(feature = "storage")]
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn random_hex(bytes: usize) -> String {
    let mut buffer = vec![0u8; bytes];
    random(&mut buffer);
    hex(&buffer)
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
    if s.len() > 2 * MAX_TOKEN_BYTES || !s.len().is_multiple_of(2) || !s.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(err(Code::InvalidVersion, "Expected an opaque version token"));
    }
    (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).map_err(engine)).collect()
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
/// JSON integers are integral numeric values, including `1.0`, `1e2` and `-0.0`.
/// Canonicalize only after proving the value fits JavaScript's exact integer range.
fn integer(value: &Value) -> Option<i64> {
    let n = value.as_f64()?;
    (n.is_finite() && n.fract() == 0.0 && n.abs() <= MAX_SAFE as f64).then_some(n as i64)
}
fn application_id() -> String {
    let mut bytes = [0u8; 16];
    random(&mut bytes);
    let mut buffer = 0u32;
    let mut bits = 0;
    let mut out = String::new();
    for byte in bytes {
        buffer = (buffer << 8) | u32::from(byte);
        bits += 8;
        while bits >= 5 {
            bits -= 5;
            out.push(wire::ID_ALPHABET[((buffer >> bits) & 31) as usize] as char);
        }
    }
    out.push(wire::ID_ALPHABET[((buffer << (5 - bits)) & 31) as usize] as char);
    out
}

fn raw(doc: &LoroDoc) -> Value {
    json(doc.get_map("data").get_deep_value())
}
fn subscribe(doc: &LoroDoc, events: &Events) {
    // The subscription lives exactly as long as this LoroDoc; a replaced doc drops it.
    publication::subscribe(doc, events).detach();
}

/// The peer a template's initial operations belong to (`Document::initial_checkpoint`).
#[cfg(feature = "storage")]
#[cfg(not(target_arch = "wasm32"))]
const TEMPLATE_PEER: u64 = 1;
/// `doc` with the layout marker and `value`, a validated value of `schema`, committed.
fn filled(doc: LoroDoc, schema: &Node, value: &Value) -> Result<LoroDoc> {
    doc.set_next_commit_message("create");
    doc.get_map(META).insert("layout", LAYOUT).map_err(engine)?;
    fill(&doc.get_map("data"), schema, value, &mut Rows::new(&HashMap::new()))?;
    doc.commit();
    Ok(doc)
}
/// Gives every row in `value` that has no `$id` one derived from its place (`at`).
#[cfg(feature = "storage")]
#[cfg(not(target_arch = "wasm32"))]
fn name_rows(node: &Node, value: &mut Value, at: &str) {
    match (node, value) {
        (Node::Optional { inner }, value) => name_rows(inner, value, at),
        (Node::Object { properties }, Value::Object(fields)) => {
            for (key, child) in properties {
                if let Some(field) = fields.get_mut(key) {
                    name_rows(child, field, &format!("{at}/{key}"));
                }
            }
        }
        (Node::Record { value: entry }, Value::Object(entries)) => {
            for (key, field) in entries.iter_mut() {
                name_rows(entry, field, &format!("{at}/{key}"));
            }
        }
        (Node::List { item }, Value::Array(rows)) if matches!(**item, Node::Object { .. }) => {
            for (index, row) in rows.iter_mut().enumerate() {
                let place = format!("{at}/{index}");
                if let Value::Object(fields) = row {
                    fields
                        .entry("$id")
                        .or_insert_with(|| Value::String(identity::derived(&format!("initial:{place}"))));
                }
                name_rows(item, row, &place);
            }
        }
        _ => {}
    }
}

/// Semantic seed equality: object order and integral number spelling may differ,
/// but converting an integer to a double must not silently round its value.
#[cfg(feature = "storage")]
#[cfg(not(target_arch = "wasm32"))]
fn same_seed(a: &Value, b: &Value) -> bool {
    fn integer_float(integer: &serde_json::Number, float: f64) -> bool {
        if float.fract() != 0.0 {
            return false;
        }
        if let Some(n) = integer.as_i64() {
            float >= i64::MIN as f64 && float < i64::MAX as f64 && float as i64 == n
        } else if let Some(n) = integer.as_u64() {
            float >= 0.0 && float < u64::MAX as f64 && float as u64 == n
        } else {
            false
        }
    }
    match (a, b) {
        (Value::Number(a), Value::Number(b)) if a != b => match (a.is_f64(), b.is_f64()) {
            (false, true) => integer_float(a, b.as_f64().expect("float")),
            (true, false) => integer_float(b, a.as_f64().expect("float")),
            _ => false,
        },
        (Value::Array(a), Value::Array(b)) => a.len() == b.len() && a.iter().zip(b).all(|(a, b)| same_seed(a, b)),
        (Value::Object(a), Value::Object(b)) => {
            a.len() == b.len() && a.iter().all(|(key, a)| b.get(key).is_some_and(|b| same_seed(a, b)))
        }
        _ => a == b,
    }
}

/// Identifies the core build (a hash of its sources and the lockfile), so a release can
/// prove the app and its helper embed the same core.
pub const BUILD_ID: &str = env!("HITSLOP_CORE_BUILD_ID");

/// The document layout: how descriptor kinds map to Loro containers
/// (docs/reference/document-types.md). A compatibility requirement, not a release number,
/// written into each document's `meta` map when it is created. A build refuses a newer one.
pub const LAYOUT: i64 = 1;
const META: &str = "meta";
/// Refuses a document whose layout this build cannot read, before interpreting it. Each
/// layout a released build wrote keeps its arm: raising `LAYOUT` adds an arm that reads
/// the new layout and leaves the old ones reading (or migrating losslessly in memory).
fn check_layout(doc: &LoroDoc) -> Result<()> {
    match doc.get_map(META).get("layout") {
        Some(ValueOrContainer::Value(loro::LoroValue::I64(1))) => Ok(()),
        Some(ValueOrContainer::Value(loro::LoroValue::I64(layout))) if layout > LAYOUT => Err(err(
            Code::RequiresUpdate,
            format!("This document uses layout {layout}; this hitSlop reads layout {LAYOUT}"),
        )),
        _ => Err(err(Code::InvalidBytes, "Document has no supported layout")),
    }
}

/// Who made a change: the person, in the page or in the window's own controls (the theme
/// panel), or an agent (the CLI and socket). All are undoable; an agent's consecutive
/// batches are one undo step. Only the window and agents change the palette.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Origin {
    Page,
    Window,
    Agent,
}
impl Origin {
    fn message(self) -> &'static str {
        match self {
            Self::Page => "page",
            Self::Window => "window",
            Self::Agent => "agent",
        }
    }
}
/// The undo step being extended: a typing run in one text field (its text and caret, in
/// UTF-16, after the last edit), a run of agent batches, or a run of the window's changes
/// to one palette color (a color panel drag).
#[derive(Clone, PartialEq)]
enum Run {
    Typing { path: Vec<Segment>, text: String, caret: usize },
    Agent,
    Color(String),
}
/// One document edit, restored by Loro as a new change. Only version references are
/// kept here; document values and their history remain in Loro.
#[derive(Clone)]
struct Step {
    before: Frontiers,
    after: Frontiers,
}
/// Undo covers the open session only: a document opens with nothing to undo.
const UNDO_STEPS: usize = 100;

/// A committed batch: its publication sequence, the IDs of inserted rows, the publication
/// to deliver (absent when the batch changed nothing), whether it changed the palette and,
/// for the page's text edit, what the page continues from.
#[derive(Debug)]
pub struct Applied {
    pub sequence: u64,
    pub ids: Vec<String>,
    pub publication: Option<String>,
    pub theme_changed: bool,
    pub text: Option<TextEdit>,
}
/// A published change: its JSON, and whether it changed the palette.
struct Published {
    json: String,
    theme: bool,
}
/// What a document is an instance of, from its app's immutable row: the descriptor, and
/// the palette its template declares.
#[derive(Clone, Debug)]
pub struct AppSpec {
    schema: Node,
    theme: theme::Theme,
}
impl AppSpec {
    /// `slug` names the template in theme files; `theme` is its declared colors (JSON).
    pub fn new(descriptor: &str, slug: &str, theme: &str) -> Result<Self> {
        Ok(Self::of(self::descriptor(descriptor)?, slug, theme::validate_defaults(theme)?))
    }
    /// An app that declares no colors.
    pub fn data(descriptor: &str) -> Result<Self> {
        Self::new(descriptor, "", "{}")
    }
    pub(crate) fn of(schema: Node, slug: &str, tokens: Vec<(String, String)>) -> Self {
        Self { schema, theme: theme::Theme::new(slug, tokens) }
    }
    /// The template's slug, which names it in theme files.
    pub fn slug(&self) -> &str {
        self.theme.template()
    }
    /// The declared colors, in the order the author wrote them.
    pub fn theme_tokens(&self) -> &[(String, String)] {
        self.theme.tokens()
    }
}
/// The page's text edit, a batch whose set carries `selection`. `authored` is the version
/// right after the edit on its own branch; the page sends it as the next `base`. The
/// selection is in UTF-16 offsets of the merged text.
#[derive(Debug)]
pub struct TextEdit {
    pub authored: String,
    pub selection: [usize; 2],
}

/// Exactly one host executor owns this value. Neither binding contains semantics.
pub struct Document {
    doc: LoroDoc,
    app: AppSpec,
    sequence: u64,
    /// Every movable list's order and row identities as of the last publication.
    lists: HashMap<ContainerID, ListState>,
    events: Events,
    /// Where the saved history starts once the latest checkpoint is written. A
    /// concurrent edit must not branch from before it: its saved operations would depend
    /// on history the checkpoint drops, and the document could not open again.
    floor: VersionVector,
    undo: VecDeque<Step>,
    redo: Vec<Step>,
    /// Consecutive edits at the caret of one text field, consecutive agent batches, or
    /// consecutive window changes to one color are one undo step.
    run: Option<Run>,
    /// A refused edit could not be rolled back: the replica may hold part of it, so it
    /// accepts nothing more. Its host reloads durable state.
    broken: bool,
}
impl Document {
    /// `check` is false only for a document just built from validated input; stored state
    /// is checked against its app before anything reads it.
    fn from_doc(doc: LoroDoc, app: AppSpec, check: bool) -> Result<Self> {
        if check {
            check::stored(&app.schema, Some(ValueOrContainer::Container(Container::Map(doc.get_map("data")))))?;
            app.theme.check_stored(&doc.get_map(theme::ROOT))?;
        }
        doc.set_record_timestamp(true);
        let events = Events::default();
        subscribe(&doc, &events);
        let this = Self {
            lists: publication::index_all(&doc),
            undo: VecDeque::new(),
            redo: vec![],
            run: None,
            broken: false,
            doc,
            app,
            sequence: 0,
            events,
            floor: VersionVector::default(),
        };
        Ok(this)
    }
    /// A new document of `app` holding `initial`, its values as JSON.
    pub fn create(app: &AppSpec, initial: &str) -> Result<Self> {
        let mut initial: Value = parse(initial)?;
        app.schema.fill_defaults(&mut initial);
        app.schema.validate(&initial, false)?;
        let doc = LoroDoc::new();
        doc.set_record_timestamp(true);
        let doc = filled(doc, &app.schema, &initial)?;
        Self::from_doc(doc, app.clone(), false)
    }
    /// A template's initial state, as the checkpoint every document of it starts from. The
    /// same app always packs the same bytes, so a rebuild reproduces its template: rows
    /// without an `$id` get one derived from their place, and the operations belong to a
    /// fixed peer. A document that opens it edits as a peer of its own.
    #[cfg(feature = "storage")]
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn initial_checkpoint(app: &AppSpec, initial: &str) -> Result<Vec<u8>> {
        let mut initial: Value = parse(initial)?;
        app.schema.fill_defaults(&mut initial);
        app.schema.validate(&initial, false)?;
        name_rows(&app.schema, &mut initial, "");
        let doc = LoroDoc::new();
        doc.set_peer_id(TEMPLATE_PEER).map_err(engine)?;
        let checkpoint = filled(doc, &app.schema, &initial)?.export(ExportMode::Snapshot).map_err(engine)?;
        let reopened = Self::open(app, &checkpoint, &[])?;
        let saved: Value = parse(&reopened.value())?;
        if !same_seed(&initial, &saved) {
            return Err(err(Code::InvalidRequest, "The initial checkpoint would change the declared values"));
        }
        Ok(checkpoint)
    }
    /// A saved document of `app`: its checkpoint and the updates saved after it.
    pub fn open(app: &AppSpec, checkpoint: &[u8], updates: &[Vec<u8>]) -> Result<Self> {
        let total = updates
            .iter()
            .try_fold(checkpoint.len(), |n, b| n.checked_add(b.len()))
            .ok_or_else(|| err(Code::TooLarge, "Input bytes"))?;
        if total > STORAGE_BYTES {
            return Err(err(Code::TooLarge, "Input bytes"));
        }
        Self::open_with(app, checkpoint, |e| e, |import| updates.iter().try_for_each(|bytes| import(bytes)))
    }
    /// Imports `checkpoint`, then each update `updates` passes to its import function, in
    /// order. Storage streams rows through it without copying them, failing in its own
    /// error type (`core` wraps the document's); callers bound the total size first.
    pub(crate) fn open_with<E>(
        app: &AppSpec,
        checkpoint: &[u8],
        core: impl Fn(Error) -> E,
        updates: impl FnOnce(&mut dyn FnMut(&[u8]) -> std::result::Result<(), E>) -> std::result::Result<(), E>,
    ) -> std::result::Result<Self, E> {
        let doc = LoroDoc::new();
        // A checkpoint may start its history late; the updates after it never move that start.
        imported(doc.import(checkpoint)).map_err(&core)?;
        let trimmed = doc.shallow_since_vv();
        // One import per update measured faster than Loro's `import_batch` here.
        updates(&mut |bytes: &[u8]| {
            imported(doc.import(bytes)).map_err(&core)?;
            if doc.shallow_since_vv() != trimmed {
                return Err(core(err(Code::InvalidBytes, "Only a checkpoint may trim history")));
            }
            Ok(())
        })?;
        check_layout(&doc).map_err(&core)?;
        Self::from_doc(doc, app.clone(), true).map_err(core)
    }
    pub fn version(&self) -> String {
        version_token(&self.doc.oplog_frontiers())
    }
    /// The application value: the stored value, each counter and number as it reads.
    fn projected(&self) -> Value {
        project(Some(&self.app.schema), raw(&self.doc))
    }
    /// The application value as JSON.
    pub fn value(&self) -> String {
        encode(&self.projected())
    }
    /// `{sequence, version, value, theme}` for a page: the reading, led by the publication
    /// sequence that orders the page's stream. The oracle that publications replayed on a
    /// page are tested against.
    pub fn state(&self) -> Result<String> {
        Ok(encode(&self.reading()?.sequenced(self.sequence)))
    }
    /// `{version, value, theme}`: the document as it reads, computed from the full stored
    /// value.
    pub fn reading(&self) -> Result<Reading> {
        Ok(Reading {
            version: self.version(),
            value: self.projected(),
            theme: self.app.theme.effective(&self.doc.get_map(theme::ROOT))?,
        })
    }
    /// Rebuilds the owner at the pre-call version after a partial mutation. This also
    /// handles one replace that failed after changing an earlier field. The history
    /// references survive replay, including redo; rejected operations were never exported.
    fn abort(&mut self, before: &loro::Frontiers) -> Result<()> {
        if self.doc.get_pending_txn_len() == 0 && self.doc.state_frontiers() == *before {
            return Ok(());
        }
        self.rebuild_at(before)
    }
    /// Replaces the Loro document with one replayed to `before`, under a new peer.
    /// Unconditional: an import Loro refused can keep operations whose dependencies are
    /// missing without moving its frontiers.
    fn rebuild_at(&mut self, before: &loro::Frontiers) -> Result<()> {
        let fresh = replica_at(&self.doc, before).inspect_err(|_| self.broken = true)?;
        lock(&self.events).clear();
        subscribe(&fresh, &self.events);
        self.doc = fresh;
        // Publication may have failed after updating indexes; rebuild those too.
        self.lists = publication::index_all(&self.doc);
        Ok(())
    }
    /// Whether a failed rollback left this replica unusable; see `broken`.
    pub fn is_broken(&self) -> bool {
        self.broken
    }
    fn intact(&self) -> Result<()> {
        if self.broken {
            Err(err(Code::EngineError, "The document could not be rolled back; reload it"))
        } else {
            Ok(())
        }
    }
    /// The publication sequence: the number of published changes since open.
    pub fn sequence(&self) -> u64 {
        self.sequence
    }
    /// The palette: the template's colors, the overrides and the result.
    pub fn theme_state(&self) -> Result<theme::ThemeState> {
        self.app.theme.state(&self.doc.get_map(theme::ROOT))
    }
    /// The full palette as a theme file, the bytes every export writes.
    pub fn export_theme(&self) -> Result<String> {
        self.app.theme.export(&self.doc.get_map(theme::ROOT))
    }
    /// Applies a batch atomically; the result is a record so hosts never parse the reply.
    pub fn apply_batch(&mut self, batch: Batch, origin: Origin) -> Result<Applied> {
        self.apply(batch, origin, None)
    }
    /// A command's intents: one undo step, labeled by the app-declared command the owner
    /// evaluated. No wire names a command for a batch.
    pub fn apply_command(&mut self, batch: Batch, origin: Origin, command: &str) -> Result<Applied> {
        self.apply(batch, origin, Some(command))
    }
    fn apply(&mut self, batch: Batch, origin: Origin, command: Option<&str>) -> Result<Applied> {
        self.intact()?;
        batch.check_size(wire::PAGE_PAYLOAD)?;
        if let Some(expected) = &batch.ifVersion
            && *expected != self.version()
        {
            return Err(err(Code::StaleBase, "The document changed since the command read it"));
        }
        if batch.intents.len() > wire::BATCH_INTENTS {
            return Err(err(Code::TooLarge, format!("Batch exceeds {} intents", wire::BATCH_INTENTS)));
        }
        if batch.intents.len() > 1
            && batch.intents.iter().any(|op| matches!(op, Intent::Set { selection: Some(_), .. }))
        {
            return Err(err(Code::InvalidRequest, "A text edit with a selection is its own batch"));
        }
        // Every path validates the base first: an unknown operation must never reach Loro.
        let base = match batch.base {
            Some(token) => {
                let (at, vv) = decode_version(&self.doc, &token)?;
                Some(text::Base { token, at, vv })
            }
            None => None,
        };
        let before = self.doc.state_frontiers();
        let mut ids = vec![];
        let mut failure = None;
        let message = command.map_or_else(|| origin.message().to_owned(), |name| format!("command:{name}"));
        self.doc.set_next_commit_message(&message);
        let typed = {
            let mut rows = Rows::new(&self.lists);
            let mut texts = text::Texts { base: base.as_ref(), floor: &self.floor, message: &message, typed: None };
            for (index, op) in batch.intents.iter().enumerate() {
                let result =
                    if origin == Origin::Page && matches!(op, Intent::SetTheme { .. } | Intent::ImportTheme { .. }) {
                        Err(err(Code::InvalidRequest, "The page cannot change the palette"))
                    } else {
                        execute(&self.doc, &self.app, op, &mut ids, &mut rows, &mut texts)
                    };
                if let Err(mut e) = result {
                    e.op_index = Some(index);
                    failure = Some(e);
                    break;
                }
            }
            texts.typed
        };
        if let Some(e) = failure {
            self.abort(&before)?;
            return Err(e);
        }
        self.doc.set_next_commit_message(&message);
        self.doc.commit();
        let published = self.publish_or_abort(&before)?;
        if published.is_some() {
            match &typed {
                _ if command.is_some() => self.record(before, None, false),
                // A merged edit ends the typing run and is its own undo step.
                Some(typed) if origin != Origin::Agent && typed.merged => self.record(before, None, false),
                Some(typed) if origin != Origin::Agent => {
                    self.record_typing(before, &typed.path, &typed.from, &typed.to, typed.caret)
                }
                _ => {
                    let run = match (origin, batch.intents.as_slice()) {
                        (Origin::Agent, _) => Some(Run::Agent),
                        // One color set, not reset: a color panel sends one per step of a drag.
                        (Origin::Window, [Intent::SetTheme { values, replace: None | Some(false) }]) => {
                            match values.first_key_value() {
                                Some((token, Some(_))) if values.len() == 1 => Some(Run::Color(token.clone())),
                                _ => None,
                            }
                        }
                        _ => None,
                    };
                    let continues = run.is_some() && run == self.run;
                    self.record(before, run, continues);
                }
            }
        }
        // An edit applied to the live text has the batch's version as its own.
        let text = typed.map(|typed| TextEdit {
            authored: typed.authored.unwrap_or_else(|| self.version()),
            selection: typed.selection,
        });
        Ok(Self::applied(self.sequence, ids, published, text))
    }
    fn applied(sequence: u64, ids: Vec<String>, published: Option<Published>, text: Option<TextEdit>) -> Applied {
        let theme_changed = published.as_ref().is_some_and(|p| p.theme);
        Applied { sequence, ids, publication: published.map(|p| p.json), theme_changed, text }
    }
    /// Reverts the person's last undo step, or reapplies the last undone one. Nothing to
    /// undo publishes nothing.
    pub fn undo(&mut self) -> Result<Applied> {
        self.history(true)
    }
    pub fn redo(&mut self) -> Result<Applied> {
        self.history(false)
    }
    fn history(&mut self, undo: bool) -> Result<Applied> {
        self.intact()?;
        let target = if undo {
            self.undo.back().map(|step| step.before.clone())
        } else {
            self.redo.last().map(|step| step.after.clone())
        };
        let Some(target) = target else {
            self.run = None;
            return Ok(Self::applied(self.sequence, vec![], None, None));
        };
        let before = self.doc.state_frontiers();
        // A new change that makes the document what it was at `target`. Loro applies it
        // all or nothing, so a refusal leaves the document as it was.
        self.doc.set_next_commit_message(if undo { "undo" } else { "redo" });
        self.doc.revert_to(&target).map_err(|e| match e {
            loro::LoroError::SwitchToVersionBeforeShallowRoot | loro::LoroError::FrontiersNotFound(_) => {
                err(Code::StaleBase, "That version precedes this document's retained history")
            }
            e => engine(e),
        })?;
        self.doc.set_next_commit_message(if undo { "undo" } else { "redo" });
        self.doc.commit();
        let published = self.publish_or_abort(&before)?;
        // A refused revert leaves the stacks and grouping untouched.
        if undo {
            self.redo.push(self.undo.pop_back().expect("checked"));
        } else {
            self.undo.push_back(self.redo.pop().expect("checked"));
        }
        self.run = None;
        Ok(Self::applied(self.sequence, vec![], published, None))
    }
    /// Records only a successfully published edit. No-op edits and refusals preserve
    /// both the current run and redo. Extending a run keeps its original before-version.
    fn record(&mut self, before: Frontiers, run: Option<Run>, continues: bool) {
        let after = self.doc.state_frontiers();
        self.redo.clear();
        if let Some(step) = self.undo.back_mut().filter(|_| continues) {
            step.after = after;
        } else {
            self.undo.push_back(Step { before, after });
            if self.undo.len() > UNDO_STEPS {
                self.undo.pop_front();
            }
        }
        self.run = run;
    }
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }
    /// Starts a new undo step unless this edit continues the typing run: the same field,
    /// unchanged since the last edit, changed at the caret that edit left.
    fn record_typing(&mut self, before: Frontiers, path: &[Segment], from: &str, to: &str, caret: usize) {
        let continues = matches!(&self.run, Some(Run::Typing { path: p, text, caret: at }) if p == path && text == from && {
            let (before, after): (Vec<u16>, Vec<u16>) = (from.encode_utf16().collect(), to.encode_utf16().collect());
            let prefix = before.iter().zip(&after).take_while(|(a, b)| a == b).count();
            let suffix = before[prefix..].iter().rev().zip(after[prefix..].iter().rev()).take_while(|(a, b)| a == b).count();
            (prefix..=before.len() - suffix).contains(at)
        });
        self.record(before, Some(Run::Typing { path: path.to_vec(), text: to.to_owned(), caret }), continues);
    }
    /// No publication has escaped when this fails, so restore the document and indexes
    /// before reporting a refusal. History is updated only after this succeeds.
    fn publish_or_abort(&mut self, before: &Frontiers) -> Result<Option<Published>> {
        match self.publish() {
            Ok(publication) => Ok(publication),
            Err(error) => {
                self.abort(before)?;
                Err(error)
            }
        }
    }
    /// Publishes the committed events as one change: `{previous, sequence, version, ops,
    /// theme?}`. Applying it to the previous snapshot yields a fresh snapshot. `None`
    /// when the document did not change: no publication, and the sequence stays.
    fn publish(&mut self) -> Result<Option<Published>> {
        self.publish_with(false)
    }
    /// A publication of what changed, or with `always`, one that only names the new
    /// version when nothing visible did (an import of operations with no visible effect).
    fn publish_with(&mut self, always: bool) -> Result<Option<Published>> {
        let events = std::mem::take(&mut *lock(&self.events));
        let theme = publication::theme_changed(&self.doc, &events)
            .then(|| self.app.theme.effective(&self.doc.get_map(theme::ROOT)))
            .transpose()?;
        let ops = publication::publish(&self.doc, &self.app.schema, &mut self.lists, events)?;
        if ops.is_none() && theme.is_none() && !always {
            return Ok(None);
        }
        let next = self.sequence.checked_add(1).ok_or_else(|| err(Code::TooLarge, "Publication sequence"))?;
        let themed = theme.is_some();
        let json = encode(&Publication {
            previous: self.sequence,
            sequence: next,
            version: self.version(),
            ops: ops.unwrap_or_default(),
            theme,
        });
        self.sequence = next;
        Ok(Some(Published { json, theme: themed }))
    }
    /// Storage calls this when a checkpoint keeps only the history since `start`.
    #[cfg(feature = "storage")]
    pub(crate) fn retain_from(&mut self, start: &Frontiers) {
        self.floor = self.doc.frontiers_to_vv(start).unwrap_or_else(|| self.doc.oplog_vv());
    }
    /// The `stored` attachment IDs this state names: each one that appears in a string, a
    /// text or a map key, alone or inside longer text such as markdown. A false match only
    /// keeps a blob, so a blob the state references is never left out.
    #[cfg(feature = "storage")]
    pub(crate) fn attachment_references(&self, stored: &[String]) -> HashSet<String> {
        let stored: HashSet<&str> = stored.iter().map(String::as_str).collect();
        let mut found = HashSet::new();
        let mut scan = |text: &str| {
            for run in text.split(|c: char| !matches!(c, '0'..='9' | 'a'..='f')).filter(|run| run.len() >= 64) {
                for start in 0..=run.len() - 64 {
                    if let Some(id) = stored.get(&run[start..start + 64]) {
                        found.insert(id.to_string());
                    }
                }
            }
        };
        let root = self.doc.get_deep_value();
        let mut pending = vec![&root];
        while let Some(value) = pending.pop() {
            match value {
                loro::LoroValue::String(text) => scan(text),
                loro::LoroValue::List(items) => pending.extend(items.iter()),
                loro::LoroValue::Map(entries) => {
                    for (key, value) in entries.iter() {
                        scan(key);
                        pending.push(value);
                    }
                }
                _ => {}
            }
        }
        found
    }
    pub fn checkpoint(&self) -> Result<Vec<u8>> {
        self.doc.export(ExportMode::Snapshot).map_err(engine)
    }
}
/// A new long-lived replica holding exactly the history up to `frontiers`, with its own
/// peer. Built by replaying the operations from where `doc`'s history starts (a trimmed
/// document's starting state, else nothing), because Loro does not implement
/// `LoroDoc::fork_at` for trimmed documents.
pub(crate) fn replica_at(doc: &LoroDoc, frontiers: &Frontiers) -> Result<LoroDoc> {
    // Measured before any export commits pending operations; those lie outside `vv`.
    let vv = doc.frontiers_to_vv(frontiers).ok_or_else(|| engine("Version is not in history"))?;
    let start = doc.shallow_since_vv().to_vv();
    let replica = LoroDoc::new();
    replica.set_record_timestamp(true);
    if doc.is_shallow() {
        let base = doc.export(ExportMode::state_only(Some(&doc.shallow_since_frontiers()))).map_err(engine)?;
        replica.import(&base).map_err(engine)?;
    }
    let spans: Vec<_> = vv.sub_iter(&start).collect();
    replica.import(&doc.export(ExportMode::updates_in_range(spans)).map_err(engine)?).map_err(engine)?;
    Ok(replica)
}
/// Callers bound the total input; a pending status means missing dependencies.
fn imported(result: loro::LoroResult<loro::ImportStatus>) -> Result<()> {
    let status = result.map_err(|e| err(Code::InvalidBytes, e))?;
    if status.pending.as_ref().is_some_and(|v| !v.is_empty()) {
        return Err(err(Code::MissingDependencies, "Durable pending-import buffering is not implemented"));
    }
    Ok(())
}

#[cfg(feature = "storage")]
mod error;
#[cfg(feature = "storage")]
pub mod file;
#[cfg(all(feature = "storage", not(target_arch = "wasm32")))]
pub mod registry;
#[cfg(feature = "storage")]
pub mod store;

#[cfg(feature = "storage")]
pub mod owner;

#[cfg(feature = "storage")]
pub mod command;
pub mod describe;
#[cfg(all(feature = "storage", not(target_arch = "wasm32")))]
pub mod socket;

#[cfg(feature = "storage")]
pub use wire::browser as browser_wire;
