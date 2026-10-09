//! Session undo, redo and edit grouping.
use super::*;

/// The undo step being extended: a typing run in one text field (its text and caret, in
/// UTF-16, after the last edit), a run of agent batches, or a run of the window's changes
/// to one palette color (a color panel drag).
#[derive(Clone, PartialEq)]
pub(super) enum Run {
    Typing { path: Vec<Segment>, text: String, caret: usize },
    Agent,
    Color(String),
}
/// One document edit, restored by Loro as a new change. Only version references are
/// kept here; document values and their history remain in Loro.
#[derive(Clone)]
pub(super) struct Step {
    pub(super) before: Frontiers,
    pub(super) after: Frontiers,
}
/// Undo covers the open session only: a document opens with nothing to undo.
const UNDO_STEPS: usize = 100;

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
    pub(super) fn record(&mut self, before: Frontiers, run: Option<Run>, continues: bool) {
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
    pub(super) fn record_typing(&mut self, before: Frontiers, path: &[Segment], from: &str, to: &str, caret: usize) {
        let continues = matches!(&self.run, Some(Run::Typing { path: p, text, caret: at }) if p == path && text == from && {
            let (before, after): (Vec<u16>, Vec<u16>) = (from.encode_utf16().collect(), to.encode_utf16().collect());
            let prefix = before.iter().zip(&after).take_while(|(a, b)| a == b).count();
            let suffix = before[prefix..].iter().rev().zip(after[prefix..].iter().rev()).take_while(|(a, b)| a == b).count();
            (prefix..=before.len() - suffix).contains(at)
        });
        self.record(before, Some(Run::Typing { path: path.to_vec(), text: to.to_owned(), caret }), continues);
    }
}
