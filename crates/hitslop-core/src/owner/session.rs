//! What sharing changes about a document's owner, as hooks the owner calls at fixed
//! points. A local document's session, this one, changes nothing. The opt-in `dev-sync`
//! feature replaces this module with `sync.rs`, whose session forwards edits to an
//! authority or serves as one; the owner itself has no other sharing code.
use super::*;

pub struct State;
impl State {
    pub(super) fn new(_: &mpsc::Sender<super::Message>, _: &store::Store) -> Result<Self> {
        Ok(Self)
    }
}
/// A local document receives no session messages.
pub enum Message {}
/// A local command carries no session context.
pub(super) enum Context {}

impl Actor {
    pub(super) fn session_message(&mut self, message: Message) {
        match message {}
    }
    /// Before a request is admitted: it may be refused, held or handed on instead.
    pub(super) fn session_admit(
        &mut self,
        request: Request,
        _: Option<String>,
        _: Option<Duration>,
        _: &mut Option<Completion>,
    ) -> Result<Option<Request>> {
        Ok(Some(request))
    }
    /// A request's reply, which a session may answer later instead.
    pub(super) fn session_reply(&mut self, reply: Reply, _: &mut Option<Completion>) -> Result<Option<Reply>> {
        Ok(Some(reply))
    }
    /// The context a command being admitted carries to its answer.
    pub(super) fn session_command(&mut self) -> Option<Context> {
        None
    }
    /// Answers a command's caller.
    pub(super) fn command_answered(&mut self, invocation: &mut commands::Invocation, result: Result<Reply>) {
        if let Some(context) = invocation.session.take() {
            match context {}
        }
        complete(invocation.take_callback(), result);
    }
    /// A change was accepted and published.
    pub(super) fn session_accepted(&mut self) {}
    /// A save succeeded; installs what the session waited for it to write.
    pub(super) fn session_installed(&mut self) -> Result<()> {
        Ok(())
    }
    /// The saved version advanced.
    pub(super) fn session_saved(&mut self) {}
    /// A save the session writes instead of the next ordinary one.
    pub(super) fn session_job(&self) -> Option<Result<store::SaveJob>> {
        None
    }
    /// Whether the owner bounds its own history (`maintenance`).
    pub(super) fn session_rebuilds(&self) -> bool {
        let State = &self.session;
        true
    }
    /// Whether Edit ▸ Undo is available.
    pub(super) fn session_undo(&self) -> bool {
        true
    }
    pub(super) fn session_shutdown(&mut self) {}
}
