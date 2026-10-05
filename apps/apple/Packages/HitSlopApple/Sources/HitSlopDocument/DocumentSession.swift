import AppKit
import Foundation
import HitSlopCore
import HitSlopCoreBinding
@preconcurrency import WebKit

/// Whether a document's edits are durable. A failed save keeps the edits live and unsaved.
public enum DocumentSaveStatus: Sendable, Equatable {
  case saved, saving
  case failed(SaveFailure)
}

/// A document's writer lease outlives its renderer. The native Rust owner interprets Loro bytes.
@MainActor
public final class DocumentSession: NSObject, WKScriptMessageHandlerWithReply, WKNavigationDelegate, WKUIDelegate {
  private var liveWebView: WKWebView?
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
  public var isReady: Bool { if case .ready? = renderer { return true }; return false }
  public var rendererDead: Bool {
    if case .failed(_, terminated: true)? = renderer { return true }; return false
  }
  private var openingError: String? {
    if case .failed(let message, _)? = renderer { return message }; return nil
  }
  private var closed: Bool { if case .closed = phase { return true }; return false }
  private var closing: Bool {
    get { if case .closing = phase { return true }; return false }
    set {
      guard let renderer else { return }
      phase = newValue ? .closing(renderer, prepared: false) : .active(renderer)
    }
  }
  private var closePrepared: Bool {
    get { if case .closing(_, prepared: true) = phase { return true }; return false }
    set {
      guard case .closing(let renderer, _) = phase else { return }
      phase = .closing(renderer, prepared: newValue)
    }
  }
  public private(set) var failureReason: SlopFailureContext.Reason?
  public var onExport: ((ExportFormat, URL, NativeCommandDeadline) async throws -> Void)?
  public private(set) var capturing = false
  /// Captures queued behind the one in progress (`withCapture`).
  private var captureQueue: [CheckedContinuation<Void, Never>] = []
  /// An owner selects and saves files; a snapshot rendered in the background never does.
  var allowsFileSelection: Bool { owner.mode == .document }
  /// The system panels, which tests replace with scripted presenters.
  var filePicker = DocumentFilePicker()
  var fileSaver = DocumentFileSaver() {
    didSet { configureFileSaver() }
  }
  /// Every read-only snapshot page is a disposable renderer.
  private func markRenderTarget(_ configuration: WKWebViewConfiguration) {
    configuration.userContentController.addUserScript(
      WKUserScript(
        source: "document.documentElement.setAttribute('data-slop-renderer','true')",
        injectionTime: .atDocumentStart, forMainFrameOnly: true))
  }
  /// The window that shows this session.
  public weak var delegate: DocumentSessionDelegate?
  /// Whether Edit ▸ Undo and Redo have anything to do in this document.
  public private(set) var undoAvailability = UndoState(canUndo: false, canRedo: false)
  let owner: DocumentOwner
  private var server: NativeSocketServer?
  /// The attached page's token (see `makeWebView`).
  private(set) var view = UUID().uuidString
  nonisolated private let pushes = PushQueue()
  private var closeTask: Task<Void, Error>?
  /// The manifest presentation as the page's `config` reply carries it.
  private lazy var presentation: [String: Any] = (try? JSONSerialization.jsonObject(
    with: JSONEncoder().encode(file.manifest.presentation))) as? [String: Any] ?? [:]
  private var waiters: [UUID: CheckedContinuation<Void, Error>] = [:]
  private let webViewResources: URL

  /// What opening reads off the main actor: the owner, with the app's assets, and the page
  /// shell.
  struct Prepared: Sendable {
    let shell: URL
    let owner: DocumentOwner

    init(url: URL, storage mode: StoreMode = .document) throws {
      shell = try DocumentSession.pageShell()
      owner = try DocumentOwner(url: url, mode: mode)
    }
  }

  /// The page shell bundled with this build; every document page loads it.
  nonisolated static func pageShell() throws -> URL {
    guard let url = Bundle.module.url(forResource: "shell", withExtension: nil),
      ["boot.js", "index.js"].allSatisfy({ FileManager.default.fileExists(atPath: url.appendingPathComponent($0).path) })
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
  }
  private func configureFileSaver() {
    fileSaver.window = { [weak self] in self?.liveWebView?.window }
    fileSaver.onFailed = { [weak self] message in self?.report(SlopPageIssue(message: message, isOperation: true)) }
  }

  private func observeOwner() {
    owner.onPublication = { [weak self] publication in
      self?.push(#"{"type":"publication","publication":"# + publication + "}")
    }
    // The ordered document publications restyle the page. This only refreshes the panel.
    owner.onTheme = { [weak self] in
      Task { @MainActor [weak self] in
        guard let self, let theme = try? await currentTheme() else { return }
        delegate?.pageSession(self, themeChanged: theme)
      }
    }
    owner.onUndoState = { [weak self] state in
      DispatchQueue.main.async { self?.undoAvailability = state }
    }
    owner.onSaveStatus = { [weak self] status in
      DispatchQueue.main.async {
        guard let self else { return }
        self.delegate?.pageSession(self, saveStatus: status)
      }
    }
  }

  // MARK: Theme panel

  /// Whether this window may change its document's palette.
  public var canEditTheme: Bool { isReady && !rendererDead && !closing && !closed && owner.mode == .document }
  /// The palette now; later changes arrive through `pageSession(_:themeChanged:)`.
  public func currentTheme() async throws -> SlopThemeState {
    try SlopThemeState(try await owner.loadTheme())
  }
  /// Applies a panel change in the order changes are made. It is accepted in memory,
  /// restyles the page, and is saved like an edit. `reply` reports the owner's theme
  /// revision once the change is accepted (unchanged when it changed nothing), or the
  /// refusal.
  public func changeTheme(_ change: SlopThemeChange, reply: @escaping @MainActor (Result<Int, Error>) -> Void = { _ in }) {
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
      guard !rendererDead, let webView = liveWebView, batch.view == view else {
        pushes.acknowledge(batch)
        continue
      }
      do {
        _ = try await webView.callHost(.publish(.init(payload: "[" + batch.items.joined(separator: ",") + "]")))
        pushes.acknowledge(batch)
        retryMS = 100
      } catch {
        pushes.failed(batch)
        if retryView != batch.view { retryView = batch.view; retryMS = 100 }
        try? await Task.sleep(for: .milliseconds(retryMS))
        retryMS = min(retryMS * 2, 2000)
        // A renderer that died reports through its termination path; only a live page
        // that could not take the changes is an issue.
        if !rendererDead, batch.view == view {
          report(SlopPageIssue(message: "Document changes could not reach the interface: \(error.localizedDescription)", isOperation: false))
        }
      }
    }
  }

  /// Opens the document at `url`: as its owner, or as a snapshot of its saved state for a
  /// background render, which selects and saves no files.
  public static func open(
    url: URL, storage: StoreMode = .document
  ) async throws -> DocumentSession {
    try await finishOpening(try await prepare(url: url, storage: storage))
  }

  private static func prepare(url: URL, storage mode: StoreMode = .document) async throws -> Prepared {
    try await SlopPreparation.run {
      // The owner's store opens and checks the file once; the session shows it from that.
      do { return try Prepared(url: SlopFile.documentRoot(url), storage: mode) }
      catch let error as SlopError {
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
    guard isReady, !closed, !closing, !capturing, !rendererDead else {
      throw SlopFailure("Document interface unavailable")
    }
    filePicker.cancel()
    fileSaver.cancel()
    _ = try await webView.callHost(.reloadInterface(.init()))
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
    configuration.setURLSchemeHandler(SchemeHandler(assets: owner.assets, shell: webViewResources), forURLScheme: "slop")
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
            webkit.messageHandlers.hitslop.postMessage({method:'failed',error}).catch(()=>{});
          }, true);
          """, injectionTime: .atDocumentStart, forMainFrameOnly: true))
    if isSnapshot { markRenderTarget(configuration) }
    let spec = file.manifest.presentation
    let view = WKWebView(
      frame: CGRect(x: 0, y: 0, width: spec.width, height: spec.height),
      configuration: configuration)
    WebViewBackground.set(!file.usesTransparentBackground, on: view)
    view.navigationDelegate = self
    view.uiDelegate = self
    liveWebView = view
  }

  public func load() { liveWebView?.load(URLRequest(url: URL(string: "slop://app/")!)) }

  public func waitUntilReady(timeout: Duration = Timeouts.pageReady) async throws {
    if isReady { return }
    if closed || rendererDead || openingError != nil {
      throw SlopFailure(openingError ?? "Document page unavailable")
    }
    let id = UUID()
    let timer = Task { [weak self] in
      try await Task.sleep(for: timeout)
      self?.waiters.removeValue(forKey: id)?.resume(throwing: SlopFailure("Document page did not become ready"))
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
    let invalid = { replyHandler(RequestOutcome.page(OwnerError.rejected("Invalid page request")), nil) }
    guard !closed, message.webView === liveWebView, message.frameInfo.isMainFrame,
      message.frameInfo.securityOrigin.protocol == "slop",
      message.frameInfo.securityOrigin.host == "app",
      let body = message.body as? [String: Any],
      let method = (body["method"] as? String).flatMap(PageRequest.Method.init(rawValue:))
    else { return invalid() }
    switch method {
    case .open, .apply, .flush, .undo, .redo, .attachmentsPut, .attachmentsRead:
      return servePage(body, storage: method == .attachmentsPut || method == .attachmentsRead, reply: replyHandler)
    case .config, .windowResize, .ready, .pageRecovered, .failed, .pageError: break
    }
    guard let request = PageRequest.checked(body) else { return invalid() }
    switch request {
    case .open, .apply, .flush, .undo, .redo, .attachmentsPut, .attachmentsRead:
      invalid()
    case .config:
      let page = message.webView
      Task { @MainActor [weak self] in
        guard let self else { return replyHandler(RequestOutcome.page(OwnerError.rejected("Page unavailable")), nil) }
        do {
          guard page === liveWebView else { throw OwnerReplaced() }
          guard let descriptor = try JSONSerialization.jsonObject(with: Data(file.descriptor.utf8)) as? [String: Any]
          else { throw SlopFailure("Invalid document descriptor") }
          replyHandler(PageResult.config(.init(
            runtimeABI: file.runtimeABI, readOnly: owner.mode == .snapshot, presentation: presentation,
            descriptor: descriptor)).json, nil)
        } catch { replyHandler(RequestOutcome.page(error), nil) }
      }
    case .windowResize(let r):
      do {
        guard file.isResizable, let delegate, !capturing, !closing, !closed
        else { throw SlopFailure("Window resizing unavailable") }
        let size = try delegate.pageSession(self, resizeContentTo: CGSize(width: r.width, height: r.height))
        replyHandler(PageResult.windowResize(.init(width: Double(size.width), height: Double(size.height))).json, nil)
      } catch { replyHandler(RequestOutcome.page(error), nil) }
    case .ready:
      switch phase {
      case .active(.opening): phase = .active(.ready)
      // A close that began while the page was opening must not turn its readiness
      // into a renderer failure.
      case .closing(.opening, let prepared): phase = .closing(.ready, prepared: prepared)
      default:
        replyHandler(RequestOutcome.page(OwnerError.rejected("Document page is not opening")), nil)
        return
      }
      completeWaiters(.success(()))
      delegate?.pageSessionDidBecomeReady(self)
      replyHandler(PageResult.ready.json, nil)
    case .pageRecovered:
      delegate?.pageSessionRecovered(self)
      replyHandler(PageResult.pageRecovered.json, nil)
    case .failed(let r):
      failOpening(r.error, reason: .startup, classification: .platform)
      replyHandler(PageResult.failed.json, nil)
    case .pageError(let r):
      if isReady { report(SlopPageIssue(message: r.error, isOperation: r.kind == .operation)) }
      else { failOpening(r.error, reason: .authoredException, classification: .authored) }
      replyHandler(PageResult.pageError.json, nil)
    }
  }

  /// Saves every accepted edit before an export, a duplicate or a close reads it.
  public func flush() async throws {
    guard isReady, !rendererDead, !closed else { throw SlopFailure("Document renderer unavailable") }
    do {
      _ = try await webView.callHost(.flush(.init()))
    } catch {
      if let failure = owner.saveFailure { throw failure }
      throw error
    }
  }

  /// Edit ▸ Undo or Redo. A live page sends its unsent edits first, as the person sees
  /// them; otherwise the owner undoes directly.
  public func undo(redo: Bool = false) async throws {
    guard isReady, !rendererDead, !closed else {
      _ = try await owner.undo(redo: redo)
      return
    }
    _ = try await webView.callHost(redo ? .redo(.init()) : .undo(.init()))
  }

  /// Retries saving on the owner directly; works whether or not the page is alive.
  public func retrySave() async throws { try await owner.flush() }
  /// Saves what the document accepted: a live page sends its unsent text first; without
  /// one, the owner saves what it accepted.
  public func saveAccepted() async throws {
    if isReady && !rendererDead { try await flush() } else { try await retrySave() }
  }

  /// One of the document's artwork images, read by its owner, off the main thread.
  public func artwork(_ name: SlopArtwork.Name) async -> Data? {
    try? await owner.artwork(name)
  }
  /// Copies the document to `destination`, with everything the owner accepted saved first.
  /// Never replaces an existing file. The copy gets its own Finder icon: file metadata is
  /// not part of the copy.
  /// Returns the copy's canonical URL (the identity Recents and live owners use).
  @discardableResult public func copy(to destination: URL) async throws -> URL {
    guard !closed, !closing else { throw SlopFailure("Document is closing") }
    try await owner.copy(to: destination)
    let copied = try SlopFile.resolvedRoot(destination)
    SlopFinderIcon.refresh(copied)
    return copied
  }

  public var isSnapshot: Bool { owner.mode == .snapshot }

  /// Freeze saved state and all owned blobs before rendering. Only acquisition holds the
  /// editor barrier; the independent copy remains alive even if this window then closes.
  public func withCaptureSnapshot<T>(_ render: (URL) async throws -> T) async throws -> T {
    let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString, isDirectory: true)
    try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
    defer { try? FileManager.default.removeItem(at: directory) }
    let source = directory.appendingPathComponent("capture.slop")
    try await withCapture {
      guard !closed else { throw SlopFailure("Document closed") }
      if !closePrepared { try await flush() }
      try await owner.copy(to: source, durable: false)
    }
    return try await render(source)
  }

  /// Whether this session saved any change, so its document's artwork may be out of date.
  public var edited: Bool { owner.edited }

  /// Serializes source acquisition in an editor or rendering in a disposable page.
  /// The flag stays set across queued acquisitions so close cannot release the source.
  public func withCapture<T>(_ body: () async throws -> T) async rethrows -> T {
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

  public func prepareClose() async throws {
    guard !closed, !closePrepared else { return }
    guard !capturing else { throw SlopFailure("Document is exporting; try again when it finishes") }
    filePicker.cancel()
    fileSaver.cancel()
    closing = true
    do {
      if isReady && !rendererDead {
        _ = try await webView.callHost(.prepareClose(.init()))
      }
      closePrepared = true
    } catch {
      closing = false
      if let failure = owner.saveFailure { throw failure }
      throw error
    }
  }

  public func cancelClose() async {
    closePrepared = false
    if isReady && !closed && !rendererDead {
      _ = try? await webView.callHost(.cancelClose(.init()))
    }
    closing = false
  }

  /// Saves, writes `artwork` and releases the document, then retires the page.
  public func close(artwork: SlopRenderedArtwork? = nil) async throws {
    if let closeTask { return try await closeTask.value }
    let task = Task { try await self.finishClose(artwork: artwork) }
    closeTask = task
    do { try await task.value } catch {
      closeTask = nil
      throw error
    }
    closeTask = nil
  }

  /// The barrier has refused new edits, so the owner saves and releases the document
  /// before the app unmounts; a failed close leaves the app mounted and editable.
  private func finishClose(artwork: SlopRenderedArtwork?) async throws {
    if closed { return }
    try await prepareClose()
    // Discovery goes before the writer lock, so it can never name the next owner. A
    // failed close keeps ownership, so commands reach this session again.
    server?.withdraw()
    do { try await owner.close(artwork: artwork) } catch {
      try? publishDiscovery()
      await cancelClose()
      throw error
    }
    server?.stop()
    server = nil
    if isReady && !rendererDead { await unmountApp() }
    // Native callbacks may run while storage releases its writer lease below.
    // Retire the session before tearing down the renderer they would access.
    phase = .closed
    destroyWebView()
    completeWaiters(.failure(SlopFailure("Document closed")))
  }

  /// The document is saved and released, so the app's teardown cannot change it: a
  /// failing or unsettled `unmount` is logged after a short grace, never awaited. The
  /// call holds no reference to the page, so an abandoned one is released with the view.
  private func unmountApp(grace: Duration = Timeouts.unmount) async {
    enum Outcome: Sendable { case settled, failed(String), unsettled }
    let (outcomes, result) = AsyncStream<Outcome>.makeStream()
    webView.callHost(.close(.init())) { reply in
      if case .failure(let error) = reply { result.yield(.failed(error.localizedDescription)) }
      else { result.yield(.settled) }
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
    guard !closed, !closing, !capturing, rendererDead || openingError != nil else {
      throw SlopFailure("Renderer is still active; retry saving instead")
    }
    replaceWebView()
    try await waitUntilReady()
  }

  private func replaceWebView() {
    destroyWebView()
    failureReason = nil
    phase = .active(.opening)
    makeWebView()
    delegate?.pageSession(self, didReplace: webView)
    load()
  }

  private func destroyWebView() {
    filePicker.cancel()
    fileSaver.cancel()
    liveWebView?.uiDelegate = nil
    liveWebView?.stopLoading()
    liveWebView?.configuration.userContentController.removeScriptMessageHandler(
      forName: "hitslop", contentWorld: .page)
    liveWebView?.navigationDelegate = nil
    liveWebView?.removeFromSuperview()
    liveWebView = nil
  }

  /// The socket and discovery file live as long as the owner, not the page.
  func startDiscovery() throws {
    guard server == nil, owner.mode == .document else { return }
    server = try owner.startServer(exporter: NativeExports { [weak self] _, format, output, deadline in
      guard let self, !closed, !closing else { throw OwnerError.closing }
      guard let onExport, isReady, !rendererDead else { throw SlopFailure("Export unavailable") }
      try await onExport(format, output, deadline)
    })
  }

  private func publishDiscovery() throws { try server?.publish() }

  private func failOpening(_ message: String, reason: SlopFailureContext.Reason = .startup,
                           classification: SlopFailureContext.Classification = .platform) {
    guard !closed, openingError == nil || reason == .webContentTerminated else { return }
    failureReason = reason
    let wasReady = isReady
    let failed = Renderer.failed(message, terminated: reason == .webContentTerminated)
    phase = closing ? .closing(failed, prepared: false) : .active(failed)
    // The owner is fully loaded before the page starts, so it keeps the writer lock and
    // the socket; a retry reattaches a new page to the same owner.
    if !wasReady { destroyWebView() }
    completeWaiters(.failure(SlopFailure(message)))
    delegate?.pageSession(self, didFail: SlopDiagnosticError(
      SlopFailure(message), diagnostic: .init(classification, reason: reason)))
  }
  private func report(_ issue: SlopPageIssue) { delegate?.pageSession(self, didReport: issue) }

  public func webViewWebContentProcessDidTerminate(_ webView: WKWebView) {
    guard webView === liveWebView, !closed else { return }
    filePicker.cancel()
    fileSaver.cancel()
    failOpening(
      "The document renderer stopped. Accepted edits remain in the native owner. Reopen the interface to continue; unsubmitted input may be unavailable.",
      reason: .webContentTerminated
    )
  }

  public func webView(
    _ webView: WKWebView, runOpenPanelWith parameters: WKOpenPanelParameters,
    initiatedByFrame frame: WKFrameInfo,
    completionHandler: @escaping @MainActor ([URL]?) -> Void
  ) {
    guard webView === liveWebView, allowsFileSelection, !capturing,
      isReady, !closing, !closed, !rendererDead,
      frame.isMainFrame, frame.securityOrigin.protocol == "slop",
      frame.securityOrigin.host == "app", let window = webView.window,
      window.isVisible
    else { completionHandler(nil); return }
    filePicker.present(in: window, multiple: parameters.allowsMultipleSelection,
      directories: parameters.allowsDirectories, completion: completionHandler)
  }

  public func webView(
    _ webView: WKWebView, navigationAction: WKNavigationAction, didBecome download: WKDownload
  ) {
    guard webView === liveWebView, allowsFileSelection, !capturing,
      isReady, !closing, !closed, !rendererDead else { download.cancel { _ in }; return }
    fileSaver.begin(download)
  }

  public func webView(
    _ webView: WKWebView, didFailProvisionalNavigation navigation: WKNavigation!,
    withError error: Error
  ) {
    guard webView === liveWebView, !closed, !closing,
          !SlopFailureContext.isCancellation(error) else { return }
    failOpening(error.localizedDescription, reason: .navigation)
  }

  public func webView(
    _ webView: WKWebView, decidePolicyFor action: WKNavigationAction,
    decisionHandler: @escaping @MainActor (WKNavigationActionPolicy) -> Void
  ) {
    if let url = action.request.url, url.scheme == "slop", url.host == "app" {
      decisionHandler(.allow)
      return
    }
    // Authored `<a download>` of in-page bytes: native asks where to save them.
    if action.shouldPerformDownload, let url = action.request.url, ["blob", "data"].contains(url.scheme),
      webView === liveWebView, allowsFileSelection, !capturing, isReady, !closing, !closed,
      action.sourceFrame.isMainFrame, action.sourceFrame.securityOrigin.protocol == "slop",
      action.sourceFrame.securityOrigin.host == "app"
    {
      decisionHandler(.download)
      return
    }
    // Embedded HTTPS frames may load and navigate themselves. The CSP `frame-src` gates the
    // initial load; the page itself can never leave `slop://app` (main frame stays cancelled).
    if let target = action.targetFrame, !target.isMainFrame, action.request.url?.scheme == "https" {
      decisionHandler(.allow)
      return
    }
    // Only the app's own links open the system browser; a click inside an embed does not.
    if action.navigationType == .linkActivated, action.sourceFrame.isMainFrame,
      let url = action.request.url, ["https", "http"].contains(url.scheme)
    {
      NSWorkspace.shared.open(url)
    }
    decisionHandler(.cancel)
  }
}

/// WebKit has no public transparent-background API on macOS. Keep this exception isolated.
@MainActor public enum WebViewBackground {
  public static func get(_ view: WKWebView) -> Bool {
    view.value(forKey: "drawsBackground") as? Bool ?? true
  }
  public static func set(_ value: Bool, on view: WKWebView) {
    view.setValue(value, forKey: "drawsBackground")
  }
}
