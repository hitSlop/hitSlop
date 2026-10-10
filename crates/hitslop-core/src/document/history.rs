//! Session undo and redo through Loro's `UndoManager`, and edit grouping. Undo reverts the
//! session peer's own changes (the person's, the window's and an agent's alike) and keeps
//! everyone else's: it is rebased over changes that arrived since.
use super::*;
use loro::UndoManager;

/// The undo step being extended: a typing run in one text field (its text and caret, in
/// UTF-16, after the last edit), a run of agent batches, or a run of the window's changes
/// to one palette color (a color panel drag).
#[derive(Clone, PartialEq)]
pub(super) enum Run {
    Typing { path: Vec<Segment>, text: String, caret: usize },
    Agent,
    Color(String),
}
/// Undo covers the open session only: a document opens with nothing to undo.
const UNDO_STEPS: usize = 100;

/// The undo manager of `doc`, created once its saved state is imported, so nothing saved is
/// a step.
pub(super) fn manager(doc: &LoroDoc) -> UndoManager {
    let mut undo = UndoManager::new(doc);
    undo.set_max_undo_steps(UNDO_STEPS);
    undo
}

impl Document {
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
        self.undo.group_end();
        self.run = None;
        let before = self.doc.state_frontiers();
        let label = if undo { "undo" } else { "redo" };
        self.doc.set_next_commit_message(label);
        let done = if undo { self.undo.undo() } else { self.undo.redo() };
        let done = match done {
            Ok(done) => done,
            Err(error) => {
                self.abort(&before)?;
                return Err(engine(error));
            }
        };
        if !done {
            return Ok(Self::applied(self.sequence, vec![], None, None));
        }
        self.doc.set_next_commit_message(label);
        self.doc.commit();
        let published = self.publish_or_abort(&before)?;
        Ok(Self::applied(self.sequence, vec![], published, None))
    }
    /// Groups the pending edit before it commits: it joins the current step when it
    /// `continues` the run, else it starts a new step, a group of its own when it begins a
    /// run.
    pub(super) fn group(&mut self, run: Option<Run>, continues: bool) {
        if continues {
            // Loro closes a group when an import touches its containers; reopening it keeps
            // the run going in a new step after the conflict.
            let _ = self.undo.group_start();
        } else {
            self.undo.group_end();
            if run.is_some() {
                let _ = self.undo.group_start();
            }
        }
        self.run = run;
    }
    pub fn can_undo(&self) -> bool {
        self.undo.can_undo()
    }
    pub fn can_redo(&self) -> bool {
        self.undo.can_redo()
    }
    /// Whether an edit of `path` from `from` to `to` continues the typing run: the same
    /// field, unchanged since the last edit, changed at the caret that edit left.
    pub(super) fn continues_typing(&self, path: &[Segment], from: &str, to: &str) -> bool {
        matches!(&self.run, Some(Run::Typing { path: p, text, caret: at }) if p == path && text == from && {
            let (before, after): (Vec<u16>, Vec<u16>) = (from.encode_utf16().collect(), to.encode_utf16().collect());
            let prefix = before.iter().zip(&after).take_while(|(a, b)| a == b).count();
            let suffix = before[prefix..].iter().rev().zip(after[prefix..].iter().rev()).take_while(|(a, b)| a == b).count();
            (prefix..=before.len() - suffix).contains(at)
        })
    }
}
