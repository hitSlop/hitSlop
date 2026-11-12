import Foundation
import PopCore
import PopCoreBinding
import Synchronization

/// Whether Edit ▸ Undo and Redo have anything to do.
public struct UndoState: Sendable, Equatable {
  public let canUndo: Bool
  public let canRedo: Bool
  public init(canUndo: Bool, canRedo: Bool) {
    self.canUndo = canUndo
    self.canRedo = canRedo
  }
}

/// The Apple façade of the shared Rust owner. Admission, sequencing, autosave, persistence
/// and the writer lease live in Rust; Swift delivers events and hosts native services.
public final class DocumentOwner: Sendable {
  public let file: PopFile
  let assets: ResourceReader
  let native: NativeOwner
  public let documentID: String?
  private let events: Events

  /// What the owner tells its document, in the owner's order, on the owner's queue.
  enum Event: Sendable {
    case publication(String)
    case themeChanged
    case undo(UndoState)
    case saveStatus(DocumentSaveStatus, Pressure, Bool)
  }
  /// Receives the owner's events. A new listener first receives current undo and save state.
  var listener: (@Sendable (Event) -> Void)? {
    get { events.facts.withLock { $0.listener } }
    set {
      let facts = events.facts.withLock { facts in
        facts.listener = newValue
        return facts
      }
      newValue?(.undo(facts.undo))
      newValue?(.saveStatus(facts.saveStatus, facts.pressure, facts.shared))
    }
  }
  /// The latest save's failure, until a save succeeds.
  var saveFailure: OwnerFailure? { events.facts.withLock { $0.failure } }
  /// Whether this session published any change, so its document's artwork may be out of date.
  var edited: Bool { events.facts.withLock { $0.published } }

  /// A `.snapshot` owner reads the saved document without the writer lock and never writes it.
  public init(url: URL, mode: StoreMode = .document) throws {
    let events = Events()
    let owner = try storeCall {
      try PopFile.opening {
        try NativeOwner.open(path: url.path, mode: mode, evaluatorPath: Self.enginePath, listener: events)
      }
    }
    native = owner
    self.events = events
    let opened = owner.app()
    documentID = opened.documentId
    file = try PopFile(url: url, opened: opened)
    assets = try storeCall { try owner.resourceReader() }
    if mode == .document { try PopLibrary.register(owner: owner) }
  }

  /// The app and its renderer ship this sibling helper. Missing helpers refuse commands;
  /// they never cause authored code to execute in the page or native process.
  public static var enginePath: String? {
    #if os(iOS)
      return nil
    #else
      #if DEBUG
        if let path = ProcessInfo.processInfo.environment["POP_HOST_ENGINE"] { return path }
      #endif
      let candidates = [
        Bundle.main.bundleURL.appendingPathComponent("Contents/Helpers/pop-engine"),
        Bundle.main.executableURL?.deletingLastPathComponent().appendingPathComponent("pop-engine"),
      ].compactMap { $0 }
      return candidates.first { FileManager.default.isExecutableFile(atPath: $0.path) }?.path
    #endif
  }

  /// What the owner has said, so nothing asks it, and who hears it next.
  private struct Facts {
    var undo = UndoState(canUndo: false, canRedo: false)
    var failure: OwnerFailure?
    var saveStatus: DocumentSaveStatus = .saved
    var pressure: Pressure = .ok
    var shared = false
    var published = false
    var listener: (@Sendable (Event) -> Void)?
  }

  private final class Events: OwnerListener {
    let facts = Mutex(Facts())
    func event(event: OwnerEvent) {
      let (listener, forwarded): ((@Sendable (Event) -> Void)?, Event?) = facts.withLock { facts in
        switch event {
        case .publication(let json):
          facts.published = true
          return (facts.listener, .publication(json))
        case .themeChanged:
          return (facts.listener, .themeChanged)
        case .undoState(let canUndo, let canRedo):
          let state = UndoState(canUndo: canUndo, canRedo: canRedo)
          facts.undo = state
          return (facts.listener, .undo(state))
        case .saveStatus(let status, let failure, let pressure, let shared):
          let failed =
            status == .failed
            ? failure ?? OwnerFailure(kind: .saveFailed, message: "Saving failed", reason: nil, opIndex: nil, path: [])
            : nil
          facts.failure = failed
          let saveStatus: DocumentSaveStatus =
            if let failed { .failed(SaveFailure(failed)) } else if status == .saving { .saving } else { .saved }
          facts.saveStatus = saveStatus
          facts.pressure = pressure
          facts.shared = shared
          return (facts.listener, .saveStatus(saveStatus, pressure, shared))
        }
      }
      if let forwarded {
        listener?(forwarded)
        if case .saveStatus = forwarded {
          NotificationCenter.default.post(name: .popDocumentSaveStatusChanged, object: nil)
        }
      }
    }
  }
  private func unit(_ start: (any OwnerUnitCompletion) -> Void) async throws {
    try await withCheckedThrowingContinuation { done in
      start(UnitAnswer { done.resume(with: $0) })
    }
  }
  /// Integration probes for saved page edits and native save recovery, including iPhone lifecycle tests.
  func state() async throws -> String {
    try await withCheckedThrowingContinuation { done in
      native.state(completion: StringAnswer { done.resume(with: $0) })
    }
  }
  struct Applied: Sendable {
    let sequence: Int
    let ids: [String]
  }
  func apply(batch: String, view: String? = nil, origin: EditOrigin = .agent) async throws -> Applied {
    let value: OwnerApplied = try await withCheckedThrowingContinuation { done in
      native.apply(batch: batch, origin: origin, view: view, completion: AppliedAnswer { done.resume(with: $0) })
    }
    return Applied(sequence: Int(value.sequence), ids: value.ids)
  }
  public func undo() async throws -> Int {
    let value: OwnerApplied = try await withCheckedThrowingContinuation { done in
      native.undo(completion: AppliedAnswer { done.resume(with: $0) })
    }
    return Int(value.sequence)
  }
  public func redo() async throws -> Int {
    let value: OwnerApplied = try await withCheckedThrowingContinuation { done in
      native.redo(completion: AppliedAnswer { done.resume(with: $0) })
    }
    return Int(value.sequence)
  }
  func attach(view: String) { native.attach(view: view) }
  /// One document request from the page `view`, as JSON. The owner checks and answers it;
  /// `reply` gets the page's reply, and the owner's failure when it refused.
  func page(json: String, view: String, reply: @escaping @Sendable (PageDispatch) -> Void) {
    native.page(json: json, view: view, completion: PageAnswer(reply))
  }
  private final class PageAnswer: PageCompletion {
    let reply: @Sendable (PageDispatch) -> Void
    init(_ reply: @escaping @Sendable (PageDispatch) -> Void) { self.reply = reply }
    func complete(reply: PageDispatch) { self.reply(reply) }
  }
  public func flush() async throws {
    try await unit { native.flush(completion: $0) }
  }
  public func discardPending() async throws { try await unit { native.discard(completion: $0) } }
  func startServer(page: any NativePageBarrier) throws -> NativeSocketServer {
    try storeCall { try NativeSocketServer.start(owner: native, page: page) }
  }
  /// The accepted live state at `destination` as a document of its own, without saving
  /// the source history first: its current state
  /// without history, the attachments that state references, and `artwork` (none when
  /// nil) in place of the original's.
  func copy(to destination: URL, artwork: PopRenderedArtwork?) async throws {
    try await unit {
      native.copy(destination: destination.path, preview: artwork?.preview, icon: artwork?.icon, completion: $0)
    }
  }
  /// The saved document at `destination` as stored, without syncing: a capture's source,
  /// rendered once and then deleted.
  func captureSource(to destination: URL) async throws {
    try await unit { native.captureSource(destination: destination.path, completion: $0) }
  }
  func exportFile(to destination: URL) async throws {
    try await unit { native.backup(destination: destination.path, completion: $0) }
  }
  func artwork(_ name: PopArtwork.Name) async throws -> Data? {
    try await withCheckedThrowingContinuation { done in
      native.artwork(name: name, completion: BytesAnswer { done.resume(with: $0) })
    }
  }
  struct ThemeRead: Sendable {
    let state: ThemeState
    let revision: Int
  }
  func loadTheme() async throws -> ThemeRead {
    let value: OwnerTheme = try await withCheckedThrowingContinuation { done in
      native.theme(completion: ThemeAnswer { done.resume(with: $0) })
    }
    return ThemeRead(state: value.state, revision: Int(value.sequence))
  }
  /// A theme panel change, as the window's own batch; `reply` has the sequence it was
  /// accepted at.
  func enqueueTheme(_ change: PopThemeChange, reply: @escaping @Sendable (Result<Int, Error>) -> Void) {
    let change: ThemeChange =
      switch change {
      case .set(let values): .set(values: values)
      case .resetAll: .resetAll
      case .importFile(let file): .importFile(file: file)
      }
    native.changeTheme(
      change: change,
      completion: AppliedAnswer { result in
        reply(result.map { Int($0.sequence) })
      })
  }

  func exportTheme() async throws -> String {
    try await withCheckedThrowingContinuation { done in
      native.exportTheme(completion: StringAnswer { done.resume(with: $0) })
    }
  }
  func installArtwork(_ artwork: CapturedArtwork) async throws {
    if try await PopLibrary.publishArtwork(
      owner: native, sourceVersion: artwork.sourceVersion, preview: artwork.preview, icon: artwork.icon)
    {
      return
    }
    try await unit {
      native.setArtwork(
        sourceVersion: artwork.sourceVersion, preview: artwork.preview, icon: artwork.icon, completion: $0)
    }
  }
  public func close(artwork: PopRenderedArtwork? = nil) async throws {
    try await unit { native.close(preview: artwork?.preview, icon: artwork?.icon, completion: $0) }
  }
  /// Moves a document that cannot save to a new document at `destination`: the owner
  /// writes, checks and publishes the copy of its live state, never over an existing file,
  /// then releases the writer lock without saving. After a failure it stays open with every
  /// edit and `destination` untouched. The session fences edits first.
  func handOff(to destination: URL, artwork: PopRenderedArtwork?) async throws {
    try await unit {
      native.handoff(dest: destination.path, preview: artwork?.preview, icon: artwork?.icon, completion: $0)
    }
  }
  public static var coreBuildID: String { coreBuildId() }
}

private final class UnitAnswer: OwnerUnitCompletion {
  let reply: @Sendable (Result<Void, Error>) -> Void
  init(_ reply: @escaping @Sendable (Result<Void, Error>) -> Void) { self.reply = reply }
  func complete() { reply(.success(())) }
  func failed(failure: OwnerFailure) { reply(.failure(failure)) }
}

private final class AppliedAnswer: OwnerAppliedCompletion {
  let reply: @Sendable (Result<OwnerApplied, Error>) -> Void
  init(_ reply: @escaping @Sendable (Result<OwnerApplied, Error>) -> Void) { self.reply = reply }
  func complete(value: OwnerApplied) { reply(.success(value)) }
  func failed(failure: OwnerFailure) { reply(.failure(failure)) }
}

private final class ThemeAnswer: OwnerThemeCompletion {
  let reply: @Sendable (Result<OwnerTheme, Error>) -> Void
  init(_ reply: @escaping @Sendable (Result<OwnerTheme, Error>) -> Void) { self.reply = reply }
  func complete(value: OwnerTheme) { reply(.success(value)) }
  func failed(failure: OwnerFailure) { reply(.failure(failure)) }
}

private final class BytesAnswer: OwnerBytesCompletion {
  let reply: @Sendable (Result<Data?, Error>) -> Void
  init(_ reply: @escaping @Sendable (Result<Data?, Error>) -> Void) { self.reply = reply }
  func complete(value: Data?) { reply(.success(value)) }
  func failed(failure: OwnerFailure) { reply(.failure(failure)) }
}

private final class StringAnswer: OwnerStringCompletion {
  let reply: @Sendable (Result<String, Error>) -> Void
  init(_ reply: @escaping @Sendable (Result<String, Error>) -> Void) { self.reply = reply }
  func complete(value: String) { reply(.success(value)) }
  func failed(failure: OwnerFailure) { reply(.failure(failure)) }
}

/// Finder and Quick Look artwork rendered from the saved document.
public struct PopRenderedArtwork: Sendable {
  public let preview: Data?
  public let icon: Data?
  public init(preview: Data?, icon: Data?) {
    self.preview = preview
    self.icon = icon
  }
}
