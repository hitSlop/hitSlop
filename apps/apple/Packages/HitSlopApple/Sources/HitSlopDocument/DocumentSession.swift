import Foundation
import HitSlopCore
import HitSlopCoreBinding
@preconcurrency import WebKit

#if os(macOS)
  import AppKit
#else
  import UIKit
#endif

/// Whether a document's edits are durable. A failed save keeps the edits live and unsaved.
public enum DocumentSaveStatus: Sendable, Equatable {
  case saved, saving
  case failed(SaveFailure)
}

/// A document's writer lease outlives its renderer. The native Rust owner interprets Loro bytes.
@MainActor
public final class DocumentSession: NSObject, WKScriptMessageHandlerWithReply {
  private var liveWebView: WKWebView?
  /// Declarative hosts can redraw during close, after the renderer has been released.
  public var currentWebView: WKWebView? { liveWebView }
  public var webView: WKWebView {
    guard let liveWebView else { preconditionFailure("Document WebView has been destroyed") }
    return liveWebView
  }
  public let file: SlopFile
  private enum Renderer {
    case opening, ready
    case failed(String, terminated: Bool)
  }
  private enum Phase {
    case active(Renderer)
    case closing(Renderer, prepared: Bool)
    case closed
  }
  private var phase: Phase = .active(.opening)
  private var renderer: Renderer? {
    switch phase {
    case .active(let renderer), .closing(let renderer, _): return renderer
    case .closed: return nil
    }
  }
  /// Rendering remains available while close prepares and captures its saved source.
  public var isReady: Bool {
    if case .ready? = renderer { return true }
    return false
  }
  /// New page work is admitted only while the renderer is ready and the session active.
  public var acceptsPageWork: Bool {
    if case .active(.ready) = phase { return true }
    return false
  }
  var isActive: Bool {
    if case .active = phase { return true }
    return false
  }
  func isCurrent(_ view: WKWebView) -> Bool { view === liveWebView && !closed }
  public var rendererDead: Bool {
    if case .failed(_, terminated: true)? = renderer { return true }
    return false
  }
  private var openingError: String? {
    if case .failed(let message, _)? = renderer { return message }
    return nil
  }
  private var closed: Bool {
    if case .closed = phase { return true }
    return false
  }
  private var closing: Bool {
    get {
      if case .closing = phase { return true }
      return false
    }
    set {
      guard let renderer else { return }
      phase = newValue ? .closing(renderer, prepared: false) : .active(renderer)
    }
  }
  private var closePrepared: Bool {
    get {
      if case .closing(_, prepared: true) = phase { return true }
      return false
    }
    set {
      guard case .closing(let renderer, _) = phase else { return }
      phase = .closing(renderer, prepared: newValue)
    }
  }
  private var closeDrafts: [UnresolvedTextDraft] = []
  var pageCallTimeout = Timeouts.pageCall
  var readyTimeout = Timeouts.pageReady
  public var onExport: ((ExportFormat, URL, NativeCommandDeadline) async throws -> Void)?
  private(set) var capturing = false
  /// Captures queued behind the one in progress (`withCapture`).
  private var captureQueue: [CheckedContinuation<Void, Never>] = []
  /// The system panels, which tests replace with scripted presenters.
  var filePicker = DocumentFilePicker()
  var fileSaver = DocumentFileSaver() {
    didSet { configureFileSaver() }
  }
  /// Recognizes recordings on this Mac; tests script their own.
  var speechRecognizer: any SpeechRecognizing = SystemSpeechRecognizer()
  #if os(iOS)
    var hapticFeedback: any HapticPlaying = DocumentHaptics()
  #endif
  /// Transcriptions the attached page started (`serveSpeech`).
  var speechTasks: [UUID: Task<Void, Never>] = [:]
  /// The window that shows this session.
  public weak var delegate: DocumentSessionDelegate?
  /// Whether Edit ▸ Undo and Redo have anything to do in this document.
  public private(set) var undoAvailability = UndoState(canUndo: false, canRedo: false)
  public private(set) var retainsHistory = false
  public private(set) var storagePressure = Pressure.ok
  /// What a shared document tells its people when saved history nears or reaches the
  /// storage limit. An unshared copy has the same limit but keeps no shared history, so it
  /// frees room. Every view that mentions storage uses this sentence.
  public var storageNotice: String? {
    guard retainsHistory else { return nil }
    return switch storagePressure {
    case .ok: nil
    case .near:
      "This shared document is almost full. Save an unshared copy to free up room: it keeps your content without the shared history."
    case .full:
      "This shared document is full, so new edits cannot be saved. Save an unshared copy to keep editing: it keeps your content without the shared history."
    }
  }
  /// A shared document at its storage limit cannot save, so it leaves through
  /// `handOff(to:)` rather than a close.
  public var isFullShared: Bool { retainsHistory && storagePressure == .full }
  let owner: DocumentOwner
  public var documentID: String? { owner.documentID }
  public private(set) var sharing: DocumentSharing?
  private final class SharedSession {
    weak var value: DocumentSession?
    init(_ value: DocumentSession) { self.value = value }
  }
  private static var sharingSessions: [String: SharedSession] = [:]
  static func sharedSession(document: String) -> DocumentSession? { sharingSessions[document]?.value }
  private var server: NativeSocketServer?
  /// The attached page's token (see `makeWebView`).
  private(set) var view = UUID().uuidString
  nonisolated private let pushes = PushQueue()
  private var closeTask: Task<Void, Error>?
  private var waiters: [UUID: CheckedContinuation<Void, Error>] = [:]
  private let webViewResources: URL

  /// What opening reads off the main actor: the owner, with the app's assets, and the page
  /// shell.
  struct Prepared: Sendable {
    let shell: URL
    let owner: DocumentOwner

    init(url: URL) throws {
      shell = try DocumentSession.pageShell()
      owner = try DocumentOwner(url: url)
    }
  }

  /// The page shell bundled with this build; every document page loads it.
  nonisolated static func pageShell() throws -> URL {
    guard let url = Bundle.module.url(forResource: "shell", withExtension: nil),
      ["boot.js", "index.js"].allSatisfy({ FileManager.default.fileExists(atPath: url.appendingPathComponent($0).path) }
      )
    else { throw SlopFailure("Incomplete page shell; reinstall hitSlop") }
    return url
  }

  private init(prepared: Prepared) {
    file = prepared.owner.file
    webViewResources = prepared.shell
    owner = prepared.owner
    super.init()
    observeOwner()
    configureFileSaver()
    makeWebView()
    if let document = documentID {
      sharing = DocumentSharing(owner: owner)
      Self.sharingSessions[document] = SharedSession(self)
    }
  }
  private func configureFileSaver() {
    fileSaver.window = { [weak self] in self?.liveWebView?.window }
    fileSaver.onFailed = { [weak self] message in self?.report(SlopPageIssue(message: message, isOperation: true)) }
  }

  /// Publications enter `pushes` on the owner's queue, so the page gets them in owner order;
  /// undo and save status reach the main actor in that order too.
  private func observeOwner() {
    owner.listener = { [weak self] event in
      switch event {
      case .publication(let publication):
        self?.push(publication)
      case .themeChanged:
        // The ordered document publications restyle the page. This only refreshes the panel.
        Task { @MainActor [weak self] in
          guard let self, let theme = try? await self.currentTheme() else { return }
          self.delegate?.pageSession(self, themeChanged: theme)
        }
      case .undo(let state):
        DispatchQueue.main.async { self?.undoAvailability = state }
      case .saveStatus(let status, let pressure, let shared):
        DispatchQueue.main.async {
          guard let self else { return }
          self.retainsHistory = shared
          self.storagePressure = pressure
          self.delegate?.pageSession(self, saveStatus: status)
        }
      }
    }
  }

  // MARK: Theme panel

  /// The palette now; later changes arrive through `pageSession(_:themeChanged:)`.
  public func currentTheme() async throws -> SlopThemeState {
    SlopThemeState(try await owner.loadTheme())
  }
  /// Applies a panel change in the order changes are made. It is accepted in memory,
  /// restyles the page, and is saved like an edit. `reply` reports the owner's theme
  /// revision once the change is accepted (unchanged when it changed nothing), or the
  /// refusal.
  public func changeTheme(
    _ change: SlopThemeChange, reply: @escaping @MainActor (Result<Int, Error>) -> Void = { _ in }
  ) {
    guard !closing, !closed else { return reply(.failure(SlopFailure("Document is closing"))) }
    owner.enqueueTheme(change) { result in
      Task { @MainActor in reply(result) }
    }
  }
  /// The full palette as a theme file, the bytes the core writes for every export.
  public func exportTheme() async throws -> Data {
    Data(try await owner.exportTheme().utf8)
  }

  /// Called on the owner queue, so pushes enter `pushes` in owner order. One drain at a
  /// time delivers everything buffered in a single awaited call; Swift never parses them.
  nonisolated private func push(_ json: String) {
    guard pushes.append(json) else { return }
    Task { @MainActor [weak self] in await self?.drainPushes() }
  }
  private func drainPushes() async {
    var retryMS = 100
    var retryView = view
    while true {
      guard let batch = pushes.peek() else { return }
      // A not-yet-attached or retired page reads current state on its next open.
      guard !rendererDead, liveWebView != nil, batch.view == view else {
        pushes.acknowledge(batch)
        continue
      }
      do {
        try await callPage(.publish(payload: "[" + batch.items.joined(separator: ",") + "]"))
        pushes.acknowledge(batch)
        retryMS = 100
      } catch {
        pushes.failed(batch)
        if retryView != batch.view {
          retryView = batch.view
          retryMS = 100
        }
        try? await Task.sleep(for: .milliseconds(retryMS))
        retryMS = min(retryMS * 2, 2000)
        // A renderer that died reports through its termination path; only a live page
        // that could not take the changes is an issue.
        if !rendererDead, batch.view == view {
          report(
            SlopPageIssue(
              message: "Document changes could not reach the interface: \(error.localizedDescription)",
              isOperation: false))
        }
      }
    }
  }

  /// Opens the document at `url` as its owner.
  public static func open(url: URL) async throws -> DocumentSession {
    try await finishOpening(try await prepare(url: url))
  }

  private static func prepare(url: URL) async throws -> Prepared {
    try await SlopPreparation.run {
      // The owner's store opens and checks the file once; the session shows it from that.
      do { return try Prepared(url: SlopFile.documentRoot(url)) } catch let error as SlopError {
        throw SlopDiagnosticError(error, diagnostic: .init(.rejection, reason: .invalidFile))
      }
    }
  }

  static func finishOpening(_ prepared: Prepared) async throws -> DocumentSession {
    if Task.isCancelled {
      // No renderer has used the owner yet. Release the lease before reporting cancellation.
      try? await prepared.owner.close()
      throw CancellationError()
    }
    let session = DocumentSession(prepared: prepared)
    do { try session.startDiscovery() } catch {
      try? await prepared.owner.close()
      throw error
    }
    return session
  }

  public func reloadInterface() async throws {
    guard acceptsPageWork, !capturing else {
      throw SlopFailure("Document interface unavailable")
    }
    filePicker.cancel()
    fileSaver.cancel()
    cancelSpeech()
    // The remounted app starts without the old one's camera and microphone.
    await webView.setCameraCaptureState(.none)
    await webView.setMicrophoneCaptureState(.none)
    try await callPage(.reloadInterface)
  }
  private func makeWebView() {
    // Each page gets its own view token; requests from a replaced page are refused.
    closePrepared = false
    view = UUID().uuidString
    pushes.configure(view: view)
    owner.attach(view: view)
    let configuration = WKWebViewConfiguration()
    configuration.websiteDataStore = .nonPersistent()
    configuration.preferences.javaScriptCanOpenWindowsAutomatically = false
    #if os(iOS)
      configuration.ignoresViewportScaleLimits = false
    #endif
    configuration.setURLSchemeHandler(
      SchemeHandler(assets: owner.assets, shell: webViewResources), forURLScheme: "slop")
    // One handler: document requests go to the owner, everything else to the host bridge.
    configuration.userContentController.addScriptMessageHandler(self, contentWorld: .page, name: "hitslop")
    configuration.userContentController.addUserScript(
      WKUserScript(
        source: """
          (() => {
            const hideControls = () => document.documentElement.setAttribute('data-slop-controls', 'hidden');
            if (document.documentElement) hideControls();
            else document.addEventListener('DOMContentLoaded', hideControls, {once: true});
          })();
          // This listener survives a failure to load the shell itself.
          addEventListener('error', e => {
            const source = e.target?.src;
            if (typeof source !== 'string' || !source.startsWith('slop://app/__shell__/')) return;
            const error = ('Could not load page shell resource: ' + source).slice(0,\(Limits.errorText));
            webkit.messageHandlers.hitslop.postMessage(JSON.stringify({method:'failed',error})).catch(()=>{});
          }, true);
          """, injectionTime: .atDocumentStart, forMainFrameOnly: true))
    let spec = file
    let view = WKWebView(
      frame: CGRect(x: 0, y: 0, width: spec.width, height: spec.height),
      configuration: configuration)
    #if os(iOS)
      view.accessibilityIdentifier = "document-page"
      // The keyboard covers the page, which keeps its size; the shell reveals focused fields
      // inside the app's own scrollers. WebKit's own keyboard avoidance would pan the whole
      // page up, so the web view never hears about the keyboard.
      for name in [
        UIResponder.keyboardWillShowNotification, UIResponder.keyboardDidShowNotification,
        UIResponder.keyboardWillHideNotification, UIResponder.keyboardDidHideNotification,
        UIResponder.keyboardWillChangeFrameNotification, UIResponder.keyboardDidChangeFrameNotification,
      ] {
        NotificationCenter.default.removeObserver(view, name: name, object: nil)
      }
      view.scrollView.keyboardDismissMode = .interactive
      view.allowsLinkPreview = false
      view.scrollView.bounces = false
      view.scrollView.pinchGestureRecognizer?.isEnabled = false
    #endif
    WebViewBackground.set(file.backdrop == .window, on: view)
    view.navigationDelegate = self
    view.uiDelegate = self
    liveWebView = view
  }

  public func load() { liveWebView?.load(URLRequest(url: URL(string: "slop://app/")!)) }

  public func waitUntilReady(timeout: Duration? = nil) async throws {
    let timeout = timeout ?? readyTimeout
    if isReady { return }
    if closed || rendererDead || openingError != nil {
      throw SlopFailure(openingError ?? "Document page unavailable")
    }
    let id = UUID()
    let timer = Task { [weak self] in
      try await Task.sleep(for: timeout)
      guard let self, waiters[id] != nil else { return }
      failOpening("Document page did not become ready", reason: .unresponsive)
    }
    defer { timer.cancel() }
    try await withTaskCancellationHandler {
      try await withCheckedThrowingContinuation { continuation in
        waiters[id] = continuation
        if Task.isCancelled {
          waiters.removeValue(forKey: id)?.resume(throwing: CancellationError())
        }
      }
    } onCancel: {
      Task { @MainActor [weak self] in
        self?.waiters.removeValue(forKey: id)?.resume(throwing: CancellationError())
      }
    }
  }

  private func completeWaiters(_ result: Result<Void, Error>) {
    let pending = waiters.values
    waiters.removeAll()
    for waiter in pending { waiter.resume(with: result) }
  }

  public func userContentController(
    _ controller: WKUserContentController, didReceive message: WKScriptMessage,
    replyHandler: @escaping @MainActor @Sendable (Any?, String?) -> Void
  ) {
    let invalid = { replyHandler(pageFailure(OwnerFailure.rejected("Invalid page request")), nil) }
    guard !closed, message.webView === liveWebView, message.frameInfo.isMainFrame,
      message.frameInfo.securityOrigin.protocol == "slop",
      message.frameInfo.securityOrigin.host == "app",
      let json = message.body as? String
    else { return invalid() }
    servePage(json, reply: replyHandler)
  }

  /// Rust has already decoded, checked and admitted this action for the attached page.
  func serveHost(_ request: HostAction, reply: @escaping @MainActor @Sendable (Any?, String?) -> Void) {
    guard !closed else { return reply(pageFailure(OwnerFailure.closed), nil) }
    switch request {
    case .hapticsPlay(let feedback):
      #if os(iOS)
        if acceptsPageWork, !capturing, HapticPreferences.isEnabled,
          let view = liveWebView, let window = view.window, !window.isHidden,
          window.windowScene?.activationState == .foregroundActive
        {
          hapticFeedback.play(feedback, in: view)
        }
      #endif
      reply(encodeHostReply(reply: .hapticsPlay), nil)
    case .environment:
      #if os(macOS)
        let environment = HostEnvironment(platform: .macos, device: .desktop)
      #else
        let device: HostDevice
        switch UIDevice.current.userInterfaceIdiom {
        case .phone: device = .phone
        case .pad: device = .tablet
        case .mac: device = .desktop
        default: device = .unknown
        }
        let environment = HostEnvironment(platform: .ios, device: device)
      #endif
      reply(encodeHostReply(reply: .environment(environment: environment)), nil)
    case .windowResize(let width, let height):
      do {
        guard file.isResizable, let delegate, !closing, !closed
        else { throw SlopFailure("Window resizing unavailable") }
        let size = try delegate.pageSession(
          self, resizeContentTo: CGSize(width: CGFloat(width), height: CGFloat(height)))
        reply(encodeHostReply(reply: .windowResize(width: Double(size.width), height: Double(size.height))), nil)
      } catch { reply(pageFailure(error), nil) }
    case .ready:
      switch phase {
      case .active(.opening): phase = .active(.ready)
      // A close that began while the page was opening must not turn its readiness
      // into a renderer failure.
      case .closing(.opening, let prepared): phase = .closing(.ready, prepared: prepared)
      default:
        reply(pageFailure(OwnerFailure.rejected("Document page is not opening")), nil)
        return
      }
      completeWaiters(.success(()))
      delegate?.pageSessionDidBecomeReady(self)
      reply(encodeHostReply(reply: .ready), nil)
    case .pageRecovered:
      delegate?.pageSessionRecovered(self)
      reply(encodeHostReply(reply: .pageRecovered), nil)
    case .failed(let error):
      failOpening(error, reason: .startup, classification: .platform)
      reply(encodeHostReply(reply: .failed), nil)
    case .speechTranscribe(let attachment, let locale):
      serveSpeech(attachment: attachment, locale: locale, reply: reply)
    case .pageError(let kind, let error):
      if isReady {
        report(SlopPageIssue(message: error, isOperation: kind == .operation))
      } else {
        failOpening(error, reason: .authoredException, classification: .authored)
      }
      reply(encodeHostReply(reply: .pageError), nil)
    }
  }

  /// A page call may outlive its view; only the current view can fail this session.
  @discardableResult private func callPage(_ request: HostRequest) async throws -> String {
    guard let page = liveWebView else { throw SlopFailure("Document renderer unavailable") }
    let name: String
    switch request {
    case .publish: name = "publish"
    case .reloadInterface: name = "reloadInterface"
    case .flush: name = "flush"
    case .undo: name = "undo"
    case .redo: name = "redo"
    case .prepareClose: name = "prepareClose"
    case .cancelClose: name = "cancelClose"
    case .close: name = "close"
    }
    let timeout: Duration
    if case .reloadInterface = request { timeout = readyTimeout } else { timeout = pageCallTimeout }
    enum Outcome: Sendable {
      case settled(String)
      case expired
      case failed(Error)
    }
    let (outcomes, result) = AsyncStream<Outcome>.makeStream()
    page.callHost(request) { reply in
      switch reply {
      case .success(let value):
        if let json = value as? String {
          result.yield(.settled(json))
        } else {
          result.yield(.failed(SlopFailure("Invalid page reply")))
        }
      case .failure(let error): result.yield(.failed(error))
      }
    }
    let timer = Task {
      try await Task.sleep(for: timeout)
      result.yield(.expired)
    }
    defer {
      timer.cancel()
      result.finish()
    }
    let outcome = await withTaskCancellationHandler {
      var first = outcomes.makeAsyncIterator()
      return await first.next()
    } onCancel: {
      result.finish()
    }
    switch outcome {
    case .settled(let json): return json
    case .failed(let error): throw error
    case .expired:
      let elapsed = String(format: "%g s", timeout / .seconds(1))
      let message = "Page did not answer \(name) within \(elapsed)"
      if isCurrent(page) { failOpening(message, reason: .unresponsive) }
      throw SlopDiagnosticError(SlopFailure(message), diagnostic: .init(reason: .unresponsive))
    case nil: throw CancellationError()
    }
  }

  /// Saves every accepted edit before an export, a duplicate or a close reads it.
  public func flush() async throws {
    guard isReady else { throw SlopFailure("Document renderer unavailable") }
    do {
      try await callPage(.flush)
    } catch {
      if let failure = owner.saveFailure { throw failure }
      throw error
    }
  }

  /// Stop page-owned services and drain accepted edits while the host still has finite
  /// background execution time. Expiration never discards the live owner's state.
  public func pauseForBackground() async throws {
    filePicker.cancel()
    fileSaver.cancel()
    cancelSpeech()
    await liveWebView?.setCameraCaptureState(.none)
    await liveWebView?.setMicrophoneCaptureState(.none)
    try await saveAccepted()
    await sharing?.pause()
  }

  /// Edit ▸ Undo or Redo. A live page sends its unsent edits first, as the person sees
  /// them; otherwise the owner undoes directly.
  public func undo(redo: Bool = false) async throws {
    guard isReady else {
      _ = try await owner.undo(redo: redo)
      return
    }
    try await callPage(redo ? .redo : .undo)
  }

  /// Retries saving on the owner directly; works whether or not the page is alive.
  public func retrySave() async throws { try await owner.flush() }
  /// Saves what the document accepted: a live page sends its unsent text first; without
  /// one, the owner saves what it accepted.
  public func saveAccepted() async throws {
    if isReady { try await flush() } else { try await retrySave() }
  }

  /// One of the document's artwork images, read by its owner, off the main thread.
  public func artwork(_ name: SlopArtwork.Name) async -> Data? {
    try? await owner.artwork(name)
  }

  /// Exports the complete saved file, including identity, history and last saved artwork.
  /// Share a Copy remains the separate clean-copy operation below.
  @discardableResult public func exportFile(to destination: URL) async throws -> URL {
    guard !closed, !closing else { throw SlopFailure("Document is closing") }
    return try await withCapture {
      try await saveAccepted()
      try await owner.exportFile(to: destination)
      return try await SlopPreparation.run { try SlopFile.resolvedRoot(destination) }
    }
  }

  /// Drains page edits and freezes the live state as a clean copy, even when retained
  /// history is full. Artwork renders from that independent source. The destination has
  /// a new identity, no history and only referenced attachments, and is never replaced.
  @discardableResult public func copy(
    to destination: URL, renderArtwork: @MainActor (URL) async -> SlopRenderedArtwork? = { _ in nil }
  ) async throws -> URL {
    guard !closed, !closing else { throw SlopFailure("Document is closing") }
    let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString, isDirectory: true)
    try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
    defer { try? FileManager.default.removeItem(at: directory) }
    let source = directory.appendingPathComponent("copy.slop")
    try await withCapture {
      do {
        try await saveAccepted()
      } catch let failure as OwnerFailure where failure.kind == .full {
        // Page flush has drained bindings and accepted edits before its save barrier.
      }
      try await owner.copy(to: source, artwork: nil)
    }
    let artwork = await renderArtwork(source)
    try Task.checkCancellation()
    let frozen = try await SlopPreparation.run { try DocumentOwner(url: source) }
    do {
      try await frozen.copy(to: destination, artwork: artwork)
    } catch {
      try? await frozen.close()
      throw error
    }
    try await frozen.close()
    let copied = try SlopFile.resolvedRoot(destination)
    SlopFinderIcon.refresh(copied)
    return copied
  }

  /// Freeze saved state and all owned blobs before rendering. Only acquisition holds the
  /// editor barrier; the independent copy remains alive even if this window then closes.
  public func withCaptureSnapshot<T>(trace: SlopCloseTrace? = nil, _ render: @MainActor (URL) async throws -> T)
    async throws -> T
  {
    try await withSavedSource(trace: trace, prepare: { try await self.flush() }, render)
  }

  private func withSavedSource<T>(
    trace: SlopCloseTrace? = nil, prepare: @MainActor () async throws -> Void,
    _ consume: @MainActor (URL) async throws -> T
  ) async throws -> T {
    let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString, isDirectory: true)
    try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
    defer { try? FileManager.default.removeItem(at: directory) }
    let source = directory.appendingPathComponent("capture.slop")
    try await withCapture {
      let interval = trace?.begin("Snapshot acquisition")
      defer { trace?.end("Snapshot acquisition", interval) }
      guard !closed else { throw SlopFailure("Document closed") }
      if !closePrepared { try await prepare() }
      try await owner.captureSource(to: source)
    }
    return try await consume(source)
  }

  /// Whether this session saved any change, so its document's artwork may be out of date.
  public var edited: Bool { owner.edited }

  /// Serializes saved-source acquisition: the barrier `withCaptureSnapshot` and `copy`
  /// hold while they save and copy, never while they render. The flag stays set across
  /// queued acquisitions so close cannot release the source.
  func withCapture<T>(_ body: @MainActor () async throws -> T) async rethrows -> T {
    if capturing { await withCheckedContinuation { captureQueue.append($0) } }
    capturing = true
    defer {
      if captureQueue.isEmpty { capturing = false } else { captureQueue.removeFirst().resume() }
    }
    return try await body()
  }

  /// Drops unsaved edits and shows saved state in a new page. Returns once the owner has
  /// discarded; a replacement page that fails to open reports through the renderer
  /// failure path, never as a failed discard.
  public func discardPending() async throws {
    guard !closed, !closing, !capturing else { throw SlopFailure("Document is closing or exporting") }
    try await owner.discardPending()
    replaceWebView()
  }

  @discardableResult public func prepareClose() async throws -> [UnresolvedTextDraft] {
    guard !closed, !closePrepared else { return closeDrafts }
    guard !capturing else { throw SlopFailure("Document is exporting; try again when it finishes") }
    filePicker.cancel()
    fileSaver.cancel()
    closing = true
    cancelSpeech()
    do {
      if isReady {
        closeDrafts = try decodeClosePreparation(json: await callPage(.prepareClose))
      }
      closePrepared = true
      return closeDrafts
    } catch {
      if rendererDead {
        closePrepared = true
        return closeDrafts
      }
      closing = false
      if let failure = owner.saveFailure { throw failure }
      throw error
    }
  }

  public func cancelClose() async {
    closePrepared = false
    closeDrafts = []
    if isReady {
      _ = try? await callPage(.cancelClose)
    }
    closing = false
  }

  /// Saves, writes `artwork` and releases the document, then retires the page.
  public func close(artwork: SlopRenderedArtwork? = nil, trace: SlopCloseTrace? = nil) async throws {
    try await leave(trace: trace) { try await $0.close(artwork: artwork) }
  }

  /// Leaves a document that cannot save (a shared one that is full) without saving it:
  /// freezes its edits, renders `renderArtwork` from a copy of the frozen state, and has the
  /// owner publish the live state as a new, unshared document at `destination` before it
  /// releases the original. The owner never replaces an existing file; any failure leaves
  /// the document open with every edit, so other participants keep the original.
  @discardableResult public func handOff(
    to destination: URL, renderArtwork: @MainActor (URL) async -> SlopRenderedArtwork? = { _ in nil }
  ) async throws -> URL {
    guard !closed, !closing, closeTask == nil else { throw SlopFailure("Document is closing") }
    try await prepareClose()
    let artwork: SlopRenderedArtwork?
    do {
      let directory = FileManager.default.temporaryDirectory.appendingPathComponent(
        UUID().uuidString, isDirectory: true)
      try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
      defer { try? FileManager.default.removeItem(at: directory) }
      let source = directory.appendingPathComponent("copy.slop")
      try await owner.copy(to: source, artwork: nil)
      artwork = await renderArtwork(source)
    } catch {
      await cancelClose()
      throw error
    }
    try await leave { try await $0.handOff(to: destination, artwork: artwork) }
    let handedOff = try await SlopPreparation.run { try SlopFile.resolvedRoot(destination) }
    SlopFinderIcon.refresh(handedOff)
    return handedOff
  }

  private func leave(
    trace: SlopCloseTrace? = nil, _ end: @escaping @Sendable (DocumentOwner) async throws -> Void
  ) async throws {
    if let closeTask { return try await closeTask.value }
    let task = Task { try await self.finishClose(trace: trace, end) }
    closeTask = task
    do { try await task.value } catch {
      closeTask = nil
      throw error
    }
    closeTask = nil
  }

  /// The barrier has refused new edits, so the owner ends the document (saving it, or
  /// handing it off to a verified copy) before the app unmounts; a failed end leaves the app
  /// mounted and editable.
  private func finishClose(
    trace: SlopCloseTrace?, _ end: @Sendable (DocumentOwner) async throws -> Void
  ) async throws {
    if closed { return }
    try await prepareClose()
    // Discovery goes before the writer lock, so it can never name the next owner. A
    // failed close keeps ownership, so commands reach this session again.
    server?.withdraw()
    await sharing?.pause()
    do {
      let interval = trace?.begin("Rust owner close")
      defer { trace?.end("Rust owner close", interval) }
      try await end(owner)
    } catch {
      sharing?.resume()
      try? publishDiscovery()
      await cancelClose()
      throw error
    }
    server?.stop()
    server = nil
    if isReady { await unmountApp() }
    // Native callbacks may run while storage releases its writer lease below.
    // Retire the session before tearing down the renderer they would access.
    phase = .closed
    closeDrafts = []
    destroyWebView()
    completeWaiters(.failure(SlopFailure("Document closed")))
  }

  /// The document is saved and released, so the app's teardown cannot change it: a
  /// failing or unsettled `unmount` is logged after a short grace, never awaited. The
  /// call holds no reference to the page, so an abandoned one is released with the view.
  private func unmountApp(grace: Duration = Timeouts.unmount) async {
    enum Outcome: Sendable {
      case settled
      case failed(String)
      case unsettled
    }
    let (outcomes, result) = AsyncStream<Outcome>.makeStream()
    webView.callHost(.close) { reply in
      if case .failure(let error) = reply {
        result.yield(.failed(error.localizedDescription))
      } else {
        result.yield(.settled)
      }
    }
    let timer = Task {
      try await Task.sleep(for: grace)
      result.yield(.unsettled)
    }
    var first = outcomes.makeAsyncIterator()
    let outcome = await first.next() ?? .unsettled
    timer.cancel()
    result.finish()
    switch outcome {
    case .settled: break
    case .failed(let message): NSLog("hitSlop: app unmount after close failed: %@", message)
    case .unsettled: NSLog("hitSlop: app unmount did not finish within %@", "\(grace)")
    }
  }

  /// The OS writer lease stays owned while replacing a failed renderer.
  public func reopenSavedDocument() async throws {
    guard !closed, !closing else { throw SlopFailure("Document is closing or closed") }
    guard !capturing else { throw SlopFailure("Document is capturing; try again when it finishes") }
    guard rendererDead || openingError != nil else {
      throw SlopFailure("Renderer is still active; retry saving instead")
    }
    replaceWebView()
    try await waitUntilReady()
  }

  private func replaceWebView() {
    destroyWebView()
    phase = .active(.opening)
    makeWebView()
    delegate?.pageSession(self, didReplace: webView)
    load()
  }

  private func destroyWebView() {
    filePicker.cancel()
    fileSaver.cancel()
    cancelSpeech()
    // A retired page's camera and microphone end now, not whenever WebKit releases it.
    liveWebView?.setCameraCaptureState(.none)
    liveWebView?.setMicrophoneCaptureState(.none)
    liveWebView?.uiDelegate = nil
    liveWebView?.navigationDelegate = nil
    if let liveWebView { WebViewProcess.terminate(liveWebView) }
    liveWebView?.stopLoading()
    liveWebView?.configuration.userContentController.removeScriptMessageHandler(
      forName: "hitslop", contentWorld: .page)
    liveWebView?.removeFromSuperview()
    liveWebView = nil
  }

  /// The socket and discovery file live as long as the owner, not the page.
  func startDiscovery() throws {
    #if os(macOS)
      guard server == nil else { return }
      server = try owner.startServer(
        exporter: NativeExports(copy: { [weak self] output, deadline in
          guard let self, acceptsPageWork else { throw OwnerFailure.closing }
          try deadline.check()
          _ = try await self.copy(to: output)
          try deadline.check()
        }) { [weak self] _, format, output, deadline in
          guard let self, !closed, !closing else { throw OwnerFailure.closing }
          guard let onExport, isReady else { throw SlopFailure("Export unavailable") }
          try await onExport(format, output, deadline)
        })
    #endif
  }

  private func publishDiscovery() throws { try server?.publish() }

  func failOpening(
    _ message: String, reason: SlopFailureContext.Reason = .startup,
    classification: SlopFailureContext.Classification = .platform
  ) {
    let terminated = reason == .webContentTerminated || reason == .unresponsive
    guard !closed, openingError == nil || (terminated && !rendererDead) else { return }
    cancelSpeech()
    let wasReady = isReady
    let failed = Renderer.failed(message, terminated: terminated)
    phase = closing ? .closing(failed, prepared: false) : .active(failed)
    // The owner is fully loaded before the page starts, so it keeps the writer lock and
    // the socket; a retry reattaches a new page to the same owner.
    if !wasReady {
      destroyWebView()
    } else if reason == .unresponsive, let liveWebView {
      // A wedged page keeps its process spinning until Reopen or close replaces the view.
      WebViewProcess.terminate(liveWebView)
    }
    let error = SlopDiagnosticError(SlopFailure(message), diagnostic: .init(classification, reason: reason))
    completeWaiters(.failure(error))
    delegate?.pageSession(self, didFail: error)
  }
  private func report(_ issue: SlopPageIssue) { delegate?.pageSession(self, didReport: issue) }

}

/// WebKit has no public transparent-background API on macOS. Keep this exception isolated.
@MainActor public enum WebViewBackground {
  public static func get(_ view: WKWebView) -> Bool {
    #if os(macOS)
      view.value(forKey: "drawsBackground") as? Bool ?? true
    #else
      view.isOpaque
    #endif
  }
  public static func set(_ value: Bool, on view: WKWebView) {
    #if os(macOS)
      view.setValue(value, forKey: "drawsBackground")
    #else
      view.isOpaque = value
      view.backgroundColor = value ? .systemBackground : .clear
      view.scrollView.backgroundColor = view.backgroundColor
    #endif
  }
}

/// Releasing a spinning view does not reliably stop its dedicated WebContent process.
/// WebKit has no public process-termination API; keep this exception isolated.
@MainActor private enum WebViewProcess {
  static func terminate(_ view: WKWebView) {
    #if os(macOS)
      let selector = NSSelectorFromString("_killWebContentProcessAndResetState")
      if view.responds(to: selector) { view.perform(selector) }
    #else
      view.stopLoading()
    #endif
  }
}
