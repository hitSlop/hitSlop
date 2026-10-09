//! Nonblocking driver for the same owner used by native hosts. The dedicated browser
//! worker drives time and serial disk work; a separate disposable worker evaluates commands.
use super::*;

pub struct BrowserDriver {
    owner: Owner,
    actor: Actor,
    messages: mpsc::Receiver<Message>,
    work: mpsc::Receiver<Work>,
    evaluations: mpsc::Receiver<EvaluationWork>,
    pending: Option<EvaluationWork>,
    stopped: bool,
}

impl BrowserDriver {
    /// Caller owns the copy's Web Lock and has installed its dedicated VFS.
    pub fn open(name: &str, vfs: &str, imported: bool, listener: Listener) -> Result<Self> {
        let store = Arc::new(store::Store::open_vfs(name, vfs, imported)?);
        let (sender, messages) = mpsc::channel();
        let (persist, work) = mpsc::channel();
        let (evaluate, evaluations) = mpsc::channel();
        let session = session::State::new(&sender, &store)?;
        let mode = store::Mode::Document;
        let actor = Actor::new(store.clone(), mode, listener, persist, Some(evaluate), session)?;
        actor.start();
        Ok(Self {
            owner: Owner { mode, sender, store, path: PathBuf::from(name) },
            actor,
            messages,
            work,
            evaluations,
            pending: None,
            stopped: false,
        })
    }
    /// Drive the shared save barrier before a synchronous export. No authored work runs here.
    pub fn flush(&mut self) -> Result<()> {
        let (send, receive) = mpsc::channel();
        self.owner.submit(
            Request::Flush,
            None,
            Box::new(move |reply| {
                let _ = send.send(reply);
            }),
        );
        self.poll(self.actor.now.as_millis() as u64, self.actor.unix_ms);
        receive
            .try_recv()
            .map_err(|_| Failure::new(FailureKind::Busy, "The document is still busy; retry export"))??;
        Ok(())
    }
    pub fn owner(&self) -> &Owner {
        &self.owner
    }
    pub fn store(&self) -> &store::Store {
        &self.owner.store
    }

    /// Returns the delay before the next timer. All completions return through the same
    /// queue, including serial persistence; accepted edits and confirmed saves stay distinct.
    pub fn poll(&mut self, monotonic_ms: u64, unix_ms: u64) -> Option<u64> {
        if self.stopped {
            return None;
        }
        self.actor.clock(Duration::from_millis(monotonic_ms), unix_ms);
        loop {
            self.actor.tick();
            if let Ok(message) = self.messages.try_recv() {
                if !self.actor.step(message) {
                    self.stopped = true;
                    self.actor.shutdown();
                    return None;
                }
                continue;
            }
            if let Ok(work) = self.work.try_recv() {
                let done = perform(&self.owner.store, work);
                let _ = self.owner.sender.send(done);
                continue;
            }
            break;
        }
        self.actor.wake().map(|wake| wake.saturating_sub(self.actor.now).as_millis() as u64)
    }

    /// One bounded envelope. Its invocation remains in Rust until this worker's result
    /// returns; a page never chooses the version, origin, clock, or random seed.
    pub fn take_evaluation(&mut self) -> Option<String> {
        if self.pending.is_some() {
            return None;
        }
        let work = self.evaluations.try_recv().ok()?;
        match work.input() {
            Ok(input) => {
                self.pending = Some(work);
                Some(input)
            }
            Err(error) => {
                let _ = self.owner.sender.send(work.finish(Err(error)));
                None
            }
        }
    }
    pub fn evaluated(&mut self, output: std::result::Result<String, String>) -> Result<()> {
        let work =
            self.pending.take().ok_or_else(|| Failure::rejected(Code::InvalidRequest, "No command is running"))?;
        self.owner.sender.send(work.finish(output)).map_err(|_| closed())
    }
}

impl Drop for BrowserDriver {
    fn drop(&mut self) {
        if !self.stopped {
            self.actor.shutdown();
        }
    }
}
