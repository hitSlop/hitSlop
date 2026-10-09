//! Rebuilding a live document whose retained history outgrew its budget: a new document
//! from a shallow checkpoint that keeps the supported undo window. Preparation never
//! changes the old document; the owner swaps the rebuilt one in only after its checkpoint
//! is written (`owner::maintenance`). No page sequence, writer peer or identity changes.
use super::*;

/// What the owner hands its persistence worker while it holds edits back. Loro's clone
/// is a reference clone; this type only reads it, never mutating or publishing, so reads
/// keep using the one live document on the owner while the worker prepares.
pub(crate) struct Seed {
    doc: LoroDoc,
    app: AppSpec,
    sequence: u64,
    floor: VersionVector,
    undo: VecDeque<Step>,
    redo: Vec<Step>,
    run: Option<Run>,
}

impl Document {
    pub(crate) fn maintenance_seed(&self) -> Result<Seed> {
        self.intact()?;
        let seed = Seed {
            doc: self.doc.clone(),
            app: self.app.clone(),
            sequence: self.sequence,
            floor: self.floor.clone(),
            undo: self.undo.clone(),
            redo: self.redo.clone(),
            run: self.run.clone(),
        };
        Ok(seed)
    }

    /// A rebuild's write failed, and it may have committed anyway: the file may now keep
    /// only the history `rebuilt` kept, so no edit may branch from before it.
    pub(crate) fn keep_floor_of(&mut self, rebuilt: &Document) {
        self.floor.merge(&rebuilt.floor);
    }

    #[cfg(test)]
    fn maintenance_candidate(&self, budget: usize) -> Result<Self> {
        Ok(self.maintenance_seed()?.build(budget)?.0)
    }
}

impl Seed {
    /// The rebuilt document and its checkpoint, keeping the supported undo/redo window
    /// when it fits in `budget` bytes. If it does not, redo expires first, then the oldest
    /// half of the undo steps, again and again: at most logarithmically many exports, and
    /// the common case keeps every step. Text bases older than what is kept become stale.
    pub(crate) fn build(mut self, budget: usize) -> Result<(Document, Vec<u8>)> {
        let source = self.doc.clone();
        let writer_peer = source.peer_id();
        // Fork on the worker before expensive historical exports. Its independent
        // locks keep later preparation from blocking reads of the live owner. This
        // fork is read-only; only the fully validated replacement can become a writer.
        self.doc = source.fork();
        let shallow = self.doc.shallow_since_vv().to_vv();
        let retained =
            |frontiers: &Frontiers| self.doc.frontiers_to_vv(frontiers).is_some_and(|vv| vv.includes_vv(&shallow));
        let valid_step = |step: &Step| retained(&step.before) && retained(&step.after);
        let mut undo = self.undo.clone();
        // Keep a suffix: dropping an undo in the middle would jump over it later.
        while undo.front().is_some_and(|step| !valid_step(step)) {
            undo.pop_front();
        }
        let mut redo = self.redo.clone();
        if redo.last().is_some_and(|step| !valid_step(step)) {
            redo.clear();
        }
        let bytes = loop {
            let mut oldest = self.doc.oplog_vv();
            // The local owner records steps after commits/imports, never checkout:
            // their operation histories only grow. Undo pushes those same steps onto
            // redo in reverse order. Thus front undo and back redo are each stack's
            // oldest required version; intersect both because neither stack's oldest
            // target needs to precede the other. Converting all 200 before/after
            // frontiers repeatedly makes shallow-history maintenance needlessly slow.
            for step in undo.front().into_iter().chain(redo.last()) {
                let version = self
                    .doc
                    .frontiers_to_vv(&step.before)
                    .ok_or_else(|| engine("Retained undo version is unavailable"))?;
                oldest = oldest.intersection(&version);
            }
            let root = self.doc.vv_to_frontiers(&oldest);
            let bytes = self.doc.export(ExportMode::shallow_snapshot(&root)).map_err(engine)?;
            if bytes.len() <= budget || (undo.is_empty() && redo.is_empty()) {
                break bytes;
            }
            if !redo.is_empty() {
                redo.clear();
            } else {
                let keep = undo.len() / 2;
                undo.drain(..undo.len() - keep);
            }
        };
        if bytes.len() > STORAGE_BYTES.saturating_sub(512) {
            return Err(err(Code::TooLarge, "Current document cannot fit a maintenance checkpoint"));
        }
        let mut candidate = Document::open(&self.app, &bytes, &[])?;
        candidate.doc.set_peer_id(writer_peer).map_err(engine)?;
        let current_theme = self.app.theme.effective(&source.get_map(theme::ROOT))?;
        // Full stored validation ran in open. Compare the live Loro values WITH all
        // container IDs, covering row/text handles, palette, and attachment references.
        if candidate.doc.oplog_vv() != source.oplog_vv()
            || candidate.doc.get_deep_value_with_id() != source.get_deep_value_with_id()
            || candidate.theme_state()?.effective != current_theme
        {
            return Err(err(Code::InvalidBytes, "Maintenance would change document state or identities"));
        }
        candidate.sequence = self.sequence;
        candidate.floor = self.floor.clone();
        candidate.floor.merge(&candidate.doc.shallow_since_vv().to_vv());
        candidate.run =
            if undo.len() == self.undo.len() && redo.len() == self.redo.len() { self.run.clone() } else { None };
        candidate.undo = undo;
        candidate.redo = redo;
        Ok((candidate, bytes))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn app() -> AppSpec {
        AppSpec::new(
            r#"{"kind":"object","properties":{"text":{"kind":"text"},"hits":{"kind":"counter"}}}"#,
            "maintenance",
            r##"{"accent":"#335577"}"##,
        )
        .unwrap()
    }
    fn edit(doc: &mut Document, intents: Value) {
        doc.apply_batch(Batch::decode(&json!({"intents":intents}).to_string()).unwrap(), Origin::Agent).unwrap();
        // These tests model separate user actions, not one extending agent undo run.
        doc.run = None;
    }
    #[test]
    fn rebuild_preserves_live_identity_peer_sequence_and_retained_undo_redo() {
        let mut doc = Document::create(&app(), r#"{"text":"Hello","hits":0}"#).unwrap();
        edit(&mut doc, json!([{"type":"set","path":["text"],"value":"Hello world"}]));
        let text_base = doc.version();
        edit(
            &mut doc,
            json!([{"type":"increment","path":["hits"],"by":5},{"type":"setTheme","values":{"accent":"#abcdef"}}]),
        );
        doc.undo().unwrap();
        let before = doc.state().unwrap();
        let mut candidate = doc.maintenance_candidate(16 * 1024 * 1024).unwrap();
        assert_eq!(candidate.state().unwrap(), before);
        assert_eq!(candidate.doc.peer_id(), doc.doc.peer_id());
        assert_eq!(candidate.doc.get_deep_value_with_id(), doc.doc.get_deep_value_with_id());
        assert!(candidate.can_undo() && candidate.can_redo());
        candidate.redo().unwrap();
        assert_eq!(serde_json::from_str::<Value>(&candidate.value()).unwrap()["hits"], 5);
        candidate.undo().unwrap();
        candidate.apply_batch(Batch::decode(&json!({"base":text_base,"intents":[{"type":"set","path":["text"],"from":"Hello world","value":"Hello world!"}]}).to_string()).unwrap(), Origin::Page).unwrap();
        assert_eq!(serde_json::from_str::<Value>(&candidate.value()).unwrap()["text"], "Hello world!");
        assert_eq!(doc.state().unwrap(), before, "preparing/using a candidate never alters the old owner");
    }
    #[test]
    fn oversized_history_expires_steps_and_bases_without_changing_current_state() {
        let mut doc = Document::create(&app(), r#"{"text":"Hello","hits":0}"#).unwrap();
        let old = doc.version();
        let mut random = 42u64;
        let text: String = (0..100_000)
            .map(|_| {
                random ^= random << 13;
                random ^= random >> 7;
                random ^= random << 17;
                char::from(b'a' + (random % 26) as u8)
            })
            .collect();
        edit(&mut doc, json!([{"type":"set","path":["text"],"value":text}]));
        edit(&mut doc, json!([{"type":"set","path":["text"],"value":"Hello"}]));
        let before = doc.state().unwrap();
        let mut candidate = doc.maintenance_candidate(2048).unwrap();
        assert_eq!(candidate.state().unwrap(), before);
        assert!(candidate.undo.len() < doc.undo.len(), "the oldest oversized window expires");
        assert!(!candidate.can_redo());
        let error = candidate
            .apply_batch(
                Batch::decode(
                    &json!({"base":old,"intents":[{"type":"set","path":["text"],"from":"Hello","value":"Hello!"}]})
                        .to_string(),
                )
                .unwrap(),
                Origin::Page,
            )
            .unwrap_err();
        assert_eq!(error.code, Code::StaleBase);
        assert!(candidate.checkpoint().unwrap().len() < doc.checkpoint().unwrap().len());
        assert!(doc.can_undo());
    }

    #[test]
    fn saved_text_floor_does_not_expire_undo_retained_in_the_live_history() {
        let mut doc = Document::create(&app(), r#"{"text":"Hello","hits":0}"#).unwrap();
        edit(&mut doc, json!([{"type":"increment","path":["hits"],"by":1}]));
        edit(&mut doc, json!([{"type":"increment","path":["hits"],"by":1}]));
        // Store may save a latest-root checkpoint while the old live engine still has
        // its undo window. Maintenance must use actual history, not this text gate.
        doc.retain_from(&doc.doc.oplog_frontiers());
        let mut candidate = doc.maintenance_candidate(16 * 1024 * 1024).unwrap();
        assert!(candidate.floor.includes_vv(&doc.floor));
        candidate.undo().unwrap();
        assert_eq!(serde_json::from_str::<Value>(&candidate.value()).unwrap()["hits"], 1);
        candidate.redo().unwrap();
        assert_eq!(serde_json::from_str::<Value>(&candidate.value()).unwrap()["hits"], 2);
    }
}
