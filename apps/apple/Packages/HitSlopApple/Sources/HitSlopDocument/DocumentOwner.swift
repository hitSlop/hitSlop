import Foundation
import HitSlopCore
import HitSlopCoreBinding

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
public final class DocumentOwner: @unchecked Sendable {
  public let file: SlopFile
  let mode: StoreMode
  let assets: AssetReader
  private let native: NativeOwner
  private let events: Events

  private let callbacksLock = NSLock()
  /// What the owner has told this façade, so nothing asks it: the latest undo state and
  /// save failure, and whether any change was published since open.
  private var undo = UndoState(canUndo: false, canRedo: false)
  private var failure: SaveFailure?
  private var published = false
  private var publicationCallback: (@Sendable (String) -> Void)?
  private var statusCallback: (@Sendable (DocumentSaveStatus) -> Void)?
  private var themeCallback: (@Sendable () -> Void)?
  private var undoCallback: (@Sendable (UndoState) -> Void)?
  var onPublication: (@Sendable (String) -> Void)? {
    get { callbacksLock.withLock { publicationCallback } }
    set { callbacksLock.withLock { publicationCallback = newValue } }
  }
  var onSaveStatus: (@Sendable (DocumentSaveStatus) -> Void)? {
    get { callbacksLock.withLock { statusCallback } }
    set { callbacksLock.withLock { statusCallback = newValue } }
  }
  var onTheme: (@Sendable () -> Void)? {
    get { callbacksLock.withLock { themeCallback } }
    set { callbacksLock.withLock { themeCallback = newValue } }
  }
  /// Receives the current undo state at once, then each change.
  var onUndoState: (@Sendable (UndoState) -> Void)? {
    get { callbacksLock.withLock { undoCallback } }
    set {
      let current = callbacksLock.withLock { undoCallback = newValue; return undo }
      newValue?(current)
    }
  }
  /// The latest save's failure, until a save succeeds.
  var saveFailure: SaveFailure? { callbacksLock.withLock { failure } }
  /// Whether this session published any change, so its document's artwork may be out of date.
  var edited: Bool { callbacksLock.withLock { published } }
  private func record(_ change: (DocumentOwner) -> Void) { callbacksLock.withLock { change(self) } }

  public init(url: URL, mode: StoreMode = .document) throws {
    self.mode = mode
    let events = Events()
    let owner = try storeCall { try SlopFile.opening { try NativeOwner.open(path: url.path, mode: mode, listener: events) } }
    native = owner
    self.events = events
    file = try SlopFile(url: url, opened: owner.app())
    assets = try storeCall { try owner.assetReader() }
    events.owner = self
  }

  private final class Events: OwnerListener, @unchecked Sendable {
    weak var owner: DocumentOwner?
    func event(event: OwnerEvent) {
      guard let owner else { return }
      switch event {
      case .publication(let json):
        owner.record { $0.published = true }
        owner.onPublication?(json)
      case .themeChanged: owner.onTheme?()
      case .undoState(let canUndo, let canRedo):
        // The owner states its undo state when it starts, which a listener that set
        // `onUndoState` already received; only changes are forwarded.
        let state = UndoState(canUndo: canUndo, canRedo: canRedo)
        var changed = false
        owner.record { changed = $0.undo != state; $0.undo = state }
        if changed { owner.onUndoState?(state) }
      case .saveStatus(let status, let failure):
        let failed = status == .failed ? failure.map { Self.saveFailure($0) } ?? .io("Saving failed") : nil
        owner.record { $0.failure = failed }
        switch status {
        case .saved: owner.onSaveStatus?(.saved)
        case .saving: owner.onSaveStatus?(.saving)
        case .failed: owner.onSaveStatus?(.failed(failed ?? .io("Saving failed")))
        }
      }
    }
    private static func saveFailure(_ failure: OwnerFailure) -> SaveFailure { DocumentOwner.saveFailure(failure) }
  }
  private final class Completion: OwnerCompletion, @unchecked Sendable {
    let reply: @Sendable (OwnerReply) -> Void
    init(_ reply: @escaping @Sendable (OwnerReply) -> Void) { self.reply = reply }
    func complete(reply: OwnerReply) { self.reply(reply) }
  }
  private static func error(_ failure: OwnerFailure) -> Error {
    switch failure.kind {
    case .replaced: return OwnerReplaced()
    case .closing: return OwnerError.closing
    case .closed: return OwnerError.closed
    case .readOnly: return OwnerError.readOnly
    case .invalidated: return CoreError.Invalidated(message: failure.message)
    case .locked: return DocumentLocked()
    case .busy: return SaveFailure.busy
    case .full: return SaveFailure.full
    case .moved: return SaveFailure.moved
    case .rejected:
      return CoreError.Rejected(code: failure.reason ?? "invalid_request", message: failure.message, opIndex: failure.opIndex)
    case .saveFailed: return SaveFailure.io(failure.message)
    case .failed: return SlopFailure(failure.message)
    }
  }
  private static func saveFailure(_ failure: OwnerFailure) -> SaveFailure {
    failure.kind == .invalidated ? .invalidated : SaveFailure(error(failure))
  }
  /// `submit` admits synchronously, preserving bridge and panel arrival order. The callback
  /// runs after the Rust worker completes; no Swift task is involved in admission.
  private func submit(_ request: OwnerRequest, view: String? = nil,
    reply: @escaping @Sendable (Result<OwnerReply, Error>) -> Void
  ) {
    native.submit(request: request, view: view, completion: Completion { result in
      if case .failed(let failure) = result { reply(.failure(Self.error(failure))) }
      else { reply(.success(result)) }
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
  public struct Applied: Sendable { public let sequence: Int; public let ids: [String] }
  public func apply(batch: String, view: String? = nil, origin: EditOrigin = .agent) async throws -> Applied {
    guard case .applied(let sequence, let ids) = try await call(.apply(batchJson: batch, origin: origin), view: view)
    else { throw SlopFailure("Invalid apply response") }
    return Applied(sequence: Int(sequence), ids: ids)
  }
  public func undo(redo: Bool = false) async throws -> Int {
    guard case .applied(let sequence, _) = try await call(.undo(redo: redo)) else { throw SlopFailure("Invalid undo response") }
    return Int(sequence)
  }
  func attach(view: String) { native.attach(view: view) }
  /// One document request from the page `view`, as JSON. The owner checks and answers it;
  /// `reply` gets the page's reply, and the owner's refusal when it refused.
  func page(json: String, view: String, reply: @escaping @Sendable (String, Error?) -> Void) {
    native.page(json: json, view: view, completion: PageAnswer { answer, failure in reply(answer, failure.map(Self.error)) })
  }
  private final class PageAnswer: PageCompletion, @unchecked Sendable {
    let reply: @Sendable (String, OwnerFailure?) -> Void
    init(_ reply: @escaping @Sendable (String, OwnerFailure?) -> Void) { self.reply = reply }
    func complete(replyJson: String, failure: OwnerFailure?) { reply(replyJson, failure) }
  }
  public func flush() async throws {
    do { try await unit(.flush) } catch let error as SlopFailure { throw SaveFailure(error) }
  }
  public func discardPending() async throws { try await unit(.discard) }
  func startServer(exporter: any NativeExportHandler) throws -> NativeSocketServer {
    try storeCall { try NativeSocketServer.start(owner: native, exporter: exporter) }
  }
  func request(_ request: SocketRequest) async -> Data {
    guard let bytes = try? JSONSerialization.data(withJSONObject: request.json, options: .withoutEscapingSlashes)
    else { return RequestOutcome.socket(OwnerError.rejected("Invalid document request")).encoded() }
    return await withCheckedContinuation { done in
      native.request(json: String(decoding: bytes, as: UTF8.self), completion: CommandCompletion { done.resume(returning: $0) })
    }
  }
  /// The saved document at `destination`: `durable` for a copy a person keeps, not for a
  /// capture's disposable source.
  func copy(to destination: URL, durable: Bool = true) async throws {
    try await unit(.copy(destination: destination.path, durable: durable))
  }
  func artwork(_ name: SlopArtwork.Name) async throws -> Data? {
    guard case .bytes(let bytes) = try await call(.artwork(name: name.rawValue)) else { throw SlopFailure("Invalid artwork response") }
    return bytes
  }
  func listAttachments() async throws -> [PageAttachmentsPutResult] {
    guard case .attachments(let items) = try await call(.attachments) else { throw SlopFailure("Invalid attachments response") }
    return items.map { .init(id: $0.id, byteLength: Int($0.byteLength)) }
  }
  struct ThemeRead: Sendable { let state: ThemeState; let revision: Int }
  private static func themeRead(_ reply: OwnerReply) throws -> ThemeRead {
    guard case .theme(let state, let sequence) = reply else { throw SlopFailure("Invalid theme response") }
    return ThemeRead(state: state, revision: Int(sequence))
  }
  func loadTheme() async throws -> ThemeRead { try Self.themeRead(try await call(.theme)) }
  /// A theme panel change, as the window's own batch; `reply` has the sequence it was
  /// accepted at.
  func enqueueTheme(_ change: SlopThemeChange, reply: @escaping @Sendable (Result<Int, Error>) -> Void) {
    do {
      let intent: [String: Any] = switch change {
      case .set(let values): ["type": "setTheme", "values": values]
      case .resetAll: ["type": "setTheme", "values": [String: String](), "replace": true]
      case .importFile(let file): ["type": "importTheme", "file": file]
      }
      let batch = String(decoding: try JSONSerialization.data(withJSONObject: ["intents": [intent]]), as: UTF8.self)
      submit(.apply(batchJson: batch, origin: .window)) { result in
        reply(result.flatMap { value in
          guard case .applied(let sequence, _) = value else { return .failure(SlopFailure("Invalid theme response")) }
          return .success(Int(sequence))
        })
      }
    } catch { reply(.failure(error)) }
  }
  func exportTheme() async throws -> String {
    guard case .state(let json) = try await call(.exportTheme) else { throw SlopFailure("Invalid theme export") }
    return json
  }
  public func close(artwork: SlopRenderedArtwork? = nil) async throws {
    do { try await unit(.close(preview: artwork?.preview, icon: artwork?.icon)) }
    catch let error as SlopFailure { throw SaveFailure(error) }
  }
  public static var coreBuildID: String { coreBuildId() }
}

/// Finder and Quick Look artwork rendered from the saved document.
public struct SlopRenderedArtwork: Sendable {
  public let preview: Data?
  public let icon: Data?
  public init(preview: Data?, icon: Data?) { self.preview = preview; self.icon = icon }
}
public struct OwnerReplaced: LocalizedError {
  public var errorDescription: String? { "owner_replaced: the document was reloaded; this edit was not applied" }
}
