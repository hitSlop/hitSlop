import Foundation
import HitSlopCore
import HitSlopCoreBinding

/// The Apple façade of the shared Rust owner. Admission, sequencing, autosave, persistence
/// and the writer lease live in Rust; Swift delivers events and hosts native services.
public final class DocumentOwner: @unchecked Sendable {
  public let file: SlopFile
  let mode: StoreMode
  let assets: AssetReader
  private let native: NativeOwner
  private let events: Events
  public var epoch: String { native.epoch() }

  private let callbacksLock = NSLock()
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
  var onUndoState: (@Sendable (UndoState) -> Void)? {
    get { callbacksLock.withLock { undoCallback } }
    set { callbacksLock.withLock { undoCallback = newValue } }
  }

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
      case .publication(let json): owner.onPublication?(json)
      case .themeChanged: owner.onTheme?()
      case .undoState(let canUndo, let canRedo): owner.onUndoState?(UndoState(canUndo: canUndo, canRedo: canRedo))
      case .saveStatus(let status, let failure):
        switch status {
        case .saved: owner.onSaveStatus?(.saved)
        case .saving: owner.onSaveStatus?(.saving)
        case .failed: owner.onSaveStatus?(.failed(failure.map { Self.saveFailure($0) } ?? .io("Saving failed")))
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
  private func submit(_ request: OwnerRequest, epoch: String? = nil, view: String? = nil,
    reply: @escaping @Sendable (Result<OwnerReply, Error>) -> Void
  ) {
    native.submit(request: request, epoch: epoch, view: view, completion: Completion { result in
      if case .failed(let failure) = result { reply(.failure(Self.error(failure))) }
      else { reply(.success(result)) }
    })
  }
  private func call(_ request: OwnerRequest, epoch: String? = nil, view: String? = nil) async throws -> OwnerReply {
    try await withCheckedThrowingContinuation { done in
      submit(request, epoch: epoch, view: view) { done.resume(with: $0) }
    }
  }
  private func unit(_ request: OwnerRequest, epoch: String? = nil) async throws {
    guard case .unit = try await call(request, epoch: epoch) else { throw SlopFailure("Invalid owner response") }
  }
  public func state() async throws -> String {
    guard case .state(let json) = try await call(.state) else { throw SlopFailure("Invalid state response") }
    return json
  }
  public struct Applied: Sendable { public let sequence: Int; public let ids: [String] }
  public func apply(batch: String, epoch: String? = nil, view: String? = nil, origin: EditOrigin = .agent) async throws -> Applied {
    guard case .applied(let sequence, let ids) = try await call(.apply(batchJson: batch, origin: origin), epoch: epoch, view: view)
    else { throw SlopFailure("Invalid apply response") }
    return Applied(sequence: Int(sequence), ids: ids)
  }
  public func undo(redo: Bool = false) async throws -> Int {
    guard case .applied(let sequence, _) = try await call(.undo(redo: redo)) else { throw SlopFailure("Invalid undo response") }
    return Int(sequence)
  }
  func attach(view: String) { native.attach(view: view) }
  func publishUndoState() { submit(.publishUndoState) { _ in } }
  func enqueuePage(_ request: PageRequest, view: String,
    reply: @escaping @Sendable (sending Result<PageResult, Error>) -> Void
  ) {
    let operation: OwnerRequest
    switch request {
    case .open: operation = .state
    case .apply(let r): operation = .apply(batchJson: r.batch, origin: .page)
    case .text(let r): operation = .text(requestJson: r.request)
    case .undo: operation = .undo(redo: false)
    case .redo: operation = .undo(redo: true)
    case .flush: operation = .flush
    default: return reply(.failure(OwnerError.rejected("Not a document request")))
    }
    submit(operation, view: view) { result in
      reply(result.flatMap { result in
        switch (request, result) {
        case (.open, .state(let json)): return .success(.open(.init(state: json)))
        case (.apply, .applied(let sequence, let ids)): return .success(.apply(.init(sequence: Int(sequence), ids: ids)))
        case (.text, .text(let sequence, let authored, let start, let end)):
          return .success(.text(.init(sequence: Int(sequence), authored: authored, selectionStart: Int(start), selectionEnd: Int(end))))
        case (.undo, .applied(let sequence, _)): return .success(.undo(.init(sequence: Int(sequence))))
        case (.redo, .applied(let sequence, _)): return .success(.redo(.init(sequence: Int(sequence))))
        case (.flush, .unit): return .success(.flush)
        default: return .failure(SlopFailure("Invalid page response"))
        }
      })
    }
  }
  func currentSaveFailure() async throws -> SaveFailure? {
    guard case .failureState(let failure) = try await call(.saveFailure) else { throw SlopFailure("Invalid save status") }
    return failure.map { Self.saveFailure($0) }
  }
  public func flush() async throws {
    do { try await unit(.flush) } catch let error as SlopFailure { throw SaveFailure(error) }
  }
  public func discardPending() async throws { try await unit(.discard) }
  func edited() async -> Bool {
    guard case .bool(let value) = try? await call(.edited) else { return false }
    return value
  }
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
  func copy(to destination: URL) async throws { try await unit(.copy(destination: destination.path)) }
  func artwork(_ name: SlopArtwork.Name) async throws -> Data? {
    guard case .bytes(let bytes) = try await call(.artwork(name: name.rawValue)) else { throw SlopFailure("Invalid artwork response") }
    return bytes
  }
  func listAttachments() async throws -> [PageAttachmentsPutResult] {
    guard case .attachments(let items) = try await call(.attachments) else { throw SlopFailure("Invalid attachments response") }
    return items.map { .init(id: $0.id, byteLength: Int($0.byteLength)) }
  }
  func readAttachment(_ id: String) async throws -> String {
    guard case .bytes(let bytes?) = try await call(.readAttachment(id: id)) else { throw SlopFailure("Invalid attachment response") }
    return bytes.base64EncodedString()
  }
  func putAttachment(base64 encoded: String, epoch: String? = nil, view: String? = nil) async throws -> PageAttachmentsPutResult {
    guard let bytes = Data(base64Encoded: encoded) else { throw OwnerError.rejected("Invalid attachment bytes") }
    guard case .attachment(let item) = try await call(.putAttachment(bytes: bytes), epoch: epoch, view: view)
    else { throw SlopFailure("Invalid attachment response") }
    return .init(id: item.id, byteLength: Int(item.byteLength))
  }
  struct ThemeRead: Sendable { let state: ThemeState; let revision: Int }
  private static func themeRead(_ reply: OwnerReply) throws -> ThemeRead {
    guard case .theme(let state, let sequence) = reply else { throw SlopFailure("Invalid theme response") }
    return ThemeRead(state: state, revision: Int(sequence))
  }
  func loadTheme() async throws -> ThemeRead { try Self.themeRead(try await call(.theme(change: .get, gesture: false))) }
  /// An independent CLI theme command never joins a color-panel gesture.
  func applyTheme(_ change: ThemeChange, epoch: String? = nil) async throws -> ThemeRead {
    return try Self.themeRead(try await call(.theme(change: change, gesture: false), epoch: epoch))
  }
  func beginThemeGesture() { submit(.beginThemeGesture) { _ in } }
  func endThemeGesture() { submit(.endThemeGesture) { _ in } }
  func enqueueTheme(_ change: SlopThemeChange, reply: @escaping @Sendable (Result<ThemeRead, Error>) -> Void) {
    do {
      let operation: ThemeChange
      switch change {
      case .set(let values): operation = .set(valuesJson: String(decoding: try JSONSerialization.data(withJSONObject: values), as: UTF8.self))
      case .resetAll: operation = .reset(token: nil)
      case .importFile(let file): operation = .import(fileJson: file)
      }
      submit(.theme(change: operation, gesture: true)) { result in reply(result.flatMap { value in Result { try Self.themeRead(value) } }) }
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
