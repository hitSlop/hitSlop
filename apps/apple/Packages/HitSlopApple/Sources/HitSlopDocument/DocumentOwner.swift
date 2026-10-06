import Foundation
import HitSlopCore
import HitSlopCoreBinding

/// Document ownership. `queue` owns the core and every field below. The Rust store owns
/// SQLite, the writer lock and the save policy; every store call but `saveJob` and the
/// in-memory `theme` runs on `storageQueue`, one at a time, so a slow write never blocks
/// edits. Loro bytes never
/// reach Swift. Renderer lifetimes never determine the lifetime of this object or its
/// writer lock.
public final class DocumentOwner: @unchecked Sendable {
  public let file: SlopFile
  let mode: StoreMode
  let store: NativeStore
  /// The app's assets, on their own connection to the file this owner checked; pages read
  /// them while saves run.
  let assets: AssetReader
  let queue = DispatchQueue(label: "hitslop.owner")
  let storageQueue = DispatchQueue(label: "hitslop.persistence")
  private var core: NativeDocument
  /// The attached page, set by `open`. Page requests name it; a request from a replaced
  /// page is refused with `owner_replaced` and never applied.
  private var view: String?
  /// Names this owner's live state for socket clients. Minted here, never by the core, and
  /// rotated when unsaved edits are discarded, so a client's queued request cannot apply
  /// to replaced state.
  private let epochLock = NSLock()
  private var storedEpoch = UUID().uuidString
  public var epoch: String { epochLock.withLock { storedEpoch } }
  var onPublication: (@Sendable (String) -> Void)?
  /// Save status for the window: the edited mark and the save-failure sheet.
  var onSaveStatus: (@Sendable (DocumentSaveStatus) -> Void)?
  /// Signals an accepted theme change; sessions read the latest effective values.
  var onTheme: (@Sendable () -> Void)?
  /// Whether Edit ▸ Undo and Redo have anything to do, sent when that changes.
  var onUndoState: (@Sendable (UndoState) -> Void)?
  private var undoAvailability = UndoState(canUndo: false, canRedo: false)
  /// The core's publication sequence, the last one the durable state covers, and the one
  /// this owner opened at.
  private var sequence = 0
  private var savedSequence = 0
  private var openedSequence = 0
  /// Accepted theme changes, and the last one the durable state covers. A theme change is
  /// held in memory like an edit and saved by the same jobs. The revision only grows, so
  /// the window can tell which palette came after its own change.
  private var themeRevision = 0
  private var savedThemeRevision = 0
  private var unsaved: Bool { sequence > savedSequence || themeRevision > savedThemeRevision }
  private var autosave: DispatchWorkItem?
  /// When the oldest edit not yet handed to a write was accepted; autosave waits at most
  /// `autosaveMaximumMS` after it, so continuous typing still saves.
  private var unsavedSince: DispatchTime?
  private var saveFailure: SaveFailure?
  private var writing = false
  private var saveRequested = false
  private var waiters: [Waiter] = []
  /// `closing` refuses new edits while the final write runs; `closed` refuses everything.
  private enum Lifecycle { case open, closing, closed }
  private var lifecycle = Lifecycle.open
  /// Discard is reloading saved state; requests captured before it are refused.
  private var discarding = false
  /// The core refused every call; only a discard (or close) remains.
  private var invalidated = false

  private struct Waiter {
    let target: Int
    let themeTarget: Int
    let checkpoint: Bool
    let resume: @Sendable (Result<Void, Error>) -> Void
  }

  static let autosaveDelayMS = 150
  static let autosaveMaximumMS = 1000

  /// Opens the document at `url` (a canonical `.slop`, `SlopFile.resolvedRoot`): as its
  /// writer, or as a snapshot of its saved state. The store's open checks the file and its
  /// app once, and `file` comes from that check.
  public init(url: URL, mode: StoreMode = .document) throws {
    self.mode = mode
    // Taking the writer lock also removes a crashed session's discovery: a command that
    // finds the lock busy waits for this owner's address, never a dead one.
    let store = try storeCall { try SlopFile.opening { try NativeStore.open(path: url.path, mode: mode) } }
    self.store = store
    do {
      file = try SlopFile(url: url, opened: store.app())
      assets = try storeCall { try store.assetReader() }
      core = try Self.saved(store)
      sequence = Int(try core.sequence())
      savedSequence = sequence
      openedSequence = sequence
    } catch {
      try? store.close()
      throw error
    }
  }

  /// The saved document; a document without saved state starts from its app's initial
  /// values. Also the reload after a discard, on the storage queue: the new core is not
  /// shared until the owner queue installs it.
  private static func saved(_ store: NativeStore) throws -> NativeDocument {
    try storeCall { try store.document() }
  }

  private func enqueue<T: Sendable>(allowInvalidated: Bool = false, _ action: @escaping @Sendable () throws -> T) async throws -> T {
    try await withCheckedThrowingContinuation { continuation in
      queue.async {
        do {
          try self.admit(allowInvalidated: allowInvalidated)
          continuation.resume(returning: try action())
        } catch {
          self.checkPoisoned(error)
          continuation.resume(throwing: error)
        }
      }
    }
  }
  private func persist<T: Sendable>(_ action: @escaping @Sendable () throws -> T) async throws -> T {
    try await withCheckedThrowingContinuation { continuation in
      storageQueue.async { continuation.resume(with: Result { try action() }) }
    }
  }
  /// Refuses work once closed, or once invalidated unless `allowInvalidated`.
  private func admit(allowInvalidated: Bool = false) throws {
    guard lifecycle != .closed else { throw OwnerError.closed }
    guard allowInvalidated || !invalidated else { throw OwnerError.invalidated }
  }
  private func checkPoisoned(_ error: Error) {
    if case CoreError.Invalidated = error {
      invalidated = true
      publishStatus(.failed(.invalidated))
    }
  }
  /// A request captured before a discard (new epoch) or from a replaced page (new view)
  /// must not apply to state it never saw.
  private func requireCurrent(epoch: String?, view: String?) throws {
    if let epoch, epoch != self.epoch { throw OwnerReplaced() }
    if let view, view != self.view { throw OwnerReplaced() }
  }
  /// The one admission every change passes on the owner queue, whatever sent it: an
  /// editable owner that is open, not discarding, and still the state and page the
  /// request was made for.
  private func admitMutation(epoch: String?, view: String?) throws {
    guard mode == .document else { throw OwnerError.readOnly }
    guard lifecycle == .open else { throw OwnerError.closing }
    guard !discarding else { throw OwnerReplaced() }
    try requireCurrent(epoch: epoch, view: view)
  }

  /// `{sequence, version, value, issues}` as the core's JSON.
  public func state() async throws -> String { try await enqueue { try self.core.state() } }

  public struct Applied: Sendable {
    public let sequence: Int
    public let ids: [String]
  }

  /// Only the native session selects a page; an `open` request cannot replace it.
  func attach(view: String) { queue.async { self.view = view } }

  private func applyOnQueue(batch: String, epoch: String?, view: String?, origin: EditOrigin) throws -> Applied {
    try admitMutation(epoch: epoch, view: view)
    defer { refreshUndo() }
    let result = try core.applyBatch(batchJson: batch, origin: origin)
    // A batch that changed nothing publishes nothing and leaves the document clean.
    if let publication = result.publication { didEdit(publication, sequence: Int(result.sequence)) }
    return Applied(sequence: Int(result.sequence), ids: result.ids)
  }
  /// Socket and CLI edits are an agent's: tagged so a later window can still undo them.
  public func apply(batch: String, epoch: String? = nil, view: String? = nil, origin: EditOrigin = .agent) async throws -> Applied {
    try await enqueue { try self.applyOnQueue(batch: batch, epoch: epoch, view: view, origin: origin) }
  }
  private func historyOnQueue(redo: Bool, view: String?) throws -> Int {
    try admitMutation(epoch: nil, view: view)
    defer { refreshUndo() }
    let result = try redo ? core.redo() : core.undo()
    if let publication = result.publication { didEdit(publication, sequence: Int(result.sequence)) }
    return Int(result.sequence)
  }
  /// Edit ▸ Undo or Redo without a page to send its unsent edits first. Returns the
  /// publication sequence, unchanged when there was nothing to do.
  public func undo(redo: Bool = false) async throws -> Int {
    try await enqueue { try self.historyOnQueue(redo: redo, view: nil) }
  }
  /// Sends the current undo state: a document can open with an agent's edits to undo.
  func publishUndoState() { queue.async { self.refreshUndo() } }
  private func refreshUndo() {
    guard let state = try? core.undoState(), state != undoAvailability else { return }
    undoAvailability = state
    onUndoState?(state)
  }
  private func textOnQueue(_ request: String, view: String?) throws -> PageTextResult {
    try admitMutation(epoch: nil, view: view)
    defer { refreshUndo() }
    let result = try core.editText(requestJson: request)
    if let publication = result.publication { didEdit(publication, sequence: Int(result.sequence)) }
    return PageTextResult(sequence: Int(result.sequence), authored: result.authored,
      selectionStart: Int(result.selectionStart), selectionEnd: Int(result.selectionEnd))
  }

  /// Enqueues admission synchronously in bridge arrival order. Flush retains its
  /// completion without blocking subsequent edits behind the persistence queue.
  func enqueuePage(_ request: PageRequest, view: String,
    reply: @escaping @Sendable (sending Result<PageResult, Error>) -> Void
  ) {
    queue.async {
      do {
        try self.admit()
        try self.requireCurrent(epoch: nil, view: view)
        switch request {
        case .open: reply(.success(.open(.init(state: try self.core.state()))))
        case .apply(let r):
          let applied = try self.applyOnQueue(batch: r.batch, epoch: nil, view: view, origin: .page)
          reply(.success(.apply(.init(sequence: applied.sequence, ids: applied.ids))))
        case .text(let r): reply(.success(.text(try self.textOnQueue(r.request, view: view))))
        case .undo: reply(.success(.undo(.init(sequence: try self.historyOnQueue(redo: false, view: view)))))
        case .redo: reply(.success(.redo(.init(sequence: try self.historyOnQueue(redo: true, view: view)))))
        case .flush: self.addWaiter(checkpoint: false) { reply($0.map { .flush }) }
        default: throw OwnerError.rejected("Not a document request")
        }
      } catch {
        self.checkPoisoned(error)
        reply(.failure(error))
      }
    }
  }
  private func publishStatus(_ status: DocumentSaveStatus) {
    switch status {
    case .failed(let failure): saveFailure = failure
    case .saved: saveFailure = nil
    case .saving: break
    }
    onSaveStatus?(saveFailure.map { .failed($0) } ?? status)
  }
  func currentSaveFailure() async throws -> SaveFailure? {
    try await enqueue(allowInvalidated: true) { self.saveFailure }
  }
  private func didEdit(_ publication: String, sequence next: Int) {
    let wasSaved = !unsaved
    sequence = next
    onPublication?(publication)
    scheduleSave(wasSaved: wasSaved)
  }
  private func didChangeTheme() {
    let wasSaved = !unsaved
    themeRevision += 1
    onTheme?()
    scheduleSave(wasSaved: wasSaved)
  }
  private func scheduleSave(wasSaved: Bool) {
    // Status changes once when the document becomes dirty, not on every keystroke.
    if wasSaved { publishStatus(.saving) }
    // Each change restarts the short delay, but never past the maximum wait.
    autosave?.cancel()
    let now = DispatchTime.now(), since = unsavedSince ?? now
    unsavedSince = since
    let task = DispatchWorkItem { [weak self] in
      guard let self, (try? self.admit()) != nil else { return }
      self.saveRequested = true
      self.pump()
    }
    autosave = task
    queue.asyncAfter(deadline: min(now + .milliseconds(Self.autosaveDelayMS), since + .milliseconds(Self.autosaveMaximumMS)), execute: task)
  }

  // MARK: Writes. Scheduling runs on `queue`; database work runs on `storageQueue`.

  /// Starts the next write when none is in flight and there is something to save. The
  /// store chooses what to write and exports it here; the write runs on `storageQueue`.
  private func pump() {
    guard !writing, !discarding, (try? admit()) != nil else { return }
    let forceCheckpoint = waiters.contains { $0.checkpoint }
    guard unsaved || forceCheckpoint else { return settle(checkpointed: false) }
    saveRequested = false
    unsavedSince = nil
    do {
      guard let job = try storeCall({ try core.saveJob(store: store, forceCheckpoint: forceCheckpoint) }) else {
        // The durable state already covers these edits.
        savedSequence = sequence
        savedThemeRevision = themeRevision
        publishStatus(.saved)
        return settle(checkpointed: false)
      }
      writing = true
      let epoch = self.epoch, target = sequence, themeTarget = themeRevision
      storageQueue.async {
        let result = Result { try storeCall { try self.store.write(job: job) } }
        self.queue.async {
          self.finish(epoch: epoch, target: target, themeTarget: themeTarget, checkpoint: job.isCheckpoint(), result)
        }
      }
    } catch {
      checkPoisoned(error)
      // A poisoned core must keep reporting invalidation, not a generic I/O failure.
      fail(invalidated ? .invalidated : SaveFailure(error), upTo: .max)
    }
  }
  /// The store re-reads its sizes after a failure, so the next write chooses correctly.
  private func finish(epoch: String, target: Int, themeTarget: Int, checkpoint: Bool, _ result: Result<Void, Error>) {
    guard epoch == self.epoch else { return }
    writing = false
    switch result {
    case .success:
      savedSequence = max(savedSequence, target)
      savedThemeRevision = max(savedThemeRevision, themeTarget)
      saveFailure = nil
      publishStatus(unsaved ? .saving : .saved)
      settle(checkpointed: checkpoint)
    case .failure(let error): fail(SaveFailure(error), upTo: target, themeTarget: themeTarget)
    }
    // After a failure, retry only for work that arrived during the write; never spin.
    if saveRequested || !waiters.isEmpty { pump() }
  }
  /// Resumes waiters whose edits, theme changes (and requested checkpoint) are durable.
  private func settle(checkpointed: Bool) {
    waiters.removeAll { waiter in
      guard waiter.target <= savedSequence, waiter.themeTarget <= savedThemeRevision,
        !waiter.checkpoint || checkpointed
      else { return false }
      waiter.resume(.success(()))
      return true
    }
  }
  private func fail(_ failure: SaveFailure, upTo target: Int, themeTarget: Int = .max) {
    publishStatus(.failed(failure))
    waiters.removeAll { waiter in
      guard waiter.target <= target, waiter.themeTarget <= themeTarget else { return false }
      waiter.resume(.failure(failure))
      return true
    }
  }
  /// Waits until every edit accepted so far is durable. Refused while discarding: the
  /// restored core restarts its publication sequence, so a waiter captured before the
  /// restore would wait for a sequence that never arrives.
  private func addWaiter(checkpoint: Bool, _ resume: @escaping @Sendable (Result<Void, Error>) -> Void) {
    do { try admit() } catch { return resume(.failure(error)) }
    if discarding { return resume(.failure(OwnerReplaced())) }
    waiters.append(Waiter(target: sequence, themeTarget: themeRevision, checkpoint: checkpoint, resume: resume))
    pump()
  }
  private func rejectWaiters(_ error: Error) {
    let rejected = waiters
    waiters.removeAll()
    for waiter in rejected { waiter.resume(.failure(error)) }
  }
  private func write(checkpoint: Bool) async throws {
    try await withCheckedThrowingContinuation { (continuation: CheckedContinuation<Void, Error>) in
      queue.async { self.addWaiter(checkpoint: checkpoint) { continuation.resume(with: $0) } }
    }
  }
  public func flush() async throws { try await write(checkpoint: false) }
  /// Saves everything as one checkpoint with no history, once admitted for `epoch`.
  public func compact(epoch: String? = nil) async throws {
    try await enqueue { try self.admitMutation(epoch: epoch, view: nil) }
    try await write(checkpoint: true)
  }

  /// After the final write, a session that edited a document larger than 4 MiB leaves no
  /// history (`Store::close_job`). Housekeeping: on failure the saved state is unchanged
  /// and closing continues.
  private func trimHistory() async {
    let store = store
    guard let job = try? await enqueue({ try storeCall { try self.core.closeJob(store: store) } }) else { return }
    _ = try? await persist { try storeCall { try store.write(job: job) } }
  }

  /// Drops unsaved edits and reloads saved state. The epoch fences a write in flight: it
  /// finishes on the persistence queue before the reload reads, and its reply is ignored.
  public func discardPending() async throws {
    try await enqueue(allowInvalidated: true) {
      guard !self.discarding else { throw OwnerReplaced() }
      self.epochLock.withLock { self.storedEpoch = UUID().uuidString }
      self.discarding = true
      self.autosave?.cancel()
      self.rejectWaiters(OwnerReplaced())
    }
    do {
      let store = store
      let restored = try await persist { try Self.saved(store) }
      try await enqueue(allowInvalidated: true) {
        // Sequences restart with the restored core; nothing may wait on the old ones.
        // Read everything that can throw before replacing any state.
        let sequence = Int(try restored.sequence())
        self.rejectWaiters(OwnerReplaced())
        self.core = restored
        self.refreshUndo()
        self.invalidated = false
        self.writing = false
        self.view = nil
        self.sequence = sequence
        self.savedSequence = sequence
        // The reload read the saved theme too.
        self.savedThemeRevision = self.themeRevision
        self.discarding = false
        self.publishStatus(.saved)
        self.onTheme?()
        self.pump()
      }
    } catch {
      try await enqueue(allowInvalidated: true) {
        self.discarding = false
        self.writing = false
        // The save-failure sheet presents every save failure, so a failed reload is
        // published like a failed write; an invalidated owner publishes nothing else.
        self.publishStatus(.failed(self.invalidated ? .invalidated : SaveFailure(error)))
        self.pump()
      }
      throw error
    }
  }
  /// Whether this session saved any edit or theme change: its artwork may be out of date.
  func edited() async -> Bool {
    (try? await enqueue(allowInvalidated: true) { self.savedSequence > self.openedSequence || self.savedThemeRevision > 0 }) ?? false
  }

  // MARK: Discovery and copies. The store holds the writer lock, so it alone names this
  // owner's socket in the registry, and copies an open document from its own connection.

  /// Names this owner's live socket for clients. Snapshot owners never publish.
  func publishDiscovery(_ json: Data) throws {
    guard mode == .document else { return }
    try storeCall { try store.publishDiscovery(json: String(decoding: json, as: UTF8.self)) }
  }
  func withdrawDiscovery() {
    guard mode == .document else { return }
    store.withdrawDiscovery()
  }
  /// Copies the open document to `destination` as a new logical document, after saving
  /// what it accepted. Saves wait behind the copy on the storage queue.
  func copy(to destination: URL) async throws {
    try await flush()
    let store = store
    try await persist { try storeCall { try store.copyTo(destination: destination.path) } }
  }

  // MARK: Host-owned attachments and theme. Page and socket calls go through the owner,
  // so they pass its admission.

  /// One of the document's artwork images, through the owner's own connection.
  func artwork(_ name: SlopArtwork.Name) async throws -> Data? {
    let store = store
    return try await persist { try storeCall { try store.artwork(name: name.rawValue) } }
  }
  func listAttachments() async throws -> [PageAttachmentsPutResult] {
    let store = store
    return try await persist {
      try storeCall { try store.attachments() }.map { PageAttachmentsPutResult(id: $0.id, byteLength: Int($0.byteLength)) }
    }
  }
  /// Attachment bytes cross the page bridge and the socket as base64, encoded here, off
  /// the main thread.
  func readAttachment(_ id: String) async throws -> String {
    let store = store
    return try await persist { try storeCall { try store.attachment(id: id) }.base64EncodedString() }
  }
  /// Stores an attachment once admitted like an edit; snapshot renders own nothing, so
  /// they can never add one.
  func putAttachment(base64 encoded: String, epoch: String? = nil, view: String? = nil) async throws -> PageAttachmentsPutResult {
    try await enqueue { try self.admitMutation(epoch: epoch, view: view) }
    let store = store
    return try await persist {
      guard let bytes = Data(base64Encoded: encoded) else { throw OwnerError.rejected("Invalid attachment bytes") }
      let stored = try storeCall { try store.putAttachment(bytes: bytes) }
      return PageAttachmentsPutResult(id: stored.id, byteLength: Int(stored.byteLength))
    }
  }
  /// A palette and the owner's theme revision when it was read or changed.
  struct ThemeRead: Sendable {
    let state: ThemeState
    let revision: Int
  }
  /// The palette, validated and merged by the core.
  func loadTheme() async throws -> ThemeRead {
    try await enqueue { try self.themeOnQueue(.get, epoch: nil) }
  }
  /// A theme command, under the core's one rule set. A change is accepted in memory on the
  /// edit queue like an edit, restyles the page through `onTheme`, and is saved by the
  /// same jobs, so flush, close and retry cover it. Page panel and CLI changes both land
  /// here; a snapshot answers from the theme it read and refuses changes.
  func applyTheme(_ change: ThemeChange, epoch: String? = nil) async throws -> ThemeState {
    try await enqueue { try self.themeOnQueue(change, epoch: epoch).state }
  }
  /// Enqueues a panel change synchronously, so changes apply in the order they are made.
  func enqueueTheme(_ change: SlopThemeChange, reply: @escaping @Sendable (Result<ThemeRead, Error>) -> Void) {
    queue.async {
      do {
        try self.admit()
        let core: ThemeChange
        switch change {
        case .set(let values):
          core = .set(valuesJson: String(decoding: try JSONSerialization.data(withJSONObject: values), as: UTF8.self))
        case .resetAll: core = .reset(token: nil)
        case .importFile(let file): core = .import(fileJson: file)
        }
        reply(.success(try self.themeOnQueue(core, epoch: nil)))
      } catch {
        self.checkPoisoned(error)
        reply(.failure(error))
      }
    }
  }
  /// The full palette as a theme file for this document's template, as the core writes
  /// it, once it is saved: export is behind the same barrier as close, so a failing save
  /// fails the export.
  func exportTheme() async throws -> String {
    let file = try await enqueue { try storeCall { try self.store.exportTheme() } }
    try await flush()
    return file
  }
  private func themeOnQueue(_ change: ThemeChange, epoch: String?) throws -> ThemeRead {
    if case .get = change {} else { try admitMutation(epoch: epoch, view: nil) }
    let state = try storeCall { try store.theme(change: change) }
    if state.changed { didChangeTheme() }
    return ThemeRead(state: state, revision: themeRevision)
  }
  /// Refuses new edits, writes everything accepted and the window's `artwork`, then
  /// releases the lock. A failed final write keeps ownership and the live state so the
  /// window can retry; artwork never fails a close.
  public func close(artwork: SlopRenderedArtwork? = nil) async throws {
    try await enqueue {
      self.lifecycle = .closing
      self.autosave?.cancel()
    }
    do {
      try await flush()
      if mode == .document { await trimHistory() }
      let store = store
      if let artwork, mode == .document {
        do { try await persist { try storeCall { try store.setArtwork(preview: artwork.preview, icon: artwork.icon) } } }
        catch { NSLog("hitSlop: artwork was not saved: %@", error.localizedDescription) }
      }
      try await persist { try storeCall { try store.close() } }
      try await enqueue(allowInvalidated: true) { self.lifecycle = .closed }
    } catch {
      try await enqueue(allowInvalidated: true) { self.lifecycle = .open }
      throw error
    }
  }
}

extension DocumentOwner {
  /// The embedded core's build ID; a release checks the app and helper report the same one.
  public static var coreBuildID: String { coreBuildId() }
}

/// Finder and Quick Look artwork a window rendered from its document as it closed.
public struct SlopRenderedArtwork: Sendable {
  public let preview: Data?
  public let icon: Data?
  public init(preview: Data?, icon: Data?) {
    self.preview = preview
    self.icon = icon
  }
}

/// The page or epoch a request was captured for has been replaced (discard, reload or a
/// new view); the request was not applied.
public struct OwnerReplaced: LocalizedError {
  public var errorDescription: String? { "owner_replaced: the document was reloaded; this edit was not applied" }
}
