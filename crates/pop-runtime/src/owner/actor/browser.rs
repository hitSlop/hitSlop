//! Nonblocking driver for the same owner used by native hosts. The dedicated browser
//! worker drives time and serial disk work. Browser pages submit edits, never commands.
use super::*;

pub struct BrowserDriver {
    owner: Owner,
    actor: Actor,
    messages: mpsc::Receiver<Message>,
    work: mpsc::Receiver<Work>,
    stopped: bool,
}

impl BrowserDriver {
    /// Caller owns the copy's Web Lock and has installed its dedicated VFS.
    pub fn open(name: &str, vfs: &str, imported: bool, listener: Listener) -> Result<Self> {
        let store = Arc::new(store::Store::open_vfs(name, vfs, imported)?);
        let (sender, messages) = mpsc::channel();
        let (persist, work) = mpsc::channel();
        let actor = Actor::new(store.clone(), store::Mode::Document, listener, persist)?;
        actor.start();
        Ok(Self {
            owner: Owner {
                sender,
                store,
                path: PathBuf::from(name),
                sync_admission: Arc::default(),
                admission: actor.admission.clone(),
            },
            actor,
            messages,
            work,
            stopped: false,
        })
    }
    /// Drive the shared save barrier before a synchronous export. No authored work runs here.
    pub fn flush(&mut self) -> Result<()> {
        self.request(Request::Flush)?;
        Ok(())
    }
    /// Run a trusted host operation through the same serial queue and disk effects.
    /// One drain returns its completion; an absent completion is never a retryable refusal.
    pub fn request(&mut self, request: Request) -> Result<Reply> {
        if self.stopped {
            return Err(Failure::new(FailureKind::Closed, "The document is closed"));
        }
        let (send, receive) = mpsc::channel();
        self.owner.submit(
            request,
            None,
            Box::new(move |reply| {
                let _ = send.send(reply);
            }),
        );
        self.drain();
        receive.try_recv().map_err(|_| Failure::new(FailureKind::Failed, "The owner result could not be confirmed"))?
    }
    /// Drain queued callbacks and persistence without changing the host clock.
    pub fn drain(&mut self) {
        self.poll(self.actor.now.as_millis() as u64, self.actor.unix_ms);
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
                    self.stop();
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
    fn stop(&mut self) {
        if self.stopped {
            return;
        }
        self.stopped = true;
        let (_, messages) = mpsc::channel();
        drop(std::mem::replace(&mut self.messages, messages));
        let (_, work) = mpsc::channel();
        drop(std::mem::replace(&mut self.work, work));
        self.actor.shutdown();
    }
}

impl Drop for BrowserDriver {
    fn drop(&mut self) {
        self.stop();
    }
}
