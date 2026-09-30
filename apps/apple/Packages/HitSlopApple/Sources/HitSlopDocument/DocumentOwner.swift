import Foundation
import HitSlopCore
import HitSlopCoreBinding

/// Document ownership. `queue` owns the core and every field below; SQLite runs only on
/// `storage.queue`, one write at a time, so a slow write never blocks edits.
/// Renderer lifetimes never determine the lifetime of this object or its writer lock.
public final class DocumentOwner: @unchecked Sendable {
  public let package: SlopPackage
  let storage: Storage
  let queue = DispatchQueue(label: "hitslop.owner")
  private var core: NativeDocument
  public private(set) var session: String
  /// Names this owner's live state for socket clients. Minted here, never by the core, and
  /// rotated when unsaved edits are discarded, so a client's queued request cannot apply
  /// to replaced state.
  public private(set) var epoch = UUID().uuidString
  public let documentID: String
  var onPublication: (@Sendable (String) -> Void)?
  var onSaveStatus: (@Sendable (String, SaveFailure?, Int) -> Void)?
  private var sequence = 0
  private var autosave: DispatchWorkItem?
  private let schemaKey: String
  private var generation: Int64
  private var savedVersion: String
  private var saveFailure: SaveFailure?
  /// Accepted edits, and how many of them the durable state covers.
  private var edits = 0
  private var savedEdits = 0
  /// Bumped by discard; a write completion from an older epoch is ignored.
  private var writeEpoch = 0
  private var writing = false
  private var saveRequested = false
  private var waiters: [Waiter] = []
  /// Stored sizes, tracked here so choosing append or checkpoint needs no database read.
  private var stored: (rows: Int64, updateBytes: Int64, checkpointBytes: Int64)
  /// A write whose reply was lost and whose outcome is not yet known.
  private var uncertainWrite: WriteJob?
  private var closing = false
  private var discarding = false
  private var closed = false
  private var invalidated = false

  private struct Waiter {
    let target: Int
    let checkpoint: Bool
    let continuation: CheckedContinuation<Void, Error>
  }
  private struct WriteJob: @unchecked Sendable {
    let epoch: Int
    let attempt: String
    let target: Int
    let version: String
    let checkpoint: Bool
    let bytes: Int64
    let generation: Int64
    let write: Storage.Write
  }
  private enum Outcome: Sendable {
    case committed(Int64)
    case failed(SaveFailure)
    /// The reply was lost and the attempt token could not be read.
    case uncertain(SaveFailure)
  }
  private struct Restored: @unchecked Sendable {
    let core: NativeDocument
    let generation: Int64
    let stored: (rows: Int64, updateBytes: Int64, checkpointBytes: Int64)
  }

  /// Benchmarks can push autosave out of the edit loop to attribute its cost.
  static let autosaveDelayMS: Int = {
    #if DEBUG
    if let value = ProcessInfo.processInfo.environment["HITSLOP_AUTOSAVE_MS"], let ms = Int(value) { return ms }
    #endif
    return 150
  }()

  public init(package: SlopPackage, mode: StorageMode = .document) throws {
    self.package = package
    let descriptor = try SlopFile.read(package.dataSchemaURL, within: package.rootURL, maximumBytes: 1_048_576)
    let canonical = try JSONSerialization.data(withJSONObject: JSONSerialization.jsonObject(with: descriptor), options: [.sortedKeys, .withoutEscapingSlashes])
    schemaKey = String(decoding: canonical, as: UTF8.self)
    storage = try Storage(root: package.rootURL, mode: mode)
    do {
      let loaded = try storage.load()
      documentID = loaded.docID
      if loaded.checkpoint != nil {
        let restored = try Self.restore(loaded, schemaKey: schemaKey, storage: storage)
        core = restored.core
        generation = restored.generation
        stored = restored.stored
      } else {
        let initial = try SlopFile.read(package.initialURL, within: package.rootURL)
        core = try NativeDocument.create(schemaJson: schemaKey, initialJson: String(decoding: initial, as: UTF8.self))
        let checkpoint = try core.checkpoint()
        generation = try storage.write(.checkpoint(checkpoint, schemaKey: schemaKey), generation: loaded.generation, attempt: UUID().uuidString)
        stored = (0, 0, Int64(checkpoint.count))
      }
      savedVersion = try core.version()
      let frame = try JSONSerialization.jsonObject(with: Data(core.snapshot().utf8)) as! [String: Any]
      session = frame["session"] as! String
      sequence = frame["sequence"] as! Int
    } catch {
      storage.close()
      throw error
    }
  }

  /// Opens saved bytes as a new core. Runs on the persistence queue during discard; the
  /// new core is not shared until the owner queue installs it.
  private static func restore(_ loaded: Storage.Loaded, schemaKey: String, storage: Storage) throws -> Restored {
    guard loaded.schemaKey == schemaKey else { throw failure("Document schema differs from saved state") }
    guard let checkpoint = loaded.checkpoint else { throw failure("Missing durable checkpoint") }
    let core = try NativeDocument.open(schemaJson: schemaKey, checkpoint: checkpoint, updates: loaded.updates)
    let meta = try storage.metadata()
    return Restored(core: core, generation: loaded.generation, stored: (meta.rows, meta.updateBytes, meta.checkpointBytes))
  }

  private func enqueue<T: Sendable>(allowInvalidated: Bool = false, _ action: @escaping @Sendable () throws -> T) async throws -> T {
    try await withCheckedThrowingContinuation { continuation in
      queue.async {
        do {
          guard !self.closed else { throw failure("Document owner is closed") }
          guard allowInvalidated || !self.invalidated else { throw failure("Owner invalidated; explicit recovery is required") }
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
      storage.queue.async { continuation.resume(with: Result { try action() }) }
    }
  }
  private func checkPoisoned(_ error: Error) {
    if case CoreError.Invalidated = error {
      invalidated = true
      publishStatus("save-failed", .invalidated, sequence)
    }
  }
  private func requireEditable() throws {
    guard storage.mode == .document else { throw failure("Read-only capture cannot edit") }
    guard !closing else { throw failure("Document is closing") }
    guard !discarding else { throw failure("session_changed") }
  }

  public func state() async throws -> String { try await enqueue { try self.core.snapshot() } }

  /// Applies one batch. Callers never resend a request; a lost reply is resolved by
  /// reading state, never by replaying.
  public func apply(session: String, batch: String, current: Bool = false) async throws -> String {
    try await enqueue {
      try self.requireEditable()
      guard self.session == session else { throw failure("session_changed") }
      guard batch.utf8.count <= 4 * 1024 * 1024 else { throw failure("Request exceeds size limit") }
      let reply = try current ? self.core.commandCurrent(batchJson: batch) : self.core.apply(batchJson: batch)
      self.didEdit(reply)
      return reply
    }
  }

  public func text(_ request: String) async throws -> String {
    try await enqueue {
      try self.requireEditable()
      let reply = try self.core.text(requestJson: request)
      self.didEdit(reply)
      return reply
    }
  }
  public func releaseDraft(_ draft: String) async throws {
    try await enqueue { try self.core.releaseDraft(draft: draft) }
  }
  private func publishStatus(_ status: String, _ error: SaveFailure?, _ sequence: Int) {
    if status == "save-failed" { saveFailure = error }
    else if status == "saved" { saveFailure = nil }
    onSaveStatus?(saveFailure == nil ? status : "save-failed", saveFailure, sequence)
  }
  func republishStatus() async throws {
    try await enqueue(allowInvalidated: true) {
      self.onSaveStatus?(self.saveFailure == nil ? (self.edits > self.savedEdits ? "pending" : "saved") : "save-failed", self.saveFailure, self.sequence)
    }
  }
  private func didEdit(_ publication: String) {
    edits += 1
    if let reply = try? JSONSerialization.jsonObject(with: Data(publication.utf8)) as? [String: Any], let patch = reply["patch"] as? [String: Any], let next = patch["sequence"] as? Int { sequence = next }
    onPublication?(publication)
    // Status changes once when the document becomes dirty, not on every keystroke.
    if edits == savedEdits + 1 { publishStatus("pending", nil, sequence) }
    autosave?.cancel()
    let task = DispatchWorkItem { [weak self] in
      guard let self, !self.closed, !self.invalidated else { return }
      self.saveRequested = true
      self.pump()
    }
    autosave = task
    queue.asyncAfter(deadline: .now() + .milliseconds(Self.autosaveDelayMS), execute: task)
  }

  // MARK: Writes. Everything here runs on `queue`; only `perform` runs on `storage.queue`.

  /// Starts the next write when none is in flight and there is something to save.
  private func pump() {
    guard !writing, !closed, !discarding, !invalidated else { return }
    if let pending = uncertainWrite { resolve(pending); return }
    let forceCheckpoint = waiters.contains { $0.checkpoint }
    guard edits > savedEdits || forceCheckpoint else { return settle(checkpointed: false) }
    saveRequested = false
    do {
      let version = try core.version()
      if version == savedVersion && !forceCheckpoint {
        savedEdits = edits
        publishStatus("saved", nil, sequence)
        return settle(checkpointed: false)
      }
      let job = try makeJob(version: version, forceCheckpoint: forceCheckpoint)
      writing = true
      storage.queue.async {
        let outcome = self.perform(job)
        self.queue.async { self.finish(job, outcome) }
      }
    } catch {
      checkPoisoned(error)
      fail(error as? SaveFailure ?? .io(error.localizedDescription), upTo: .max)
    }
  }
  private func makeJob(version: String, forceCheckpoint: Bool) throws -> WriteJob {
    let dirty = version != savedVersion
    let updates = dirty ? try core.exportSince(version: savedVersion) : Data()
    let canAppend = dirty && stored.rows < Storage.maximumRows
      && stored.checkpointBytes + stored.updateBytes + Int64(updates.count) <= Storage.maximumBytes
    let wantsCheckpoint = forceCheckpoint || !canAppend || stored.rows >= 256 || stored.updateBytes >= 4 * 1024 * 1024
    let attempt = UUID().uuidString
    if wantsCheckpoint {
      let checkpoint = try core.checkpoint()
      // SQLite also bounds the complete checkpoint row (including its schema key).
      if Int64(checkpoint.count + schemaKey.utf8.count + 512) <= Storage.maximumBytes {
        return WriteJob(epoch: writeEpoch, attempt: attempt, target: edits, version: version, checkpoint: true, bytes: Int64(checkpoint.count),
          generation: generation, write: .checkpoint(checkpoint, schemaKey: schemaKey))
      } else if forceCheckpoint || !canAppend {
        throw SaveFailure.full
      }
      // Optional maintenance must not prevent an update that still fits the log.
    }
    return WriteJob(epoch: writeEpoch, attempt: attempt, target: edits, version: version, checkpoint: false, bytes: Int64(updates.count),
      generation: generation, write: .append(updates))
  }
  /// A successful SQLite commit may lose its reply. The stored attempt token says whether
  /// the write committed; never guess from the generation, which another writer can advance.
  private func perform(_ job: WriteJob) -> Outcome {
    do {
      return .committed(try storage.write(job.write, generation: job.generation, attempt: job.attempt))
    } catch {
      let failure = error as? SaveFailure ?? .io(error.localizedDescription)
      // BUSY at BEGIN or COMMIT rolls back: a definite failure that needs no recovery read.
      if failure == .busy { return .failed(failure) }
      guard let meta = try? storage.metadata() else { return .uncertain(failure) }
      if meta.lastAttempt == job.attempt { return .committed(meta.generation) }
      return .failed(failure)
    }
  }
  private func finish(_ job: WriteJob, _ outcome: Outcome) {
    guard job.epoch == writeEpoch else { return }
    writing = false
    switch outcome {
    case .committed(let observed): commit(job, generation: observed)
    case .failed(let failure): fail(failure, upTo: job.target)
    case .uncertain(let failure):
      uncertainWrite = job
      fail(failure, upTo: job.target)
    }
    // After a failure, retry only for work that arrived during the write; never spin.
    if saveRequested || !waiters.isEmpty { pump() }
  }
  private func commit(_ job: WriteJob, generation observed: Int64) {
    generation = observed
    savedVersion = job.version
    savedEdits = max(savedEdits, job.target)
    stored = job.checkpoint ? (0, 0, job.bytes) : (stored.rows + 1, stored.updateBytes + job.bytes, stored.checkpointBytes)
    saveFailure = nil
    publishStatus(edits > savedEdits ? "pending" : "saved", nil, sequence)
    settle(checkpointed: job.checkpoint)
  }
  /// Settles a lost reply before the next write, which must build on the right generation.
  private func resolve(_ pending: WriteJob) {
    writing = true
    storage.queue.async {
      let meta = try? self.storage.metadata()
      let attempt = meta?.lastAttempt, observed = meta?.generation
      self.queue.async {
        guard pending.epoch == self.writeEpoch else { return }
        self.writing = false
        guard let observed else { return self.fail(.io("Could not confirm the last save; retry saving"), upTo: .max) }
        self.uncertainWrite = nil
        if attempt == pending.attempt { self.commit(pending, generation: observed) } else { self.generation = observed }
        self.pump()
      }
    }
  }
  /// Resumes waiters whose edits (and requested checkpoint) are durable.
  private func settle(checkpointed: Bool) {
    waiters.removeAll { waiter in
      guard waiter.target <= savedEdits, !waiter.checkpoint || checkpointed else { return false }
      waiter.continuation.resume()
      return true
    }
  }
  private func fail(_ failure: SaveFailure, upTo target: Int) {
    publishStatus("save-failed", failure, sequence)
    waiters.removeAll { waiter in
      guard waiter.target <= target else { return false }
      waiter.continuation.resume(throwing: failure)
      return true
    }
  }
  /// Waits until every edit accepted before the call is durable.
  private func write(checkpoint: Bool) async throws {
    try await withCheckedThrowingContinuation { (continuation: CheckedContinuation<Void, Error>) in
      queue.async {
        guard !self.closed else { return continuation.resume(throwing: failure("Document owner is closed")) }
        guard !self.invalidated else { return continuation.resume(throwing: failure("Owner invalidated; explicit recovery is required")) }
        self.waiters.append(Waiter(target: self.edits, checkpoint: checkpoint, continuation: continuation))
        self.pump()
      }
    }
  }
  public func flush() async throws { try await write(checkpoint: false) }
  public func compact() async throws {
    guard storage.mode == .document else { throw failure("Read-only capture cannot compact") }
    try await write(checkpoint: true)
  }
  public func detachRenderer() async throws { try await enqueue { try self.core.detachRenderer() } }

  /// Drops unsaved edits and reloads saved state. The epoch fences a write in flight: it
  /// finishes on the persistence queue before the reload reads, and its reply is ignored.
  public func discardPending() async throws {
    try await enqueue(allowInvalidated: true) {
      self.writeEpoch += 1
      self.discarding = true
      self.autosave?.cancel()
      self.fail(.io("Unsaved edits were discarded"), upTo: .max)
    }
    do {
      let schemaKey = schemaKey, storage = storage
      let restored = try await persist { try Self.restore(storage.load(), schemaKey: schemaKey, storage: storage) }
      try await enqueue(allowInvalidated: true) {
        self.core = restored.core
        self.invalidated = false
        self.generation = restored.generation
        self.stored = restored.stored
        self.uncertainWrite = nil
        self.writing = false
        self.savedVersion = try restored.core.version()
        let frame = try JSONSerialization.jsonObject(with: Data(restored.core.snapshot().utf8)) as! [String: Any]
        self.session = frame["session"] as! String
        self.epoch = UUID().uuidString
        self.sequence = frame["sequence"] as! Int
        self.savedEdits = self.edits
        self.discarding = false
        self.publishStatus("saved", nil, self.sequence)
        self.pump()
      }
    } catch {
      queue.async { self.discarding = false }
      throw error
    }
  }
  // MARK: Host-owned attachments and theme. Page and socket calls go through the owner,
  // so they honor its closed and closing guards.

  private struct Unchecked<T>: @unchecked Sendable { let value: T }
  private func requireWritable() async throws {
    try await enqueue { guard !self.closing else { throw failure("Document is closing") } }
  }
  func listAttachments() async throws -> [[String: Any]] {
    let storage = storage
    return try await persist { Unchecked(value: try storage.listAttachments()) }.value
  }
  func readAttachment(_ id: String) async throws -> Data {
    let storage = storage
    return try await persist { try storage.readAttachment(id) }
  }
  func putAttachment(_ bytes: Data) async throws -> [String: Any] {
    try await requireWritable()
    let storage = storage
    return try await persist { Unchecked(value: try storage.putAttachment(bytes)) }.value
  }
  func loadTheme() async throws -> [String: String] {
    let storage = storage
    return try await persist { try storage.loadTheme() }
  }
  /// The package's theme tokens and their default values.
  func themeDefaults() throws -> [String: String] {
    let bytes = try SlopFile.read(package.rootURL.appendingPathComponent("assets/theme.json"), within: package.rootURL, maximumBytes: 65536)
    guard let defaults = try JSONSerialization.jsonObject(with: bytes) as? [String: String] else {
      throw failure("Invalid theme defaults")
    }
    return defaults
  }
  /// Replaces the theme overrides. The only theme validator: page and CLI writes both
  /// land here, so the same values are accepted whether or not a window is open.
  func saveTheme(_ values: [String: String]) async throws {
    try ThemeRules.validate(values, defaults: themeDefaults())
    try await requireWritable()
    let storage = storage
    try await persist { try storage.saveTheme(values) }
  }
  /// Refuses new edits, writes everything accepted, then releases the lock. A failed
  /// final write keeps ownership and the live state so the window can retry.
  public func close() async throws {
    try await enqueue {
      self.closing = true
      self.autosave?.cancel()
    }
    do { try await flush() } catch {
      queue.async { self.closing = false }
      throw error
    }
    let storage = storage
    try await persist { storage.close() }
    try await enqueue(allowInvalidated: true) { self.closed = true }
  }
}

/// Theme override rules, enforced on writes only: loading never fails on theme, and the
/// browser ignores CSS it cannot parse.
enum ThemeRules {
  static let maximumValue = 4096
  static func validate(_ overrides: [String: String], defaults: [String: String]) throws {
    let reference = try NSRegularExpression(pattern: #"var\(\s*--slop-([a-zA-Z0-9-]+)"#)
    for (key, value) in overrides {
      guard defaults[key] != nil else { throw failure("Unknown theme token: \(key)") }
      guard !value.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty, value.utf16.count <= maximumValue,
        value.rangeOfCharacter(from: CharacterSet(charactersIn: "{};")) == nil
      else { throw failure("Invalid theme value: \(key)") }
      for match in reference.matches(in: value, range: NSRange(value.startIndex..., in: value)) {
        guard let range = Range(match.range(at: 1), in: value), defaults[String(value[range])] != nil else {
          throw failure("Unknown theme reference in \(key)")
        }
      }
    }
  }
}

extension DocumentOwner {
  /// The embedded core's build ID; a release checks the app and helper report the same one.
  public static var coreBuildID: String { coreBuildId() }
}

extension Error {
  /// The core or owner refuses every call until saved state is reloaded explicitly.
  var isOwnerInvalidation: Bool {
    if case CoreError.Invalidated? = self as? CoreError { return true }
    return localizedDescription.contains("Owner invalidated")
  }
}
