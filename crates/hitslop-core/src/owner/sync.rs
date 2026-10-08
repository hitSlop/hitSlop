//! The shared session of the opt-in `dev-sync` proof, in place of `session.rs`: an owner
//! is local, an authority that accepts forwarded edits, or a replica that forwards its
//! edits and installs what the authority accepted (`crate::replication`). Callbacks
//! enqueue work; network I/O never runs on the owner. No request is replayed
//! automatically after an uncertain outcome.
use super::*;
use std::collections::VecDeque;

/// What a room checks before any byte is installed: the same document, app and layout.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Identity {
    pub document_uuid: String,
    pub app_digest: String,
    pub layout: i64,
}

pub struct Accepted {
    pub reply: Reply,
    pub updates: Vec<u8>,
}
pub struct Update {
    pub before: Vec<u8>,
    pub after: Vec<u8>,
    pub bytes: Vec<u8>,
}
pub type SyncCompletion = Box<dyn FnOnce(Result<Accepted>) + Send>;
pub type Forwarder = Arc<dyn Fn(Request, Vec<u8>, SyncCompletion) + Send + Sync>;
pub type UpdateListener = Arc<dyn Fn(Update) + Send + Sync>;
pub type ExportCompletion = Box<dyn FnOnce(Result<(Identity, Vec<u8>, Vec<u8>)>) + Send>;
pub struct Context {
    base: Vec<u8>,
    callback: SyncCompletion,
}
enum Role {
    Local,
    Authority { listener: UpdateListener, version: Vec<u8> },
    Replica { forwarder: Option<mpsc::SyncSender<(Request, Vec<u8>, SyncCompletion)>> },
}
pub struct State {
    role: Role,
    identity: Option<Identity>,
    sender: mpsc::Sender<super::Message>,
    context: Option<Context>,
    pending: usize,
    uncertain: bool,
    snapshot: Option<Document>,
    exports: Vec<ExportCompletion>,
    broadcasts: VecDeque<(u64, Update)>,
}
impl State {
    pub(super) fn new(sender: &mpsc::Sender<super::Message>, store: &store::Store) -> Result<Self> {
        let identity = match &store.app().document_uuid {
            Some(uuid) => {
                Some(Identity { document_uuid: uuid.clone(), app_digest: store.app_digest()?, layout: crate::LAYOUT })
            }
            None => None,
        };
        let sender = sender.clone();
        Ok(Self {
            role: Role::Local,
            identity,
            sender,
            context: None,
            pending: 0,
            uncertain: false,
            snapshot: None,
            exports: vec![],
            broadcasts: VecDeque::new(),
        })
    }
    pub fn shared(&self) -> bool {
        !matches!(self.role, Role::Local)
    }
}
pub enum Message {
    Authority(UpdateListener, Completion),
    Replica(Identity, Forwarder, Completion),
    Submit(Request, Context),
    Install(Identity, Vec<u8>, Completion),
    Snapshot(Identity, Vec<u8>, Completion),
    Export(ExportCompletion),
    Check(Vec<u8>, Completion),
    Disconnect(Completion),
    Forwarded(Result<Accepted>, Completion),
    Settled(Result<Reply>, Completion),
}
impl Owner {
    fn sync_send(&self, message: Message) {
        if let Err(mpsc::SendError(super::Message::Session(message))) =
            self.sender.send(super::Message::Session(message))
        {
            message.reject(closed());
        }
    }
    pub fn sync_authority(&self, listener: UpdateListener, callback: Completion) {
        self.sync_send(Message::Authority(listener, callback));
    }
    pub fn sync_replica(&self, identity: Identity, forwarder: Forwarder, callback: Completion) {
        self.sync_send(Message::Replica(identity, forwarder, callback));
    }
    pub fn sync_submit(&self, request: Request, base: Vec<u8>, callback: SyncCompletion) {
        self.sync_send(Message::Submit(request, Context { base, callback }));
    }
    pub fn sync_install(&self, identity: Identity, bytes: Vec<u8>, callback: Completion) {
        self.sync_send(Message::Install(identity, bytes, callback));
    }
    pub fn sync_snapshot(&self, identity: Identity, bytes: Vec<u8>, callback: Completion) {
        self.sync_send(Message::Snapshot(identity, bytes, callback));
    }
    pub fn sync_export(&self, callback: ExportCompletion) {
        self.sync_send(Message::Export(callback));
    }
    pub fn sync_check_base(&self, base: Vec<u8>, callback: Completion) {
        self.sync_send(Message::Check(base, callback));
    }
    pub fn sync_disconnect(&self, callback: Completion) {
        self.sync_send(Message::Disconnect(callback));
    }
}
impl Message {
    fn reject(self, error: Failure) {
        match self {
            Self::Authority(_, callback)
            | Self::Replica(_, _, callback)
            | Self::Install(_, _, callback)
            | Self::Snapshot(_, _, callback)
            | Self::Check(_, callback)
            | Self::Disconnect(callback)
            | Self::Forwarded(_, callback)
            | Self::Settled(_, callback) => complete(callback, Err(error)),
            Self::Export(callback) => deliver(callback, Err(error)),
            Self::Submit(_, context) => deliver(context.callback, Err(error)),
        }
    }
}
/// Calls a session callback as `complete` calls a request's: its panic cannot reach the
/// owner.
fn deliver<T>(callback: Box<dyn FnOnce(T) + Send>, value: T) {
    let _ = catch_unwind(AssertUnwindSafe(|| callback(value)));
}
fn refused(message: &str) -> Failure {
    Failure::rejected(Code::InvalidRequest, message)
}
fn mutation(request: &Request) -> bool {
    matches!(request, Request::Apply { .. } | Request::Command { .. })
}
fn barrier(request: &Request) -> bool {
    matches!(
        request,
        Request::Flush
            | Request::Copy { .. }
            | Request::CaptureSource { .. }
            | Request::Close { .. }
            | Request::ExportTheme
            | Request::Backup { .. }
    )
}
impl Actor {
    fn check_identity(&self, identity: &Identity) -> Result<()> {
        if self.session.identity.as_ref() != Some(identity) {
            return Err(refused("Shared document identity, app or layout differs"));
        }
        Ok(())
    }
    pub(super) fn session_admit(
        &mut self,
        request: Request,
        view: Option<String>,
        deadline: Option<Instant>,
        callback: &mut Option<Completion>,
    ) -> Result<Option<Request>> {
        if !self.session.shared() {
            return Ok(Some(request));
        }
        if matches!(request, Request::Undo { .. } | Request::PutAttachment { .. } | Request::Discard) {
            return Err(refused("Shared undo, attachment imports and discard are unavailable in the loopback proof"));
        }
        if self.session.snapshot.is_some() && mutation(&request) {
            return Err(Failure::new(FailureKind::Busy, "Installing a shared checkpoint; retry after it settles"));
        }
        if mutation(&request) {
            self.mutation()?;
            if self.session.broadcasts.len() >= 128 {
                return Err(Failure::new(FailureKind::Busy, "Shared durability queue is full; retry saving"));
            }
            if self.session.uncertain {
                return Err(Failure::new(FailureKind::Failed, "A shared outcome is unknown; reconnect before editing"));
            }
            if let Role::Replica { forwarder, .. } = &self.session.role {
                let forwarder = forwarder.clone().ok_or_else(|| {
                    Failure::new(FailureKind::Busy, "Shared document is disconnected; keep your draft")
                })?;
                if self.session.pending >= 128 {
                    return Err(Failure::new(FailureKind::Busy, "Shared request queue is full"));
                }
                let base = self.core.version_vector();
                let sender = self.session.sender.clone();
                let callback = take(callback);
                self.session.pending += 1;
                let completion: SyncCompletion = Box::new(move |result| {
                    if let Err(mpsc::SendError(super::Message::Session(message))) =
                        sender.send(super::Message::Session(Message::Forwarded(result, callback)))
                    {
                        message.reject(closed());
                    }
                });
                if let Err(error) = forwarder.try_send((request, base, completion)) {
                    let (_, _, completion) = match error {
                        mpsc::TrySendError::Full(work) | mpsc::TrySendError::Disconnected(work) => work,
                    };
                    completion(Err(Failure::rejected(
                        Code::InvalidRequest,
                        "Shared forwarding queue unavailable; request was not sent",
                    )));
                }
                return Ok(None);
            }
        }
        if barrier(&request) {
            if self.session.uncertain {
                return Err(Failure::new(
                    FailureKind::Failed,
                    "A shared outcome is unknown; reconnect before exporting or closing",
                ));
            }
            if self.session.pending > 0 || self.evaluating {
                self.hold(request, view, deadline, callback)?;
                return Ok(None);
            }
        }
        Ok(Some(request))
    }
    pub(super) fn sync_finish(&mut self, result: Result<Reply>, context: Context) {
        let accepted =
            result.and_then(|reply| Ok(Accepted { reply, updates: self.core.updates_since(&context.base)? }));
        match accepted {
            Err(error) => deliver(context.callback, Err(error)),
            Ok(accepted) => {
                let mut callback: Option<Completion> =
                    Some(Box::new(move |result| deliver(context.callback, result.map(|_| accepted))));
                if let Err(error) = self.wait(&mut callback, AfterSave::Reply(Reply::Unit))
                    && let Some(callback) = callback
                {
                    complete(callback, Err(error));
                }
            }
        }
        self.sync_drain();
    }
    pub(super) fn session_accepted(&mut self) {
        let Role::Authority { version, .. } = &mut self.session.role else {
            return;
        };
        let before = version.clone();
        let after = self.core.version_vector();
        let bytes = match self.core.updates_since(&before) {
            Ok(bytes) => bytes,
            Err(error) => {
                self.invalidated = true;
                self.fail(error.into(), u64::MAX);
                return;
            }
        };
        *version = after.clone();
        self.session.broadcasts.push_back((self.revision, Update { before, after, bytes }));
    }
    pub(super) fn sync_release_broadcasts(&mut self) {
        let Role::Authority { listener, .. } = &self.session.role else {
            return;
        };
        while self.session.broadcasts.front().is_some_and(|(revision, _)| *revision <= self.saved) {
            let (_, update) = self.session.broadcasts.pop_front().expect("front exists");
            let _ = catch_unwind(AssertUnwindSafe(|| listener(update)));
        }
    }
    fn sync_wait(&mut self, callback: Completion, next: AfterSave) {
        let mut callback = Some(callback);
        if let Err(error) = self.wait(&mut callback, next)
            && let Some(callback) = callback
        {
            complete(callback, Err(error));
        }
    }

    fn sync_install_update(&mut self, identity: &Identity, bytes: &[u8]) -> Result<()> {
        self.admit(false, None)?;
        self.mutation()?;
        if !matches!(self.session.role, Role::Replica { .. }) {
            return Err(refused("Only a replica installs accepted updates"));
        }
        self.check_identity(identity)?;
        if self.session.snapshot.is_some() {
            return Err(Failure::new(FailureKind::Busy, "A shared checkpoint is being installed"));
        }
        let applied = self.edited(|core| core.import_accepted(bytes))?;
        self.accepted(applied.sequence, applied.publication, applied.theme_changed);
        Ok(())
    }
    pub(super) fn session_message(&mut self, message: Message) {
        if self.lifecycle == Lifecycle::Closed {
            message.reject(closed());
            return;
        }
        match message {
            Message::Authority(listener, callback) => {
                let result = self.mutation().and_then(|()| {
                    if self.maintenance_running() {
                        return Err(Failure::new(
                            FailureKind::Busy,
                            "Finish local history maintenance before starting a shared session",
                        ));
                    }
                    if self.session.shared() {
                        return Err(refused("Shared role already configured"));
                    }
                    if self.session.identity.is_none() {
                        return Err(refused("A room requires a document"));
                    }
                    self.session.role = Role::Authority { listener, version: self.core.version_vector() };
                    self.refresh_undo();
                    Ok(Reply::Unit)
                });
                complete(callback, result);
            }
            Message::Replica(identity, forwarder, callback) => {
                let result = self.mutation().and_then(|()| {
                    if self.maintenance_running() {
                        return Err(Failure::new(
                            FailureKind::Busy,
                            "Finish local history maintenance before starting a shared session",
                        ));
                    }
                    self.check_identity(&identity)?;
                    if matches!(self.session.role, Role::Authority { .. })
                        || self.session.pending != 0
                        || self.evaluating
                    {
                        return Err(refused("Cannot replace an active shared role"));
                    }
                    let (send, receive) = mpsc::sync_channel::<(Request, Vec<u8>, SyncCompletion)>(128);
                    std::thread::Builder::new()
                        .name("hitslop.sync-forward".into())
                        .spawn(move || {
                            for (request, base, callback) in receive {
                                forwarder(request, base, callback);
                            }
                        })
                        .map_err(|error| Failure::new(FailureKind::Failed, error.to_string()))?;
                    self.session.role = Role::Replica { forwarder: Some(send) };
                    self.refresh_undo();
                    Ok(Reply::Unit)
                });
                complete(callback, result);
            }
            Message::Submit(request, context) => {
                if !matches!(self.session.role, Role::Authority { .. }) || !mutation(&request) {
                    deliver(context.callback, Err(refused("Only an authority accepts forwarded edits")));
                    return;
                }
                // A divergent base is a definite refusal before applying any intent.
                if let Err(error) = self.core.updates_since(&context.base) {
                    deliver(context.callback, Err(error.into()));
                    return;
                }
                self.session.context = Some(context);
                let mut callback: Option<Completion> = Some(Box::new(|_| {}));
                let result = self.request(request, None, None, &mut callback);
                if let Some(context) = self.session.context.take() {
                    self.sync_finish(result.map(|r| r.unwrap_or(Reply::Unit)), context);
                }
            }
            Message::Install(identity, bytes, callback) => {
                let result = self.sync_install_update(&identity, &bytes);
                match result {
                    Err(error) => complete(callback, Err(error)),
                    Ok(()) => self.sync_wait(callback, AfterSave::Reply(Reply::Unit)),
                }
            }
            Message::Snapshot(identity, bytes, callback) => {
                let result = (|| {
                    self.admit(false, None)?;
                    self.mutation()?;
                    self.check_identity(&identity)?;
                    if !matches!(self.session.role, Role::Replica { .. })
                        || self.session.pending > 0
                        || self.writing
                        || self.session.snapshot.is_some()
                        || self.revision != self.saved
                    {
                        return Err(Failure::new(
                            FailureKind::Busy,
                            "Replica must be drained before installing a checkpoint",
                        ));
                    }
                    let candidate = self.core.snapshot_candidate(&bytes)?;
                    self.session.snapshot = Some(candidate);
                    self.revision += 1;
                    self.status(SaveStatus::Saving);
                    Ok(())
                })();
                match result {
                    Err(error) => complete(callback, Err(error)),
                    Ok(()) => self.sync_wait(callback, AfterSave::Reply(Reply::Unit)),
                }
            }
            Message::Export(callback) => {
                if self.session.pending > 0 || self.evaluating || self.session.snapshot.is_some() {
                    if self.session.exports.len() >= 32 {
                        deliver(callback, Err(Failure::new(FailureKind::Busy, "Shared export queue is full")));
                    } else {
                        self.session.exports.push(callback);
                    }
                    return;
                }
                let export = (|| {
                    Ok((
                        self.session.identity.clone().ok_or_else(|| refused("A room requires a document"))?,
                        self.core.transfer_snapshot()?,
                        self.core.version_vector(),
                    ))
                })();
                match export {
                    Err(error) => deliver(callback, Err(error)),
                    Ok(export) => {
                        self.sync_wait(
                            Box::new(move |result| deliver(callback, result.map(|_| export))),
                            AfterSave::Reply(Reply::Unit),
                        );
                    }
                }
            }
            Message::Check(base, callback) => {
                let result = self
                    .core
                    .updates_since(&base)
                    .map(|_| Reply::Unit)
                    .or_else(|error| if error.code == Code::MissingDependencies { Ok(Reply::Unit) } else { Err(error) })
                    .map_err(Failure::from);
                complete(callback, result);
            }
            Message::Disconnect(callback) => {
                if let Role::Replica { forwarder, .. } = &mut self.session.role {
                    *forwarder = None;
                }
                complete(callback, Ok(Reply::Unit));
            }
            Message::Forwarded(result, callback) => {
                let result = result.and_then(|accepted| {
                    let install = (|| {
                        let identity = match &self.session.role {
                            Role::Replica { .. } => {
                                self.session.identity.clone().ok_or_else(|| refused("A room requires a document"))?
                            }
                            _ => return Err(refused("Replica role was replaced")),
                        };
                        self.sync_install_update(&identity, &accepted.updates)?;
                        Ok(match accepted.reply {
                            Reply::Applied { ids, text, .. } => Reply::Applied { sequence: self.sequence, ids, text },
                            Reply::Command { ids, result_json, .. } => {
                                Reply::Command { sequence: self.sequence, ids, result_json }
                            }
                            _ => return Err(refused("Authority returned an invalid edit outcome")),
                        })
                    })();
                    install.map_err(|error: Failure| Failure {
                        kind: FailureKind::Failed,
                        message: format!(
                            "The authority accepted the edit, but this replica could not install its outcome: {}",
                            error.message
                        ),
                        reason: error.reason,
                        op_index: error.op_index,
                    })
                });
                match result {
                    Err(error) => self.sync_settled(Err(error), callback, true),
                    Ok(reply) => {
                        let sender = self.session.sender.clone();
                        let completion: Completion = Box::new(move |result| {
                            if let Err(mpsc::SendError(super::Message::Session(message))) =
                                sender.send(super::Message::Session(Message::Settled(result, callback)))
                            {
                                message.reject(closed());
                            }
                        });
                        self.sync_wait(completion, AfterSave::Reply(reply));
                    }
                }
            }
            Message::Settled(result, callback) => self.sync_settled(result, callback, false),
        }
    }
    fn sync_settled(&mut self, result: Result<Reply>, callback: Completion, remote_failure: bool) {
        self.session.pending = self.session.pending.saturating_sub(1);
        if remote_failure && result.as_ref().is_err_and(|error| error.kind != FailureKind::Rejected) {
            self.session.uncertain = true;
        }
        complete(callback, result);
        self.sync_drain();
    }
    pub(super) fn sync_drain(&mut self) {
        if self.session.pending != 0 || self.evaluating {
            return;
        }
        for callback in std::mem::take(&mut self.session.exports) {
            self.session_message(Message::Export(callback));
        }
        if self.session.shared() {
            self.release_held();
        }
    }
    pub(super) fn session_shutdown(&mut self) {
        for callback in self.session.exports.drain(..) {
            deliver(callback, Err(closed()));
        }
        if let Some(context) = self.session.context.take() {
            deliver(context.callback, Err(closed()));
        }
    }
    pub(super) fn session_installed(&mut self) -> Result<()> {
        if let Some(candidate) = self.session.snapshot.take() {
            let publication = candidate.reset_publication(self.sequence)?;
            self.sequence = candidate.sequence();
            self.core = candidate;
            self.session.uncertain = false;
            self.emit(Event::Publication { json: publication });
            self.emit(Event::ThemeChanged);
            self.refresh_undo();
        }
        Ok(())
    }

    pub(super) fn session_reply(&mut self, reply: Reply, callback: &mut Option<Completion>) -> Result<Option<Reply>> {
        if let Some(context) = self.session.context.take() {
            self.sync_finish(Ok(reply), context);
            callback.take();
            return Ok(None);
        }
        // A shared edit is answered once it is durable, as its broadcast is.
        if self.session.shared() && matches!(reply, Reply::Applied { .. }) {
            self.wait(callback, AfterSave::Reply(reply))?;
            return Ok(None);
        }
        Ok(Some(reply))
    }
    pub(super) fn session_command(&mut self) -> Option<Context> {
        self.session.context.take()
    }
    pub(super) fn command_answered(&mut self, invocation: &mut commands::Invocation, result: Result<Reply>) {
        if let Some(context) = invocation.session.take() {
            self.sync_finish(result, context);
            return;
        }
        match result {
            Ok(reply) if self.session.shared() => {
                let mut callback = Some(invocation.take_callback());
                if let Err(error) = self.wait(&mut callback, AfterSave::Reply(reply))
                    && let Some(callback) = callback
                {
                    complete(callback, Err(error));
                }
            }
            result => complete(invocation.take_callback(), result),
        }
        self.sync_drain();
    }
    pub(super) fn session_saved(&mut self) {
        self.sync_release_broadcasts();
        self.sync_drain();
    }
    pub(super) fn session_job(&self) -> Option<Result<store::SaveJob>> {
        let candidate = self.session.snapshot.as_ref()?;
        Some(self.store.replacement(candidate).map_err(Failure::from))
    }
    /// A shared document's retention belongs to its room.
    pub(super) fn session_rebuilds(&self) -> bool {
        !self.session.shared()
    }
    /// Undo of the whole document would revert other people's edits.
    pub(super) fn session_undo(&self) -> bool {
        !self.session.shared()
    }
}
