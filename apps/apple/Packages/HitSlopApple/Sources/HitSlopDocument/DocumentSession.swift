import AppKit
import Foundation
import HitSlopCore
@preconcurrency import WebKit

public struct DocumentSaveStatus: Sendable {
  public let status: String
  public let failure: SaveFailure?
  public var error: String? { failure?.localizedDescription }
}

/// A package lease outlives its renderer. The native Rust owner interprets Loro bytes.
@MainActor
public final class DocumentSession: NSObject, WKScriptMessageHandlerWithReply, WKNavigationDelegate, WKUIDelegate {
  private var liveWebView: WKWebView?
  public var webView: WKWebView {
    guard let liveWebView else { preconditionFailure("Document WebView has been destroyed") }
    return liveWebView
  }
  public let package: SlopPackage
  /// The owner's epoch: socket clients name it, and it rotates on discard.
  public var epoch: String { owner.epoch }
  public private(set) var isReady = false
  public private(set) var rendererDead = false
  public private(set) var failureReason: SlopFailureContext.Reason?
  public private(set) var failureClassification: SlopFailureContext.Classification = .platform
  public var onResize: ((CGSize) throws -> CGSize)?
  public var onExport: ((String, URL, NativeCommandDeadline) async throws -> Void)?
  public var capturing = false
  public var allowsFileSelection = true {
    didSet { if !allowsFileSelection { filePicker.cancel(); fileSaver.cancel() } }
  }
  var filePicker = DocumentFilePicker()
  var fileSaver = DocumentFileSaver() {
    didSet { configureFileSaver() }
  }
  public var onRecovered: (() -> Void)?
  public var onReady: (() -> Void)?
  public var onStatus: ((DocumentSaveStatus) -> Void)?
  public var onStorageFailure: ((SlopFailureContext) -> Void)?
  public var onIssue: ((String, Bool) -> Void)?
  public var onError: ((String) -> Void)?
  private(set) var owner: DocumentOwner
  private var storage: Storage
  private var server: SocketServer?
  /// The attached page's token (see `makeWebView`).
  private var view = UUID().uuidString
  nonisolated private let pushes = PushQueue()
  private var closing = false
  private var closed = false
  private var closeTask: Task<Void, Error>?
  private var openingError: String?
  private var becameReady = false
  private var waiters: [UUID: CheckedContinuation<Void, Error>] = [:]
  public let webViewResources: URL

  public convenience init(
    package: SlopPackage, storage mode: StorageMode = .document
  ) throws {
    try self.init(prepared: Prepared(package: package, storage: mode))
    try startDiscovery()
  }

  struct Prepared: Sendable {
    let package: SlopPackage
    let shell: URL
    let owner: DocumentOwner
    var storage: Storage { owner.storage }

    init(package: SlopPackage, storage mode: StorageMode = .document) throws {
      self.package = package
      shell = try DocumentSession.pageShell()
      owner = try DocumentOwner(package: package, mode: mode)
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
    package = prepared.package
    webViewResources = prepared.shell
    owner = prepared.owner
    storage = prepared.storage
    super.init()
    observeOwner()
    makeWebView()
  }

  private func observeOwner() {
    owner.onPublication = { [weak self] publication in
      self?.push(#"{"type":"publication","publication":"# + publication + "}")
    }
    owner.onSaveStatus = { [weak self] status, failure, savedSequence in
      if let failure {
        let error = String(decoding: (try? JSONSerialization.data(withJSONObject: failure.localizedDescription, options: .fragmentsAllowed)) ?? Data(#""Save failed""#.utf8), as: UTF8.self)
        self?.push(#"{"type":"failed","error":"# + error + "}")
      } else {
        self?.push(#"{"type":"saved","sequence":\#(savedSequence)}"#)
      }
      Task { @MainActor in
        self?.onStatus?(.init(status: status == "pending" ? "saving" : status, failure: failure))
      }
    }
  }

  /// Called on the owner queue, so pushes enter `pushes` in owner order. One drain at a
  /// time delivers everything buffered in a single awaited call; Swift never parses them.
  nonisolated private func push(_ json: String) {
    guard pushes.append(json) else { return }
    Task { @MainActor [weak self] in await self?.drainPushes() }
  }
  private func drainPushes() async {
    while true {
      let batch = pushes.take()
      if batch.isEmpty { return }
      // A page that has not installed its receiver yet reads these changes when it opens.
      guard !rendererDead, let view = liveWebView else { continue }
      do {
        _ = try await view.callAsyncJavaScript("globalThis.__hitslop?.publish(JSON.parse(payload)); return true",
          arguments: ["payload": "[" + batch.joined(separator: ",") + "]"], in: nil, contentWorld: .page)
      } catch {
        onIssue?("Document changes could not reach the interface: \(error.localizedDescription)", false)
      }
    }
  }

  public static func open(
    packageURL: URL, storage mode: StorageMode = .document
  ) async throws -> DocumentSession {
    let prepared = try await prepare(packageURL: packageURL, storage: mode)
    return try await finishOpening(prepared)
  }

  static func prepare(packageURL: URL, storage mode: StorageMode = .document) async throws -> Prepared {
    try await SlopPreparation.run {
      try SlopLocalDocument.requireLocal(packageURL)
      let package: SlopPackage
      do { package = try SlopPackage(rootURL: packageURL) }
      catch let error as SlopPackageError {
        throw SlopDiagnosticError(error, diagnostic: .init(.rejection, reason: .invalidPackage))
      }
      return try Prepared(package: package, storage: mode)
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
    view = UUID().uuidString
    let configuration = WKWebViewConfiguration()
    configuration.websiteDataStore = .nonPersistent()
    configuration.preferences.javaScriptCanOpenWindowsAutomatically = false
    configuration.setURLSchemeHandler(
      SchemeHandler(root: package.rootURL, shell: webViewResources),
      forURLScheme: "slop")
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
          for (const type of ['error','unhandledrejection']) addEventListener(type,e=> {
            const source = e.target?.src;
            const shellResource = typeof source === 'string' && source.startsWith('slop://app/__shell__/');
            const error = String(shellResource ? 'Could not load page shell resource: ' + source : e.error?.stack ?? e.reason?.stack ?? e.error?.message ?? e.reason ?? e.message).slice(0,4096);
            webkit.messageHandlers.hitslop.postMessage(shellResource ? {method:'failed',error} : {method:'runtimeError',kind:'application',error}).catch(()=>{});
          }, true);
          """, injectionTime: .atDocumentStart, forMainFrameOnly: true))
    let spec = package.manifest.presentation
    let view = WKWebView(
      frame: CGRect(x: 0, y: 0, width: spec.width, height: spec.height),
      configuration: configuration)
    WebViewBackground.set(!package.usesTransparentBackground, on: view)
    view.navigationDelegate = self
    view.uiDelegate = self
    liveWebView = view
    configureFileSaver()
  }

  private func configureFileSaver() {
    fileSaver.window = { [weak self] in self?.liveWebView?.window }
    fileSaver.onFailed = { [weak self] message in self?.onIssue?(message, true) }
  }

  public func load() { liveWebView?.load(URLRequest(url: URL(string: "slop://app/")!)) }

  public func waitUntilReady(timeout: Duration = .seconds(15)) async throws {
    if isReady { return }
    if closed || rendererDead || openingError != nil {
      throw failure(openingError ?? "Document runtime unavailable")
    }
    let id = UUID()
    try await withTaskCancellationHandler {
      try await withCheckedThrowingContinuation { continuation in
        waiters[id] = continuation
        Task { [weak self] in
          try? await Task.sleep(for: timeout)
          self?.waiters.removeValue(forKey: id)?.resume(
            throwing: failure("Document runtime did not become ready"))
        }
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
      let args = message.body as? [String: Any]
    else {
      replyHandler(nil, "Invalid bridge request")
      return
    }
    if let method = args["method"] as? String, ["open", "apply", "text", "flush"].contains(method) {
      guard let bytes = try? JSONSerialization.data(withJSONObject: args), bytes.count <= 4 * 1024 * 1024 else {
        replyHandler(nil, "Invalid owner request"); return
      }
      Task { replyHandler(await owner.bridge(args), nil) }
      return
    }
    guard let request = StorageRequest(args),
      let rawMethod = args["method"] as? String, let method = BridgeMethod(rawValue: rawMethod)
    else {
      replyHandler(nil, "Invalid bridge request")
      return
    }
    switch method {
    case .config:
      let spec = package.manifest.presentation
      replyHandler(
        [
          "epoch": epoch,
          "view": view,
          "documentID": owner.documentID,
          "readOnly": storage.mode == .snapshot,
          "presentation": [
            "width": spec.width, "height": spec.height,
            "resizable": package.isResizable, "shape": package.shape.rawValue,
            "mode": package.isSkinned
              ? "skin" : package.usesTransparentBackground ? "transparent" : "standard",
          ],
        ], nil)
    case .windowResize:
      do {
        guard package.isResizable, let onResize,
          let width = args["width"] as? Double, let height = args["height"] as? Double
        else { throw failure("Window resizing unavailable") }
        let size = try onResize(CGSize(width: width, height: height))
        replyHandler(["width": size.width, "height": size.height], nil)
      } catch { replyHandler(nil, error.localizedDescription) }
    case .ready:
      isReady = true
      becameReady = true
      Task { try? await owner.republishStatus() }
      completeWaiters(.success(()))
      onReady?()
      replyHandler([:], nil)
    case .runtimeRecovered:
      onRecovered?()
      replyHandler([:], nil)
    case .failed, .runtimeError:
      let error = args["error"] as? String ?? "Runtime error"
      if method == .runtimeError && isReady {
        onIssue?(error, args["kind"] as? String == "operation")
      } else {
        let authored = method == .runtimeError
        failOpening(error, reason: authored ? .authoredException : .startup,
                    classification: authored ? .authored : .platform)
      }
      replyHandler([:], nil)
    case .attachmentsPut, .attachmentsRead, .attachmentsList, .themeLoad, .themeSave:
      servePageStorage(request, method: method.rawValue, reply: replyHandler)
    }
  }

  public func request(
    _ request: SocketRequest, deadline: NativeCommandDeadline = NativeCommandDeadline()
  ) async -> SocketReply {
    guard PlatformContract.valid(request.json, against: socketRequestSchema) else {
      return .init(ok: false, error: "Invalid socket request", code: .rejected)
    }
    if closing || capturing {
      return .init(ok: false, epoch: epoch, error: "Document barrier is active", code: .closing)
    }
    guard !closed, request.documentPath == package.rootURL.path else {
      return .init(ok: false, epoch: epoch, error: "Document unavailable or path mismatch", code: .unavailable)
    }
    do {
      try deadline.check()
      if case .export(let export) = request {
        guard let onExport, isReady, !rendererDead, export.epoch == epoch
        else { throw failure("Export unavailable or session changed") }
        try await onExport(export.format.rawValue, URL(fileURLWithPath: export.output), deadline)
        return .init(ok: true, output: export.output)
      }
      if request.requiresEpoch, request.json["epoch"] as? String != epoch {
        return .init(ok: false, epoch: epoch, error: "Owner session changed", code: .sessionChanged)
      }
      // Commands run on the owner directly: the page keeps focus and composition, and a
      // live `get` returns owner-accepted state (text only in the DOM is not included).
      let reply = await owner.request(request)
      if reply.ok, request.method == .themeSet || request.method == .themeReset,
        isReady, !rendererDead, let theme = reply.state as? [String: Any]
      {
        _ = try? await webView.callAsyncJavaScript(
          "globalThis.__slop?.applyTheme?.(overrides); return true",
          arguments: ["overrides": theme["overrides"] ?? [:]], in: nil, contentWorld: .page)
      }
      return reply
    } catch { return .init(ok: false, epoch: epoch, error: error.localizedDescription, code: .failed) }
  }

  public func flush() async throws {
    guard isReady, !rendererDead, !closed else { throw failure("Document renderer unavailable") }
    _ = try await webView.callAsyncJavaScript(
      "await globalThis.__slop.flush(); return true", arguments: [:], in: nil, contentWorld: .page)
  }

  /// Restores the latest durable state under the existing writer lock, after the user chose to.
  /// Retries saving on the owner directly; works whether or not the page is alive.
  public func retrySave() async throws { try await owner.flush() }

  public func discardPending() async throws {
    guard isReady, !rendererDead, !closed else { return }
    try await owner.discardPending()
    destroyWebView(); isReady = false; makeWebView(); load()
    try await waitUntilReady()
  }

  public func prepareClose() async throws {
    guard !closed else { return }
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
    } catch {
      closing = false
      throw error
    }
  }

  public func cancelClose() async {
    if isReady && !closed && !rendererDead {
      _ = try? await webView.callAsyncJavaScript(
        "globalThis.__slop.cancelClose(); return true", arguments: [:], in: nil, contentWorld: .page
      )
    }
    closing = false
  }

  public func close() async throws {
    if let closeTask { return try await closeTask.value }
    let task = Task { try await self.finishClose() }
    closeTask = task
    do { try await task.value } catch {
      closeTask = nil
      throw error
    }
    closeTask = nil
  }

  private func finishClose() async throws {
    if closed { return }
    try await prepareClose()
    do {
      if isReady && !rendererDead {
        _ = try await webView.callAsyncJavaScript(
          "await globalThis.__slop.close(); return true", arguments: [:], in: nil,
          contentWorld: .page)
      }
    } catch {
      await cancelClose()
      throw error
    }
    do { try await owner.close() } catch { await cancelClose(); throw error }
    stopDiscovery()
    // Native callbacks may run while storage releases its writer lease below.
    // Retire the session before tearing down the renderer they would access.
    closed = true
    isReady = false
    destroyWebView()
    completeWaiters(.failure(failure("Document closed")))
  }

  /// The OS writer lease stays owned while replacing a failed renderer.
  public func reopenSavedDocument() async throws {
    guard !closed, !capturing, rendererDead || openingError != nil else {
      throw failure("Renderer is still active; retry saving instead")
    }
    closing = true
    destroyWebView()
    rendererDead = false
    failureClassification = .platform
    failureReason = nil
    openingError = nil
    isReady = false
    closing = false
    makeWebView()
    load()
    try await waitUntilReady()
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
    guard server == nil, storage.mode == .document else { return }
    let server = try SocketServer { [weak self] request, deadline, reply in
      guard let self else {
        reply(.init(ok: false, error: "Document closed", code: .unavailable))
        return
      }
      Task { reply(await self.request(request, deadline: deadline)) }
    }
    self.server = server
    let discovery = SocketDiscovery(socket: server.path, documentPath: package.rootURL.path)
    try JSONSerialization.data(withJSONObject: discovery.json).write(
      to: package.rootURL.appendingPathComponent("state/host.lock"), options: .atomic)
  }

  private func stopDiscovery() {
    server?.stop()
    server = nil
    // Snapshot sessions do not own the package and never publish discovery.
    guard storage.mode == .document else { return }
    try? FileManager.default.removeItem(
      at: package.rootURL.appendingPathComponent("state/host.lock"))
  }

  private func failOpening(_ message: String, reason: SlopFailureContext.Reason = .startup,
                           classification: SlopFailureContext.Classification = .platform) {
    guard !closed, openingError == nil else { return }
    failureClassification = classification
    failureReason = reason
    openingError = message
    // The owner is fully loaded before the page starts, so it keeps the writer lock and
    // the socket; a retry reattaches a new page to the same owner.
    if !becameReady { destroyWebView() }
    completeWaiters(.failure(failure(message)))
    onError?(message)
  }

  public func webViewWebContentProcessDidTerminate(_ webView: WKWebView) {
    filePicker.cancel()
    fileSaver.cancel()
    rendererDead = true
    isReady = false
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
    if action.navigationType == .linkActivated, let url = action.request.url,
      ["https", "http"].contains(url.scheme)
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

/// Pushes waiting for delivery. `append` reports whether a drain must start; `take`
/// empties the queue and, once empty, lets the next append start a drain.
final class PushQueue: @unchecked Sendable {
  private let lock = NSLock()
  private var items: [String] = []
  private var draining = false
  func append(_ json: String) -> Bool {
    lock.withLock {
      items.append(json)
      if draining { return false }
      draining = true
      return true
    }
  }
  func take() -> [String] {
    lock.withLock {
      let batch = items
      items.removeAll()
      if batch.isEmpty { draining = false }
      return batch
    }
  }
}
