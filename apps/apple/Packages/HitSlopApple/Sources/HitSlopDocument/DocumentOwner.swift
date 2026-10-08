import Foundation
import HitSlopCore
import HitSlopCoreBinding
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
  public let file: SlopFile
  let mode: StoreMode
  let assets: ResourceReader
  private let native: NativeOwner
  private let events: Events

  /// What the owner tells its document, in the owner's order, on the owner's queue.
  enum Event: Sendable {
    case publication(String)
    case themeChanged
    case undo(UndoState)
    case saveStatus(DocumentSaveStatus)
  }
  /// Receives the owner's events. A new listener first receives the current undo state.
  var listener: (@Sendable (Event) -> Void)? {
    get { events.facts.withLock { $0.listener } }
    set {
      let undo = events.facts.withLock { facts in
        facts.listener = newValue
        return facts.undo
      }
      newValue?(.undo(undo))
    }
  }
  /// The latest save's failure, until a save succeeds.
  var saveFailure: SaveFailure? { events.facts.withLock { $0.failure } }
  /// Whether this session published any change, so its document's artwork may be out of date.
  var edited: Bool { events.facts.withLock { $0.published } }

  public init(url: URL, mode: StoreMode = .document) throws {
    self.mode = mode
    let events = Events()
    let owner = try storeCall {
      try SlopFile.opening {
        try NativeOwner.open(path: url.path, mode: mode, evaluatorPath: Self.evaluatorPath, listener: events)
      }
    }
    native = owner
    self.events = events
    file = try SlopFile(url: url, opened: owner.app())
    assets = try storeCall { try owner.resourceReader() }
  }

  /// The app and its renderer ship this sibling helper. Missing helpers refuse commands;
  /// they never cause authored code to execute in the page or native process.
  private static var evaluatorPath: String? {
    #if DEBUG
      if let path = ProcessInfo.processInfo.environment["HITSLOP_EVALUATOR"] { return path }
    #endif
    let candidates = [
      Bundle.main.bundleURL.appendingPathComponent("Contents/Helpers/hitslop-evaluator"),
      Bundle.main.executableURL?.deletingLastPathComponent().appendingPathComponent("hitslop-evaluator"),
    ].compactMap { $0 }
    return candidates.first { FileManager.default.isExecutableFile(atPath: $0.path) }?.path
  }

  /// What the owner has said, so nothing asks it, and who hears it next.
  private struct Facts {
    var undo = UndoState(canUndo: false, canRedo: false)
    var failure: SaveFailure?
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
          // The owner states its undo state when it starts, which a listener received when
          // it was set; only changes are forwarded.
          let state = UndoState(canUndo: canUndo, canRedo: canRedo)
          guard state != facts.undo else { return (nil, nil) }
          facts.undo = state
          return (facts.listener, .undo(state))
        case .saveStatus(let status, let failure):
          let failed = status == .failed ? failure.map(\.saveFailure) ?? .io("Saving failed") : nil
          facts.failure = failed
          let saveStatus: DocumentSaveStatus =
            if let failed { .failed(failed) } else if status == .saving { .saving } else { .saved }
          return (facts.listener, .saveStatus(saveStatus))
        }
      }
      if let forwarded { listener?(forwarded) }
    }
  }
  private final class Completion: OwnerCompletion {
    let reply: @Sendable (OwnerReply) -> Void
    init(_ reply: @escaping @Sendable (OwnerReply) -> Void) { self.reply = reply }
    func complete(reply: OwnerReply) { self.reply(reply) }
  }
  /// `submit` admits synchronously, preserving bridge and panel arrival order. The callback
  /// runs after the Rust worker completes; no Swift task is involved in admission.
  private func submit(
    _ request: OwnerRequest, view: String? = nil,
    reply: @escaping @Sendable (Result<OwnerReply, Error>) -> Void
  ) {
    native.submit(
      request: request, view: view,
      completion: Completion { result in
        if case .failed(let failure) = result { reply(.failure(failure.hostError)) } else { reply(.success(result)) }
      })
  }
  private func call(_ request: OwnerRequest, view: String? = nil) async throws -> OwnerReply {
    try await withCheckedThrowingContinuation { done in
      submit(request, view: view) { done.resume(with: $0) }
    }
  }
  private func unit(_ request: OwnerRequest) async throws {
    guard case .unit = try await call(request) else { throw SlopFailure("Invalid owner response") }
  }
  public func state() async throws -> String {
    guard case .state(let json) = try await call(.state) else { throw SlopFailure("Invalid state response") }
    return json
  }
  public struct Applied: Sendable {
    public let sequence: Int
    public let ids: [String]
  }
  public func apply(batch: String, view: String? = nil, origin: EditOrigin = .agent) async throws -> Applied {
    guard case .applied(let sequence, let ids) = try await call(.apply(batch: batch, origin: origin), view: view)
    else { throw SlopFailure("Invalid apply response") }
    return Applied(sequence: Int(sequence), ids: ids)
  }
  public func undo(redo: Bool = false) async throws -> Int {
    guard case .applied(let sequence, _) = try await call(.undo(redo: redo)) else {
      throw SlopFailure("Invalid undo response")
    }
    return Int(sequence)
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
    do { try await unit(.flush) } catch let error as SlopFailure { throw SaveFailure(error) }
  }
  public func discardPending() async throws { try await unit(.discard) }
  func startServer(exporter: any NativeExportHandler) throws -> NativeSocketServer {
    try storeCall { try NativeSocketServer.start(owner: native, exporter: exporter) }
  }
  /// The saved document at `destination` as a document of its own: its current state
  /// without history, the attachments that state references, and `artwork` (none when
  /// nil) in place of the original's.
  func copy(to destination: URL, artwork: SlopRenderedArtwork?) async throws {
    try await unit(.copy(destination: destination.path, preview: artwork?.preview, icon: artwork?.icon))
  }
  /// The saved document at `destination` as stored, without syncing: a capture's source,
  /// rendered once and then deleted.
  func captureSource(to destination: URL) async throws {
    try await unit(.captureSource(destination: destination.path))
  }
  func artwork(_ name: SlopArtwork.Name) async throws -> Data? {
    guard case .bytes(let bytes) = try await call(.artwork(name: name)) else {
      throw SlopFailure("Invalid artwork response")
    }
    return bytes
  }
  struct ThemeRead: Sendable {
    let state: ThemeState
    let revision: Int
  }
  private static func themeRead(_ reply: OwnerReply) throws -> ThemeRead {
    guard case .theme(let state, let sequence) = reply else { throw SlopFailure("Invalid theme response") }
    return ThemeRead(state: state, revision: Int(sequence))
  }
  func loadTheme() async throws -> ThemeRead { try Self.themeRead(try await call(.theme)) }
  /// A theme panel change, as the window's own batch; `reply` has the sequence it was
  /// accepted at.
  func enqueueTheme(_ change: SlopThemeChange, reply: @escaping @Sendable (Result<Int, Error>) -> Void) {
    let native: ThemeChange =
      switch change {
      case .set(let values): .set(values: values)
      case .resetAll: .resetAll
      case .importFile(let file): .importFile(file: file)
      }
    submit(.changeTheme(change: native)) { result in
      reply(
        result.flatMap { value in
          guard case .applied(let sequence, _) = value else { return .failure(SlopFailure("Invalid theme response")) }
          return .success(Int(sequence))
        })
    }
  }

  func exportTheme() async throws -> String {
    guard case .themeFile(let json) = try await call(.exportTheme) else { throw SlopFailure("Invalid theme export") }
    return json
  }
  public func close(artwork: SlopRenderedArtwork? = nil) async throws {
    do { try await unit(.close(preview: artwork?.preview, icon: artwork?.icon)) } catch let error as SlopFailure {
      throw SaveFailure(error)
    }
  }
  public static var coreBuildID: String { coreBuildId() }
}

/// Finder and Quick Look artwork rendered from the saved document.
public struct SlopRenderedArtwork: Sendable {
  public let preview: Data?
  public let icon: Data?
  public init(preview: Data?, icon: Data?) {
    self.preview = preview
    self.icon = icon
  }
}
public struct OwnerReplaced: LocalizedError {
  public var errorDescription: String? { "owner_replaced: the document was reloaded; this edit was not applied" }
}
