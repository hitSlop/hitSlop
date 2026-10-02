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
  public let package: SlopPackage
  let mode: StorageMode
  let store: NativeStore
  let queue = DispatchQueue(label: "hitslop.owner")
  let storageQueue = DispatchQueue(label: "hitslop.persistence")
  #if DEBUG
  /// Fault injection at the storage I/O boundary. Fires on `storageQueue`.
  var testingPhase: ((String) throws -> Void)? {
    didSet { store.setPhases(phases: testingPhase.map(PhaseHook.init)) }
  }
  #endif
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
  var onUndoState: (@Sendable (UndoAvailability) -> Void)?
  private var undoAvailability = UndoAvailability()
  /// The core's publication sequence, and the last one the durable state covers.
  private var sequence = 0
  private var savedSequence = 0
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

  /// Benchmarks can push autosave out of the edit loop to attribute its cost.
  static let autosaveDelayMS: Int = {
    #if DEBUG
    if let value = ProcessInfo.processInfo.environment["HITSLOP_AUTOSAVE_MS"], let ms = Int(value) { return ms }
    #endif
    return 150
  }()
  static let autosaveMaximumMS = max(autosaveDelayMS, 1000)

  public init(package: SlopPackage, mode: StorageMode = .document) throws {
    self.package = package
    self.mode = mode
    store = try storeCall { try NativeStore.open(root: package.rootURL.path, mode: mode.store) }
    // The writer lock is ours, so a discovery file is a crashed session's leftover. A
    // command that finds the lock busy must wait for this owner's address, not read a dead one.
    if mode == .document { try? FileManager.default.removeItem(at: package.discoveryURL) }
    do {
      core = try Self.saved(package, store)
      sequence = Int(try core.sequence())
      savedSequence = sequence
    } catch {
      try? store.close()
      throw error
    }
  }

  /// The saved document; a package without saved state starts from its initial value.
  /// Also the reload after a discard, on the storage queue: the new core is not shared
  /// until the owner queue installs it.
  private static func saved(_ package: SlopPackage, _ store: NativeStore) throws -> NativeDocument {
    let initial = String(decoding: try SlopFile.read(package.initialURL, within: package.rootURL), as: UTF8.self)
    return try storeCall {
      try store.document(schemaKey: package.schemaKey, initialJson: initial, themeDefaultsJson: package.themeDefaults)
    }
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
  private func requireEditable() throws {
    guard mode == .document else { throw OwnerError.readOnly }
    guard lifecycle == .open else { throw OwnerError.closing }
    guard !discarding else { throw OwnerReplaced() }
  }
  /// A request captured before a discard (new epoch) or from a replaced page (new view)
  /// must not apply to state it never saw.
  private func requireCurrent(epoch: String?, view: String?) throws {
    if let epoch, epoch != self.epoch { throw OwnerReplaced() }
    if let view, view != self.view { throw OwnerReplaced() }
  }

  /// `{sequence, version, value, issues}` as the core's JSON.
  public func state() async throws -> String { try await enqueue { try self.core.state() } }
  /// The application value as the core's JSON.
  func value() async throws -> String { try await enqueue { try self.core.value() } }

  public struct Applied: Sendable {
    public let sequence: Int
    public let ids: [String]
  }
  public struct TextEdit: Sendable {
    public let sequence: Int
    public let authored: String
    public let selectionStart: Int
    public let selectionEnd: Int
  }
  public struct Opened: Sendable {
    public let state: String
  }

  /// Only the native session selects a page; an `open` request cannot replace it.
  func attach(view: String) { queue.async { self.view = view } }

  private func openOnQueue(view: String) throws -> Opened {
    try requireCurrent(epoch: nil, view: view)
    return Opened(state: try core.state())
  }
  func open(view: String) async throws -> Opened {
    try await enqueue { try self.openOnQueue(view: view) }
  }
  private func applyOnQueue(batch: String, epoch: String?, view: String?, origin: EditOrigin) throws -> Applied {
    try requireEditable()
    try requireCurrent(epoch: epoch, view: view)
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
    try requireEditable()
    try requireCurrent(epoch: nil, view: view)
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
    guard let state = try? core.undoState() else { return }
    let next = UndoAvailability(canUndo: state.canUndo, canRedo: state.canRedo)
    guard next != undoAvailability else { return }
    undoAvailability = next
    onUndoState?(next)
  }
  private func textOnQueue(_ request: String, view: String?) throws -> TextEdit {
    try requireEditable()
    try requireCurrent(epoch: nil, view: view)
    defer { refreshUndo() }
    let result = try core.editText(requestJson: request)
    if let publication = result.publication { didEdit(publication, sequence: Int(result.sequence)) }
    return TextEdit(sequence: Int(result.sequence), authored: result.authored,
      selectionStart: Int(result.selectionStart), selectionEnd: Int(result.selectionEnd))
  }

  /// Enqueues admission synchronously in bridge arrival order. Flush retains its
  /// completion without blocking subsequent edits behind the persistence queue.
  func enqueuePage(_ command: PageCommand, view: String,
    reply: @escaping @Sendable (Result<PageOutcome, Error>) -> Void
  ) {
    queue.async {
      do {
        try self.admit()
        try self.requireCurrent(epoch: nil, view: view)
        switch command {
        case .open: reply(.success(.opened(try self.openOnQueue(view: view))))
        case .apply(let batch): reply(.success(.applied(try self.applyOnQueue(batch: batch, epoch: nil, view: view, origin: .page))))
        case .text(let request): reply(.success(.text(try self.textOnQueue(request, view: view))))
        case .undo, .redo:
          reply(.success(.history(try self.historyOnQueue(redo: { if case .redo = command { true } else { false } }(), view: view))))
        case .flush: self.addWaiter(checkpoint: false) { reply($0.map { .flushed }) }
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
  public func compact() async throws {
    guard mode == .document else { throw OwnerError.readOnly }
    try await write(checkpoint: true)
  }

  /// After the final write, a session that edited a large document keeps only its own
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
      let package = package, store = store
      let restored = try await persist { try Self.saved(package, store) }
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
  // MARK: Host-owned attachments and theme. Page and socket calls go through the owner,
  // so they honor its closed and closing guards.

  private func requireWritable() async throws {
    try await enqueue { guard self.lifecycle == .open else { throw OwnerError.closing } }
  }
  func listAttachments() async throws -> [AttachmentRef] {
    let store = store, root = package.rootURL
    return try await persist {
      try storeCall { try store.check(writable: false) }
      return try SlopAttachments.list(in: root)
    }
  }
  /// Attachment bytes cross the page bridge and the socket as base64, encoded here, off
  /// the main thread.
  func readAttachment(_ id: String) async throws -> String {
    let store = store, root = package.rootURL
    return try await persist {
      try storeCall { try store.check(writable: false) }
      return try SlopAttachments.read(id, in: root).base64EncodedString()
    }
  }
  /// Snapshot renders own nothing, so they can never add attachments.
  func putAttachment(base64 encoded: String) async throws -> AttachmentRef {
    try await requireWritable()
    let store = store, root = package.rootURL
    return try await persist {
      guard let bytes = Data(base64Encoded: encoded) else { throw OwnerError.rejected("Invalid attachment bytes") }
      try storeCall { try store.check(writable: true) }
      return try SlopAttachments.put(bytes, in: root)
    }
  }
  /// A palette and the owner's theme revision when it was read or changed.
  struct ThemeRead: Sendable {
    let state: ThemeState
    let revision: Int
  }
  /// The palette, validated and merged by the core.
  func loadTheme() async throws -> ThemeRead {
    try await enqueue { try self.themeOnQueue(.get) }
  }
  /// A theme command, under the core's one rule set. A change is accepted in memory on the
  /// edit queue like an edit, restyles the page through `onTheme`, and is saved by the
  /// same jobs, so flush, close and retry cover it. Page panel and CLI changes both land
  /// here; a snapshot answers from the theme it read and refuses changes.
  func applyTheme(_ change: ThemeChange) async throws -> ThemeState {
    try await enqueue { try self.themeOnQueue(change).state }
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
        case .importFile(let file): core = self.importTheme(file)
        }
        reply(.success(try self.themeOnQueue(core)))
      } catch {
        self.checkPoisoned(error)
        reply(.failure(error))
      }
    }
  }
  /// Replaces the theme with a theme file made for this document's template.
  func importTheme(_ file: String) -> ThemeChange {
    .import(template: package.manifest.slug, fileJson: file)
  }
  /// The full palette as a theme file for this document's template, once it is saved:
  /// export is behind the same barrier as close, so a failing save fails the export.
  func exportTheme() async throws -> String {
    let file = try await enqueue { try storeCall { try self.store.exportTheme(template: self.package.manifest.slug) } }
    try await flush()
    return file
  }
  private func themeOnQueue(_ change: ThemeChange) throws -> ThemeRead {
    if case .get = change {} else { try requireEditable() }
    let state = try storeCall { try store.theme(change: change) }
    if state.changed { didChangeTheme() }
    return ThemeRead(state: state, revision: themeRevision)
  }
  /// Refuses new edits, writes everything accepted, then releases the lock. A failed
  /// final write keeps ownership and the live state so the window can retry.
  public func close() async throws {
    try await enqueue {
      self.lifecycle = .closing
      self.autosave?.cancel()
    }
    do {
      try await flush()
      if mode == .document { await trimHistory() }
      let store = store
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

/// The page or epoch a request was captured for has been replaced (discard, reload or a
/// new view); the request was not applied.
public struct OwnerReplaced: LocalizedError {
  public var errorDescription: String? { "owner_replaced: the document was reloaded; this edit was not applied" }
}
