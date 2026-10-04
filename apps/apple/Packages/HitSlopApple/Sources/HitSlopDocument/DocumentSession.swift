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
  /// The owner's epoch: socket clients name it, and it rotates on discard.
  public var epoch: String { owner.epoch }
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
  public private(set) var failureClassification: SlopFailureContext.Classification = .platform
  public var onExport: ((ExportFormat, URL, NativeCommandDeadline) async throws -> Void)?
  /// While a capture reads the page, accepted theme changes wait to restyle it.
  public private(set) var capturing = false {
    didSet { if oldValue && !capturing { refreshTheme() } }
  }
  /// Captures queued behind the one in progress (`withCapture`).
  private var captureQueue: [CheckedContinuation<Void, Never>] = []
  public var allowsFileSelection = true {
    didSet { if !allowsFileSelection { filePicker.cancel(); fileSaver.cancel() } }
  }
  /// The system panels, which tests replace with scripted presenters.
  var filePicker = DocumentFilePicker()
  var fileSaver = DocumentFileSaver() {
    didSet { configureFileSaver() }
  }
  /// Render-target pages (icons, close-time assets) mark the document so authored capture
  /// views render; every replacement page carries the same marker.
  public var renderTargetsEnabled = false {
    didSet { if renderTargetsEnabled && !oldValue { markRenderTarget(webView.configuration) } }
  }
  private func markRenderTarget(_ configuration: WKWebViewConfiguration) {
    configuration.userContentController.addUserScript(
      WKUserScript(
        source: "document.documentElement.setAttribute('data-slop-renderer','true')",
        injectionTime: .atDocumentStart, forMainFrameOnly: true))
  }
  /// The window that shows this session.
  public weak var delegate: DocumentSessionDelegate?
  /// Whether Edit ▸ Undo and Redo have anything to do in this document.
  public private(set) var undoAvailability = UndoAvailability()
  let owner: DocumentOwner
  /// The app's assets, served to every page this session shows.
  private let assets: AssetReader?
  private var server: SocketServer?
  /// The attached page's token (see `makeWebView`).
  private(set) var view = UUID().uuidString
  nonisolated private let pushes = PushQueue()
  private var closeTask: Task<Void, Error>?
  /// The manifest presentation as the page's `config` reply carries it.
  private lazy var presentation: [String: Any] = (try? JSONSerialization.jsonObject(
    with: JSONEncoder().encode(file.manifest.presentation))) as? [String: Any] ?? [:]
  private var waiters: [UUID: CheckedContinuation<Void, Error>] = [:]
  private let webViewResources: URL

  /// What opening reads off the main actor: the owner, the page shell and the app's assets.
  struct Prepared: Sendable {
    let shell: URL
    let owner: DocumentOwner
    let assets: AssetReader?

    init(url: URL, storage mode: StorageMode = .document) throws {
      shell = try DocumentSession.pageShell()
      owner = try DocumentOwner(url: url, mode: mode)
      // The owner checked the file; a reader that fails here fails every asset request.
      assets = try? AssetReader.open(path: url.path)
    }
  }

  /// The page shell bundled with this build; every document page loads it.
  nonisolated static func pageShell() throws -> URL {
    guard let url = Bundle.module.url(forResource: "shell", withExtension: nil),
      ["boot.js", "index.js"].allSatisfy({ FileManager.default.fileExists(atPath: url.appendingPathComponent($0).path) })
    else { throw failure("Incomplete page shell; reinstall hitSlop") }
    return url
  }

  private init(prepared: Prepared) {
    file = prepared.owner.file
    webViewResources = prepared.shell
    owner = prepared.owner
    assets = prepared.assets
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
    // Theme changes from the panel or the CLI restyle the open page; the page only
    // applies values.
    owner.onTheme = { [weak self] in
      Task { @MainActor [weak self] in
        self?.themeDirty = true
        self?.refreshTheme()
      }
    }
    owner.onUndoState = { [weak self] state in
      DispatchQueue.main.async { self?.undoAvailability = state }
    }
    owner.publishUndoState()
    owner.onSaveStatus = { [weak self] status in
      DispatchQueue.main.async {
        guard let self else { return }
        self.delegate?.pageSession(self, saveStatus: status)
      }
    }
  }

  private var themeDirty = true
  private var themeDelivery: Task<Void, Never>?
  private var settlingTheme = false
  /// Delivers the latest effective palette, serializing deliveries and retaining changes
  /// made while the page opens. A stale page can never restyle its replacement. Waits
  /// while a capture reads the page, except to settle before it.
  private func refreshTheme() {
    guard themeDelivery == nil else { return }
    themeDelivery = Task { [weak self] in
      await self?.deliverTheme()
      self?.themeDelivery = nil
    }
  }
  private func deliverTheme() async {
    while themeDirty, isReady, !rendererDead, !capturing || settlingTheme, let webView = liveWebView {
      themeDirty = false
      do {
        let read = try await owner.loadTheme()
        guard webView === liveWebView else { themeDirty = true; continue }
        _ = try await webView.callAsyncJavaScript(
          "globalThis.__slop.applyTheme(JSON.parse(values)); return true",
          arguments: ["values": read.state.effective], in: nil, contentWorld: .page)
        delegate?.pageSession(self, themeChanged: try SlopThemeState(read))
      } catch { themeDirty = true; return }
    }
  }
  /// Every accepted theme change has reached the page, so a capture shows it.
  private func settleTheme() async throws {
    settlingTheme = true
    defer { settlingTheme = false }
    await themeDelivery?.value
    if themeDirty {
      refreshTheme()
      await themeDelivery?.value
    }
    if themeDirty { throw failure("Theme could not be applied to the page") }
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
    guard !closing, !closed else { return reply(.failure(failure("Document is closing"))) }
    owner.enqueueTheme(change) { result in
      Task { @MainActor in reply(result.map(\.revision)) }
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
        _ = try await webView.callAsyncJavaScript("globalThis.__slop?.publish(JSON.parse(payload)); return true",
          arguments: ["payload": "[" + batch.items.joined(separator: ",") + "]"], in: nil, contentWorld: .page)
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
    url: URL, renderTargetsEnabled: Bool = false, storage: StorageMode = .document
  ) async throws -> DocumentSession {
    let prepared = try await prepare(url: url, storage: storage)
    let session = try await finishOpening(prepared)
    session.allowsFileSelection = storage == .document
    session.renderTargetsEnabled = renderTargetsEnabled
    return session
  }

  static func prepare(url: URL, storage mode: StorageMode = .document) async throws -> Prepared {
    try await SlopPreparation.run {
      try SlopLocalDocument.requireLocal(url)
      // The owner's store opens and checks the file once; the session shows it from that.
      do { return try Prepared(url: SlopFile.resolvedRoot(url), storage: mode) }
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
      throw failure("Document interface unavailable")
    }
    filePicker.cancel()
    fileSaver.cancel()
    _ = try await webView.callAsyncJavaScript(
      "await globalThis.__slop.reloadInterface(); return true", arguments: [:], in: nil,
      contentWorld: .page)
  }
  private func makeWebView() {
    // Each page gets its own view token; requests from a replaced page are refused.
    closePrepared = false
    themeDirty = true
    view = UUID().uuidString
    pushes.configure(view: view)
    owner.attach(view: view)
    let configuration = WKWebViewConfiguration()
    configuration.websiteDataStore = .nonPersistent()
    configuration.preferences.javaScriptCanOpenWindowsAutomatically = false
    configuration.setURLSchemeHandler(SchemeHandler(assets: assets, shell: webViewResources), forURLScheme: "slop")
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
    if renderTargetsEnabled { markRenderTarget(configuration) }
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

  public func waitUntilReady(timeout: Duration = .seconds(15)) async throws {
    if isReady { return }
    if closed || rendererDead || openingError != nil {
      throw failure(openingError ?? "Document page unavailable")
    }
    let id = UUID()
    let timer = Task { [weak self] in
      try await Task.sleep(for: timeout)
      self?.waiters.removeValue(forKey: id)?.resume(throwing: failure("Document page did not become ready"))
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
    guard !closed, message.webView === liveWebView, message.frameInfo.isMainFrame,
      message.frameInfo.securityOrigin.protocol == "slop",
      message.frameInfo.securityOrigin.host == "app",
      let request = PageRequest.checked(message.body)
    else {
      replyHandler(RequestOutcome.page(OwnerError.rejected("Invalid page request")), nil)
      return
    }
    switch request {
    case .open, .apply, .text, .flush, .undo, .redo:
      owner.admitPage(request, view: view) { replyHandler($0, nil) }
    case .config:
      let page = message.webView
      Task { @MainActor [weak self] in
        guard let self else { return replyHandler(RequestOutcome.page(OwnerError.rejected("Page unavailable")), nil) }
        do {
          // A queued request from a replaced page cannot consume the new page's
          // dirty flag. Changes arriving during the read set it again.
          guard page === liveWebView else { throw OwnerReplaced() }
          themeDirty = false
          let theme: [String: String]
          do {
            let values = try JSONSerialization.jsonObject(with: Data(try await owner.loadTheme().state.effective.utf8))
            guard let values = values as? [String: String] else { throw failure("Invalid effective theme") }
            theme = values
          } catch {
            if page === liveWebView { themeDirty = true }
            throw error
          }
          guard let descriptor = try JSONSerialization.jsonObject(with: Data(file.descriptor.utf8)) as? [String: Any]
          else { throw failure("Invalid document descriptor") }
          replyHandler(PageResult.config(.init(
            runtimeABI: file.runtimeABI, readOnly: owner.mode == .snapshot, presentation: presentation, theme: theme,
            descriptor: descriptor)).json, nil)
        } catch { replyHandler(RequestOutcome.page(error), nil) }
      }
    case .windowResize(let r):
      do {
        guard file.isResizable, let delegate, !capturing, !closing, !closed
        else { throw failure("Window resizing unavailable") }
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
      refreshTheme()
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
    case .attachmentsPut, .attachmentsRead:
      servePageStorage(request, reply: replyHandler)
    }
  }

  /// A socket command. The session admits it past its barriers; the owner runs it off the
  /// main actor, so the page keeps focus and composition, and a live `get` returns
  /// owner-accepted state (text only in the DOM is not included).
  public func request(
    _ request: SocketRequest, deadline: NativeCommandDeadline = NativeCommandDeadline()
  ) async -> Data {
    if closing || capturing || closed {
      return RequestOutcome.socket(OwnerError.closing, epoch: epoch).encoded()
    }
    guard case .export(let export) = request else { return await owner.request(request) }
    guard export.epoch == epoch else { return RequestOutcome.socket(OwnerReplaced(), epoch: epoch).encoded() }
    do {
      try deadline.check()
      guard let onExport, isReady, !rendererDead else { throw failure("Export unavailable") }
      try await onExport(export.format, URL(fileURLWithPath: export.output), deadline)
      return SocketReply(ok: true, output: export.output).encoded()
    } catch { return RequestOutcome.socket(error, epoch: epoch).encoded() }
  }

  /// Saves every accepted edit and theme change, and settles the palette on the page,
  /// before an export, a duplicate or a close reads it.
  public func flush() async throws {
    guard isReady, !rendererDead, !closed else { throw failure("Document renderer unavailable") }
    do {
      _ = try await webView.callAsyncJavaScript(
        "await globalThis.__slop.flush(); return true", arguments: [:], in: nil, contentWorld: .page)
    } catch {
      if let failure = try? await owner.currentSaveFailure() { throw failure }
      throw error
    }
    try await settleTheme()
  }

  /// Edit ▸ Undo or Redo. A live page sends its unsent edits first, as the person sees
  /// them; otherwise the owner undoes directly.
  public func undo(redo: Bool = false) async throws {
    guard isReady, !rendererDead, !closed else {
      _ = try await owner.undo(redo: redo)
      return
    }
    _ = try await webView.callAsyncJavaScript(
      redo ? "await globalThis.__slop.redo(); return true" : "await globalThis.__slop.undo(); return true",
      arguments: [:], in: nil, contentWorld: .page)
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
  /// Copies the document to `destination` as a new logical document, with everything the
  /// owner accepted saved first. Never replaces an existing file.
  public func copy(to destination: URL) async throws {
    guard !closed, !closing else { throw failure("Document is closing") }
    try await owner.copy(to: destination)
  }

  /// Whether this session saved any change, so its document's artwork may be out of date.
  public func edited() async -> Bool { await owner.edited() }

  /// Runs one capture of the page at a time. `capturing` stays set across a hand-off to the
  /// next queued capture, so no page or socket request slips in between.
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
    guard !closed, !closing, !capturing else { throw failure("Document is closing or exporting") }
    try await owner.discardPending()
    replaceWebView()
  }

  public func prepareClose() async throws {
    guard !closed, !closePrepared else { return }
    guard !capturing else { throw failure("Document is exporting; try again when it finishes") }
    filePicker.cancel()
    fileSaver.cancel()
    closing = true
    do {
      if isReady && !rendererDead {
        _ = try await webView.callAsyncJavaScript(
          "await globalThis.__slop.prepareClose(); return true", arguments: [:], in: nil,
          contentWorld: .page)
      }
      closePrepared = true
    } catch {
      closing = false
      if let failure = try? await owner.currentSaveFailure() { throw failure }
      throw error
    }
  }

  public func cancelClose() async {
    closePrepared = false
    if isReady && !closed && !rendererDead {
      _ = try? await webView.callAsyncJavaScript(
        "globalThis.__slop.cancelClose(); return true", arguments: [:], in: nil, contentWorld: .page
      )
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
    withdrawDiscovery()
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
    completeWaiters(.failure(failure("Document closed")))
  }

  /// The document is saved and released, so the app's teardown cannot change it: a
  /// failing or unsettled `unmount` is logged after a short grace, never awaited. The
  /// call holds no reference to the page, so an abandoned one is released with the view.
  private func unmountApp(grace: Duration = .seconds(2)) async {
    enum Outcome: Sendable { case settled, failed(String), unsettled }
    let (outcomes, result) = AsyncStream<Outcome>.makeStream()
    webView.callAsyncJavaScript(
      "await globalThis.__slop.close(); return true", arguments: [:], in: nil, in: .page
    ) { reply in
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
      throw failure("Renderer is still active; retry saving instead")
    }
    replaceWebView()
    try await waitUntilReady()
  }

  private func replaceWebView() {
    destroyWebView()
    failureClassification = .platform
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
    let server = try SocketServer { [weak self] request, deadline in
      guard let self else { return RequestOutcome.socket(OwnerError.closed).encoded() }
      return await self.request(request, deadline: deadline)
    }
    self.server = server
    try publishDiscovery()
  }

  /// Names this session's socket in the registry. Snapshot sessions own nothing and never
  /// publish discovery.
  private func publishDiscovery() throws {
    guard let server, owner.mode == .document else { return }
    let discovery = SocketDiscovery(socket: server.path, documentPath: file.url.path)
    try owner.publishDiscovery(try JSONSerialization.data(withJSONObject: discovery.json))
  }

  private func withdrawDiscovery() {
    owner.withdrawDiscovery()
  }

  private func failOpening(_ message: String, reason: SlopFailureContext.Reason = .startup,
                           classification: SlopFailureContext.Classification = .platform) {
    guard !closed, openingError == nil || reason == .webContentTerminated else { return }
    failureClassification = classification
    failureReason = reason
    let wasReady = isReady
    let failed = Renderer.failed(message, terminated: reason == .webContentTerminated)
    phase = closing ? .closing(failed, prepared: false) : .active(failed)
    // The owner is fully loaded before the page starts, so it keeps the writer lock and
    // the socket; a retry reattaches a new page to the same owner.
    if !wasReady { destroyWebView() }
    completeWaiters(.failure(failure(message)))
    delegate?.pageSession(self, didFail: SlopDiagnosticError(
      SlopError.invalid(message), diagnostic: .init(classification, reason: reason)))
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
