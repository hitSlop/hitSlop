//! A document's semantics, session history and validated Loro state.
use crate::descriptor::{Node, descriptor};
use crate::execute::{execute, fill};
use crate::project::project;
use crate::publication::{self, Events};
use crate::{
    Batch, Code, Error, Intent, Publication, Reading, Result, STORAGE_BYTES, Segment, check, encode, engine, err, json,
    lock, parse, text, theme, wire,
};
use loro::{Container, ExportMode, Frontiers, LoroDoc, ValueOrContainer};
use serde_json::Value;
#[cfg(feature = "storage")]
use std::collections::HashSet;
mod history;
#[cfg(all(feature = "storage", not(target_arch = "wasm32")))]
mod template;
pub(crate) mod version;
use history::Run;
use version::version_token;

fn raw(doc: &LoroDoc) -> Value {
    json(doc.get_map("data").get_deep_value())
}
fn subscribe(doc: &LoroDoc, events: &Events) {
    // The subscription lives exactly as long as this LoroDoc; a replaced doc drops it.
    publication::subscribe(doc, events).detach();
}

/// `doc` with the layout marker and `value`, a validated value of `schema`, committed.
fn filled(doc: LoroDoc, schema: &Node, value: &Value) -> Result<LoroDoc> {
    doc.set_next_commit_message("create");
    doc.get_map(META).insert("layout", LAYOUT).map_err(engine)?;
    fill(&doc.get_map("data"), schema, value)?;
    doc.commit();
    Ok(doc)
}
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
    pub(crate) schema: Node,
    pub(crate) theme: theme::Theme,
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
    /// The declared colors, in the order the author wrote them.
    pub fn theme_tokens(&self) -> &[(String, String)] {
        self.theme.tokens()
    }
}
/// The page's text edit, a batch whose set carries `selection`: the selection in UTF-16
/// offsets of the merged text.
#[derive(Debug)]
pub struct TextEdit {
    pub selection: [usize; 2],
}

/// Exactly one host executor owns this value. Neither binding contains semantics.
pub struct Document {
    pub(crate) doc: LoroDoc,
    app: AppSpec,
    sequence: u64,
    /// Every row list's rows as of the last publication.
    lists: publication::Lists,
    events: Events,
    /// Loro's undo of this session's own changes.
    undo: loro::UndoManager,
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
            acceptable(&doc, &app)?;
        }
        doc.set_record_timestamp(true);
        let events = Events::default();
        subscribe(&doc, &events);
        let this = Self {
            lists: publication::index_all(&doc, &app.schema),
            undo: history::manager(&doc),
            run: None,
            broken: false,
            doc,
            app,
            sequence: 0,
            events,
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
        // A saved history starts with a snapshot, which alone may start it late; the updates
        // after it never move that start.
        let first =
            LoroDoc::decode_import_blob_meta(checkpoint, false).map_err(|e| core(err(Code::InvalidBytes, e)))?;
        if !matches!(first.mode, loro::EncodedBlobMode::Snapshot | loro::EncodedBlobMode::ShallowSnapshot) {
            return Err(core(err(Code::InvalidBytes, "The saved history has no snapshot; keep it for recovery")));
        }
        let doc = LoroDoc::new();
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
    pub(crate) fn projected(&self) -> Value {
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
    /// Replaces the Loro document with one replayed to `before`, under the same peer: one
    /// session writes as one peer. The refused operations were never committed to a save
    /// or sent anywhere, so the peer may reuse their counters.
    fn rebuild_at(&mut self, before: &loro::Frontiers) -> Result<()> {
        let fresh = replica_at(&self.doc, before)
            .and_then(|fresh| fresh.set_peer_id(self.doc.peer_id()).map(|()| fresh).map_err(engine))
            .inspect_err(|_| self.broken = true)?;
        lock(&self.events).clear();
        subscribe(&fresh, &self.events);
        // The undo manager belongs to the replaced document: this session's undo starts
        // over. Rehearsal (`apply`) keeps refusals from reaching here.
        self.undo = history::manager(&fresh);
        self.run = None;
        self.doc = fresh;
        // Publication may have failed after updating indexes; rebuild those too.
        self.lists = publication::index_all(&self.doc, &self.app.schema);
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
        if let Some(expected) = &batch.if_version
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
        // A batch that could fail after its first change runs on a copy of the current state
        // first. A refusal then leaves the live document, its peer and its undo untouched.
        // A single intent other than `replace` checks everything before it changes anything.
        if batch.intents.len() > 1 || batch.intents.iter().any(|op| matches!(op, Intent::Replace { .. })) {
            let copy = LoroDoc::new();
            copy.import(&self.doc.export(ExportMode::state_only(None)).map_err(engine)?).map_err(engine)?;
            self.run_intents(&copy, &batch, origin)?;
        }
        let before = self.doc.state_frontiers();
        let message = command.map_or_else(|| origin.message().to_owned(), |name| format!("command:{name}"));
        self.doc.set_next_commit_message(&message);
        let (ids, typed) = match self.run_intents(&self.doc, &batch, origin) {
            Ok(done) => done,
            Err(error) => {
                self.abort(&before)?;
                return Err(error);
            }
        };
        // An edit (not a no-op) joins or starts its undo step before it commits.
        if self.doc.get_pending_txn_len() > 0 {
            let (run, continues) = match &typed {
                _ if command.is_some() => (None, false),
                // A merged edit ends the typing run and is its own undo step.
                Some(typed) if origin != Origin::Agent && typed.merged => (None, false),
                Some(typed) if origin != Origin::Agent => (
                    Some(Run::Typing { path: typed.path.clone(), text: typed.to.clone(), caret: typed.caret }),
                    self.continues_typing(&typed.path, &typed.from, &typed.to),
                ),
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
                    (run, continues)
                }
            };
            self.group(run, continues);
        }
        self.doc.commit();
        let published = self.publish_or_abort(&before)?;
        let text = typed.map(|typed| TextEdit { selection: typed.selection });
        Ok(Self::applied(self.sequence, ids, published, text))
    }
    /// Runs a batch's intents on `doc` (the live document, or a rehearsal copy of its
    /// state): the inserted row IDs and the page's text edit, or the first refusal.
    fn run_intents(&self, doc: &LoroDoc, batch: &Batch, origin: Origin) -> Result<(Vec<String>, Option<text::Typed>)> {
        let (mut ids, mut typed) = (vec![], None);
        for (index, op) in batch.intents.iter().enumerate() {
            let result = if origin == Origin::Page && matches!(op, Intent::SetTheme { .. } | Intent::ImportTheme { .. })
            {
                Err(err(Code::InvalidRequest, "The page cannot change the palette"))
            } else {
                execute(doc, &self.app, op, &mut ids, &mut typed)
            };
            result.map_err(|mut e| {
                e.op_index = Some(index);
                e
            })?;
        }
        Ok((ids, typed))
    }
    fn applied(sequence: u64, ids: Vec<String>, published: Option<Published>, text: Option<TextEdit>) -> Applied {
        let theme_changed = published.as_ref().is_some_and(|p| p.theme);
        Applied { sequence, ids, publication: published.map(|p| p.json), theme_changed, text }
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

    /// Every operation this replica holds, by peer.
    pub(crate) fn version_vector(&self) -> loro::VersionVector {
        self.doc.oplog_vv()
    }
    /// The operations this replica holds past `version`, as one Loro update.
    pub(crate) fn updates_since(&self, version: &loro::VersionVector) -> Result<Vec<u8>> {
        self.doc.export(ExportMode::updates(version)).map_err(engine)
    }
    /// Changes other replicas made, as Loro updates from a relay: imported, checked
    /// against the app as a saved document is, and published like an edit. An update
    /// whose dependencies have not arrived waits inside Loro until they do. A change the
    /// app cannot accept is refused and leaves the document as it was, though this
    /// session's undo starts over. `None` when the updates held nothing new.
    pub fn import_remote(&mut self, updates: &[&[u8]]) -> Result<Option<Applied>> {
        self.intact()?;
        let before = self.doc.state_frontiers();
        let version = self.doc.oplog_vv();
        let imported = updates
            .iter()
            .try_for_each(|bytes| self.doc.import(bytes).map(drop))
            .map_err(|e| err(Code::InvalidBytes, e))
            .and_then(|()| check_layout(&self.doc))
            .and_then(|()| acceptable(&self.doc, &self.app));
        if let Err(error) = imported {
            self.abort(&before)?;
            return Err(error);
        }
        if self.doc.oplog_vv() == version {
            return Ok(None);
        }
        // Published even when nothing visible changed, so the page's version moves on.
        let published = match self.publish_with(true) {
            Ok(published) => published,
            Err(error) => {
                self.abort(&before)?;
                return Err(error);
            }
        };
        Ok(Some(Self::applied(self.sequence, vec![], published, None)))
    }
}
/// Whether `doc` holds what `app` accepts: checked whenever state arrives from outside
/// this session, a saved file or another replica.
fn acceptable(doc: &LoroDoc, app: &AppSpec) -> Result<()> {
    check::stored(&app.schema, Some(ValueOrContainer::Container(Container::Map(doc.get_map("data")))))?;
    app.theme.check_stored(&doc.get_map(theme::ROOT))
}
/// A new long-lived replica holding exactly the history up to `frontiers`. Built by
/// replaying the operations from where `doc`'s history starts (a trimmed
/// document's starting state, else nothing), because Loro does not implement
/// `LoroDoc::fork_at` for trimmed documents.
fn replica_at(doc: &LoroDoc, frontiers: &Frontiers) -> Result<LoroDoc> {
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
