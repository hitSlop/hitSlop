import AppKit
import HitSlopCore
import HitSlopRuntime
import HitSlopWasm
import SwiftUI
import UniformTypeIdentifiers
import WebKit

private final class FramelessDocumentWindow: NSWindow {
  override var canBecomeKey: Bool { true }
  override var canBecomeMain: Bool { true }
  override func performMiniaturize(_ sender: Any?) { miniaturize(sender) }
  override func performClose(_ sender: Any?) {
    if delegate?.windowShouldClose?(self) ?? true { close() }
  }
  override func validateMenuItem(_ menuItem: NSMenuItem) -> Bool {
    // AppKit disables Close for borderless windows despite our custom
    // performClose implementation and asynchronous save-on-close delegate.
    if menuItem.action == #selector(NSWindow.performClose(_:)) { return true }
    return super.validateMenuItem(menuItem)
  }
}

func slopDocumentWindowStyleMask(resizable: Bool) -> NSWindow.StyleMask {
  var mask: NSWindow.StyleMask = [.borderless, .miniaturizable]
  if resizable { mask.insert(.resizable) }
  return mask
}

func slopDockMenuImage(iconURL: URL, fallbackURL: URL) -> NSImage {
  let source = NSImage(contentsOf: iconURL) ?? NSWorkspace.shared.icon(forFile: fallbackURL.path)
  let image = (source.copy() as? NSImage) ?? source
  image.size = NSSize(width: 16, height: 16)
  return image
}

func dynamicSlopWindowFrame(current: NSRect, requested: NSSize, visible: NSRect?) -> NSRect {
  let size = NSSize(
    width: min(requested.width, visible?.width ?? requested.width),
    height: min(requested.height, visible?.height ?? requested.height)
  )
  var origin = NSPoint(x: current.minX, y: current.maxY - size.height)
  if let visible {
    origin.x = min(max(origin.x, visible.minX), visible.maxX - size.width)
    origin.y = min(max(origin.y, visible.minY), visible.maxY - size.height)
  }
  return NSRect(origin: origin, size: size)
}

private class HoverView: NSView {
  var changed: ((Bool) -> Void)?
  private var area: NSTrackingArea?
  override func updateTrackingAreas() {
    if let area { removeTrackingArea(area) }
    let next = NSTrackingArea(
      rect: bounds, options: [.mouseEnteredAndExited, .activeAlways, .inVisibleRect], owner: self)
    addTrackingArea(next)
    area = next
    super.updateTrackingAreas()
  }
  override func mouseEntered(with event: NSEvent) { changed?(true) }
  override func mouseExited(with event: NSEvent) { changed?(false) }
}

private final class ShapedView: HoverView {
  let windowMask: SlopWindowMask
  private let maskLayer: CALayer
  init(frame: NSRect, windowMask: SlopWindowMask) {
    self.windowMask = windowMask
    maskLayer = windowMask.makeLayer()
    super.init(frame: frame)
    wantsLayer = true
    windowMask.installBacking(on: layer)
    layer?.mask = maskLayer
  }
  required init?(coder: NSCoder) { nil }
  override func layout() {
    super.layout()
    windowMask.update(maskLayer, bounds: bounds)
  }
  override func hitTest(_ point: NSPoint) -> NSView? {
    windowMask.contains(point, in: bounds) ? super.hitTest(point) : nil
  }
}

@MainActor enum SlopDocumentAssetRefreshQueue {
  private struct Job {
    let destination: URL
    let generation: UUID
    let telemetry: SlopTelemetry
  }
  private static var generations: [URL: UUID] = [:]
  private static var pending: [Job] = []
  private static var worker: Task<Void, Never>?
  private static var rendering = false

  static func invalidate(_ url: URL) {
    let key = url.standardizedFileURL
    generations[key] = UUID()
    pending.removeAll { $0.destination == key }
  }
  static func schedule(presentedURL: URL, telemetry: SlopTelemetry = .disabled) {
    let key = presentedURL.standardizedFileURL
    invalidate(key)
    let generation = generations[key]!
    pending.append(Job(destination: key, generation: generation, telemetry: telemetry))
    startWorkerIfNeeded()
  }
  private static func startWorkerIfNeeded() {
    guard worker == nil, !pending.isEmpty else { return }
    worker = Task { @MainActor in
      defer {
        rendering = false
        worker = nil
        startWorkerIfNeeded()
      }
      while !pending.isEmpty, !Task.isCancelled {
        let job = pending.removeFirst()
        rendering = true
        defer { rendering = false }
        do {
          let assets = try await SlopRenderer.documentAssetsPNGData(packageURL: job.destination, telemetry: job.telemetry)
          guard !Task.isCancelled, generations[job.destination] == job.generation else { continue }
          if let preview = assets.previewPNG {
            do { try SlopPreviewWriter.write(preview, to: job.destination) } catch {
              job.telemetry.failure(.artwork, error: error)
            }
          }
          if let icon = assets.finderIconPNG {
            await SlopPreviewWriter.installFinderIconAsync(
              icon, for: job.destination,
              isCurrent: { generations[job.destination] == job.generation && !Task.isCancelled },
              telemetry: job.telemetry)
          }
        } catch {
          if !Task.isCancelled, generations[job.destination] == job.generation { job.telemetry.failure(.artwork, error: error) }
        }
      }
    }
  }
  static func finishForTermination(grace: Duration = .seconds(5)) async {
    // Do not start another WebView while quitting. Its synchronous construction
    // could consume the entire grace period before the timer gets to run.
    pending.removeAll()
    if !rendering {
      worker?.cancel()
      generations.removeAll()
      return
    }
    let deadline = ContinuousClock.now.advanced(by: grace)
    while worker != nil, ContinuousClock.now < deadline {
      try? await Task.sleep(for: .milliseconds(40))
    }
    if worker != nil {
      worker?.cancel()
      pending.removeAll()
      generations.removeAll()
    }
  }
}

@MainActor
public final class SlopDocumentWindowController: NSWindowController, NSWindowDelegate,
  SlopRuntimeSessionDelegate
{
  public let packageURL: URL
  public let session: SlopRuntimeSession
  public var onClose: (() -> Void)?
  public var onCommand: ((SlopDocumentCommand) -> Void)?
  public var onRuntimeReady: (() -> Void)?
  public var telemetry: SlopTelemetry = .disabled
  private var reportedSaveFailure = false
  private var reportedRendererFailure = false
  private var reportedGuestSources = Set<SlopRuntimeIssue.Source>()
  public var onRuntimeFailure: ((String) -> Void)?
  private var toolbar: NSPanel?, toolbarHost: NSHostingView<SlopToolbar>?
  private var toolbarMenuTracking = false
  private var toolbarInteracting = false
  private var toolbarVisibility = SlopToolbarVisibility()
  private weak var controlsWebView: WKWebView?
  private var publishedControlsVisible: Bool?
  private var failedOverlay: NSHostingView<FailureOverlay>?
  private var presentedRuntimeError: String?
  private var documentAttention: NSPanel?
  private var attentionIsSaveFailure = false
  private var attentionMessage: String?
  private var commandsEnabled = true
  private(set) var openingProgress: SlopOpeningProgress?
  private(set) var isLoading = false
  private var presentationRequested = false
  public private(set) var isContentReady = false
  private var loadingTask: Task<Void, Never>?
  private weak var loadingWebView: NSView?
  private let startupStarted: ContinuousClock.Instant
  private lazy var availableEditors = installedEditors()

  public convenience init(packageURL: URL) throws {
    let started = ContinuousClock.now
    try self.init(packageURL: packageURL, session: SlopRuntimeSession(packageURL: packageURL), started: started)
  }

  private static var preparingProgress: [URL: SlopOpeningProgress] = [:]

  public static func focusOpeningDocument(at url: URL) {
    preparingProgress[url.standardizedFileURL]?.focus()
  }

  public static func open(packageURL: URL, presentsWindow: Bool = false, telemetry: SlopTelemetry = .disabled) async throws -> SlopDocumentWindowController {
    let started = ContinuousClock.now
    let progress = presentsWindow ? SlopOpeningProgress(started: started) : nil
    let key = packageURL.standardizedFileURL
    if let progress { preparingProgress[key] = progress }
    defer { if preparingProgress[key] === progress { preparingProgress[key] = nil } }
    let preparation = Task { @MainActor in
      let session = try await SlopRuntimeSession.open(packageURL: packageURL)
      do {
        try Task.checkCancellation()
        return try SlopDocumentWindowController(packageURL: packageURL, session: session, started: started, telemetry: telemetry)
      } catch {
        try await session.finish()
        throw error
      }
    }
    progress?.onCancel = { preparation.cancel() }
    do {
      let controller = try await withTaskCancellationHandler {
        try await preparation.value
      } onCancel: { preparation.cancel() }
      if Task.isCancelled || preparation.isCancelled {
        try await controller.finishClose()
        throw CancellationError()
      }
      if presentsWindow {
        controller.openingProgress = progress
        controller.showWindow(nil)
      }
      return controller
    } catch {
      progress?.finish()
      throw error
    }
  }

  private init(packageURL: URL, session: SlopRuntimeSession, started: ContinuousClock.Instant, telemetry: SlopTelemetry = .disabled) throws {
    self.telemetry = telemetry
    startupStarted = started
    self.packageURL = packageURL.standardizedFileURL
    self.session = session
    // Start WebKit before building native chrome; bridge messages arrive only
    // after this initializer returns to the run loop.
    session.load()
    let windowMask = try SlopWindowMask(package: session.package)
    let spec = session.package.manifest.presentation
    let size = NSSize(width: spec.width, height: spec.height)
    let window = FramelessDocumentWindow(
      contentRect: NSRect(origin: .zero, size: size),
      styleMask: slopDocumentWindowStyleMask(resizable: session.package.isResizable),
      backing: .buffered, defer: false)
    window.title = SlopDocumentIdentity(url: self.packageURL).filename
    window.minSize = NSSize(width: 240, height: 180)
    window.isOpaque = false
    window.backgroundColor = .clear
    window.hasShadow = !session.package.usesTransparentBackground || session.package.isSkinned
    window.isReleasedWhenClosed = false
    window.tabbingMode = .disallowed
    window.representedURL = self.packageURL
    window.miniwindowTitle = window.title
    window.miniwindowImage = NSImage(contentsOf: session.package.iconURL)
    if session.package.shape == .ellipse, spec.width == spec.height {
      window.contentAspectRatio = NSSize(width: 1, height: 1)
    }
    let container = ShapedView(frame: NSRect(origin: .zero, size: size), windowMask: windowMask)
    session.webView.frame = container.bounds
    session.webView.autoresizingMask = [.width, .height]
    container.addSubview(session.webView)
    window.contentView = container
    window.center()
    super.init(window: window)
    window.delegate = self
    session.delegate = self
    SlopRenderer.installCLIExport(on: session,
      telemetry: SlopTelemetry { [weak self] in self?.telemetry.send($0) },
      onFailure: { [weak self] error, format in self?.reportLifecycleFailure(.export, error: error, format: format) })
    container.changed = { [weak self] _ in self?.refreshToolbarHover() }
    SlopToolbarPointerSampler.shared.add(self, window: window) { [weak self] point, front in
      self?.refreshToolbarHover(point: point, front: front)
    }
    SlopDocumentAssetRefreshQueue.invalidate(self.packageURL)
    startLoading()
    recordStartup("native-prepared")
    // Finder icon metadata is cosmetic; keep its disk writes off the opening path.
    Task { @MainActor [weak self, package = session.package] in
      await self?.waitForPresentation()
      guard self?.isContentReady == true else { return }
      SlopPreviewWriter.installExistingPreview(for: package, telemetry: self?.telemetry ?? .disabled)
    }
  }
  required init?(coder: NSCoder) { nil }

  deinit {
    loadingTask?.cancel()
    Task { @MainActor [progress = openingProgress] in progress?.finish() }
  }

  public func runtimeSessionDidBecomeReady(_ session: SlopRuntimeSession) {
    publishControlsVisibility(toolbar?.isVisible == true, force: true)
    if reportedRendererFailure { telemetry.send(.breadcrumb(.renderer, .recovered)) }
    reportedRendererFailure = false
    recordStartup("runtime-ready")
    #if DEBUG
    if ProcessInfo.processInfo.environment["HITSLOP_STARTUP_TIMINGS"] == "1" {
      // Page-relative milliseconds for the hitslop:* marks recorded by mountDocument.
      session.webView.evaluateJavaScript(
        "JSON.stringify(performance.getEntriesByType('mark').map(e => [e.name, Math.round(e.startTime)]))"
      ) { result, _ in print("[hitSlop startup] page \(result ?? "")") }
    }
    #endif
    // The bridge is ready before WebKit has necessarily painted. The loading
    // task owns the visual handoff and the coordinator's ready notification.
  }

  private func recordStartup(_ stage: String) {
    #if DEBUG
    if ProcessInfo.processInfo.environment["HITSLOP_STARTUP_TIMINGS"] == "1" {
      print("[hitSlop startup] \(stage) \(startupStarted.duration(to: .now))")
    }
    #endif
  }

  private func startLoading() {
    stopLoading()
    isContentReady = false
    isLoading = true
    window?.orderOut(nil)
    hideToolbar()
    loadingWebView = session.webView
    session.webView.setAccessibilityHidden(true)
    toolbarHost?.rootView = toolbarView()
    if presentationRequested { showOpeningProgress() }
    loadingTask = Task { @MainActor [weak self, session, weak view = session.webView] in
      do {
        try await session.waitUntilReady()
        try Task.checkCancellation()
        guard view != nil, session.isReady else { return }
        // Each call is bounded so cancelling an open never retains the WebView in a
        // long-lived font promise. Layout starts newly mounted fonts.
        let deadline = ContinuousClock.now.advanced(by: .seconds(15))
        while true {
          guard let current = view else { return }
          let loaded = try await current.callAsyncJavaScript("""
            document.body.getBoundingClientRect();
            await Promise.race([document.fonts.ready, new Promise(resolve => setTimeout(resolve, 250))]);
            return document.fonts.status === 'loaded';
            """, arguments: [:], in: nil, contentWorld: .page) as? Bool == true
          try Task.checkCancellation()
          if loaded { break }
          guard ContinuousClock.now < deadline else {
            throw SlopPackageError.invalid("Document fonts did not become ready")
          }
        }
        // The runtime has mounted and fonts have settled. Let the visible window paint
        // normally instead of waiting for animation frames in an ordered-out WebView.
        self?.recordStartup("fonts-ready")
        try Task.checkCancellation()
        guard let self, self.isLoading else { return }
        self.isContentReady = true
        self.finishLoading()
        if self.presentationRequested { self.revealReadyWindow() }
        if let onRuntimeReady = self.onRuntimeReady { onRuntimeReady() }
        else { self.updateRuntimeFailure(nil) }
      } catch is CancellationError {
      } catch {
        guard !Task.isCancelled else { return }
        self?.runtimeSession(session, didFail: error)
      }
    }
  }

  private func finishLoading() {
    isLoading = false
    openingProgress?.finish()
    openingProgress = nil
    if let view = loadingWebView { view.setAccessibilityHidden(false) }
    loadingWebView = nil
    toolbarHost?.rootView = toolbarView()
  }

  private func stopLoading() {
    loadingTask?.cancel()
    loadingTask = nil
    finishLoading()
  }

  public override func showWindow(_ sender: Any?) {
    presentationRequested = true
    if isContentReady || presentedRuntimeError != nil {
      openingProgress?.finish()
      openingProgress = nil
      revealReadyWindow()
    } else { showOpeningProgress() }
  }

  private func showOpeningProgress() {
    if openingProgress == nil { openingProgress = SlopOpeningProgress(started: startupStarted) }
    openingProgress?.onCancel = { [weak self] in
      self?.loadingTask?.cancel()
      self?.request(.close)
    }
    openingProgress?.focus()
  }

  private func revealReadyWindow() {
    super.showWindow(nil)
    window?.deminiaturize(nil)
    window?.makeKeyAndOrderFront(nil)
    if isContentReady {
      window?.makeFirstResponder(session.webView)
      recordStartup("content-visible")
    }
  }

  func waitForPresentation() async { await loadingTask?.value }
  public func runtimeSession(_ session: SlopRuntimeSession, resizeContentTo requested: CGSize)
    throws -> CGSize
  {
    guard let window else { throw SlopPackageError.invalid("document window is unavailable") }
    let frame = dynamicSlopWindowFrame(
      current: window.frame, requested: requested,
      visible: window.screen?.visibleFrame ?? NSScreen.main?.visibleFrame)
    window.setFrame(frame, display: true, animate: true)
    if toolbar?.isVisible == true { showToolbar() }
    return frame.size
  }
  public func runtimeSession(_ session: SlopRuntimeSession, didFail error: Error) {
    isContentReady = false
    stopLoading()
    if !reportedRendererFailure {
      reportedRendererFailure = true
      let diagnostic = (error as? any SlopDiagnosticProviding)?.diagnostic
        ?? SlopFailureContext(reason: session.engine.failureReason ?? .presentation)
      telemetry.send(.breadcrumb(.renderer, .failed))
      var context = diagnostic
      context.runtime = session.engine.telemetryRuntime
      if !reportedSaveFailure || context.reason == .webContentTerminated { telemetry.send(.failed(.renderer, context)) }
    }
    if let onRuntimeFailure {
      onRuntimeFailure(error.localizedDescription)
    } else {
      updateRuntimeFailure(error.localizedDescription)
    }
  }

  public func runtimeSession(_ session: SlopRuntimeSession, didReport issue: SlopRuntimeIssue) {
    if reportedGuestSources.insert(issue.source).inserted {
      let rejection = issue.source == .document
      telemetry.send(.failed(.renderer, .init(rejection ? .rejection : .authored,
        reason: rejection ? .operationRejected : .authoredException, runtime: session.engine.telemetryRuntime)))
    }
    guard guestIssue?.message != issue.message else { return }
    guestIssue = issue
    showDocumentAttention()
  }
  public func runtimeSession(_ session: SlopRuntimeSession, storageFailure: SlopFailureContext) {
    guard !reportedSaveFailure else { return }
    reportedSaveFailure = true
    var context = storageFailure
    context.runtime = session.engine.telemetryRuntime
    telemetry.send(.breadcrumb(.save, .failed))
    telemetry.send(.failed(.save, context))
  }

  public func runtimeSession(_ session: SlopRuntimeSession, saveStatus: WasmSaveStatus) {
    if saveStatus.status == "save-failed" {
      runtimeSession(session, storageFailure: .init(reason: .storage))
    } else if saveStatus.status == "saved" {
      if reportedSaveFailure { telemetry.send(.breadcrumb(.save, .recovered)) }
      reportedSaveFailure = false
    }
    window?.isDocumentEdited = saveStatus.status != "saved"
    if let message = saveStatus.error {
      attentionMessage = message
      showDocumentAttention()
    } else if saveStatus.status == "saved" {
      attentionMessage = nil
      if attentionIsSaveFailure, let panel = documentAttention {
        window?.endSheet(panel, returnCode: .abort)
        panel.orderOut(nil)
      }
    }
  }
  private var guestIssue: SlopRuntimeIssue?
  public func runtimeSessionRecovered(_ session: SlopRuntimeSession) {
    if guestIssue != nil { telemetry.send(.breadcrumb(.recovery, .recovered)) }
    guestIssue = nil
    publishControlsVisibility(toolbar?.isVisible == true, force: true)
  }
  private func showDocumentAttention() {
    guard let message = attentionMessage ?? guestIssue?.message else { return }
    let saving = attentionMessage != nil
    let alert = NSAlert()
    alert.messageText = saving ? "Changes could not be saved" : "This slop encountered an error"
    alert.informativeText = message
    alert.addButton(withTitle: saving ? "Retry Save" : "Reload Interface")
    // Unsaved work stays live; offer an explicit way back to the durable state.
    let full = saving && Self.isCapacityFailure(message)
    if full { alert.addButton(withTitle: "Discard Unsaved Edits") }
    if saving { alert.addButton(withTitle: "Keep Open") }
    if !saving {
      alert.addButton(withTitle: "Dismiss")
      alert.addButton(withTitle: "Copy Details")
    }
    if let window, window.attachedSheet == nil {
      documentAttention = alert.window as? NSPanel
      attentionIsSaveFailure = saving
      alert.beginSheetModal(for: window) { [weak self] result in
        guard let self else { return }
        self.documentAttention = nil
        self.attentionIsSaveFailure = false
        if result == .alertFirstButtonReturn {
          Task {
            do {
              if saving {
                try await self.session.flush()
                self.attentionMessage = nil
                self.showDocumentAttention()
              } else {
                try await self.session.engine.reloadInterface()
              }
            } catch {
              if saving {
                self.attentionMessage = error.localizedDescription
              } else {
                self.guestIssue = SlopRuntimeIssue(
                  source: .unhandled, message: error.localizedDescription)
              }
              self.showDocumentAttention()
            }
          }
        } else if full && result == .alertSecondButtonReturn {
          Task {
            do {
              try await self.session.engine.discardPending()
              try await self.session.flush()
              self.attentionMessage = nil
            } catch {
              self.attentionMessage = error.localizedDescription
              self.showDocumentAttention()
            }
          }
        } else if !saving && result == .alertThirdButtonReturn {
          NSPasteboard.general.clearContents()
          NSPasteboard.general.setString(message, forType: .string)
          if self.attentionMessage != nil { self.showDocumentAttention() }
        } else if !saving && (result == .abort || self.attentionMessage != nil) {
          self.showDocumentAttention()
        }
      }
    }
  }

  private func setupToolbar() {
    let frame = slopToolbarFrame(document: window?.frame ?? .zero, visible: window?.screen?.visibleFrame)
    let panel = SlopToolbarPanel(
      contentRect: frame,
      styleMask: [.borderless, .nonactivatingPanel], backing: .buffered, defer: false)
    panel.isOpaque = false
    panel.backgroundColor = .clear
    panel.hasShadow = true
    panel.level = .floating
    panel.hidesOnDeactivate = false
    panel.isReleasedWhenClosed = false
    panel.isExcludedFromWindowsMenu = true
    let tracking = HoverView(
      frame: panel.contentView?.bounds ?? NSRect(origin: .zero, size: frame.size))
    panel.contentView = tracking
    let host = NSHostingView(rootView: toolbarView())
    host.frame = tracking.bounds
    host.autoresizingMask = [.width, .height]
    tracking.addSubview(host)
    tracking.changed = { [weak self] _ in
      self?.refreshToolbarHover()
    }
    window?.addChildWindow(panel, ordered: .above)
    panel.orderOut(nil)
    toolbar = panel
    toolbarHost = host
    panel.drag = { [weak self] in self?.dragWindow(with: $0) }
    panel.interactionChanged = { [weak self] active in
      self?.toolbarInteracting = active
      self?.refreshToolbarHover()
    }
  }
  private func toolbarView() -> SlopToolbar {
    SlopToolbar(
      identity: SlopDocumentIdentity(url: packageURL),
      menuTrackingChanged: { [weak self] tracking in
        guard let self else { return }
        guard !tracking || toolbar?.isVisible == true else { return }
        toolbarMenuTracking = tracking
        refreshToolbarHover()
      },
      drag: { [weak self] event in self?.dragWindow(with: event) }, pinned: isPinned,
      commandsEnabled: commandsEnabled && isContentReady, close: { [weak self] in self?.request(.close) },
      minimize: { [weak self] in self?.miniaturizeFromToolbar() },
      pin: { [weak self] in self?.request(.pin(!(self?.isPinned ?? false))) },
      duplicate: { [weak self] in self?.request(.duplicate) },
      png: { [weak self] in self?.request(.exportPNG) },
      pdf: { [weak self] in self?.request(.exportPDF) },
      reveal: { [weak self] in self?.request(.reveal) },
      copyPath: { [weak self] in self?.request(.copyPath) }, editors: availableEditors,
      openEditor: { [weak self] url in self?.request(.openEditor(url)) })
  }
  private func dragWindow(with event: NSEvent) {
    window?.performDrag(with: event)
    showToolbar()
  }
  private func miniaturizeFromToolbar() {
    hideToolbar()
    window?.miniaturize(nil)
  }
  private func showToolbar() {
    guard let window, window.isVisible, !window.isMiniaturized else { return }
    if toolbar == nil { setupToolbar() }
    guard let panel = toolbar else { return }
    panel.setFrame(slopToolbarFrame(document: window.frame, visible: window.screen?.visibleFrame), display: true)
    panel.orderFrontRegardless()
    publishControlsVisibility(true)
  }

  private func hideToolbar() {
    toolbar?.orderOut(nil)
    publishControlsVisibility(false)
  }

  /// One native hover decision drives both the toolbar and opt-in authored controls.
  /// Publish transitions only; the pointer sampler must not send JavaScript every tick.
  private func publishControlsVisibility(_ visible: Bool, force: Bool = false) {
    guard session.isReady, !session.engine.rendererDead else {
      publishedControlsVisible = nil
      return
    }
    let view = session.webView
    guard force || controlsWebView !== view || publishedControlsVisible != visible else { return }
    controlsWebView = view
    publishedControlsVisible = visible
    view.evaluateJavaScript(
      "document.documentElement.setAttribute('data-slop-controls', '\(visible ? "visible" : "hidden")')",
      in: nil, in: .defaultClient
    ) { [weak self, weak view] result in
      // A failed delivery can be retried by the next sample, without an old
      // renderer's completion invalidating the replacement view's state.
      if case .failure = result, let self, self.controlsWebView === view,
         self.publishedControlsVisible == visible {
        self.publishedControlsVisible = nil
      }
    }
  }

  func refreshToolbarHover(point: NSPoint = NSEvent.mouseLocation, front: Int? = nil,
                           now: TimeInterval = ProcessInfo.processInfo.systemUptime) {
    guard let window else { return }
    guard window.isVisible, !window.isMiniaturized, window.isOnActiveSpace, !NSApp.isHidden, !isLoading else {
      toolbarVisibility = SlopToolbarVisibility()
      hideToolbar()
      return
    }
    let front = front ?? NSWindow.windowNumber(at: point, belowWindowWithWindowNumber: 0)
    let overDocument: Bool
    if front == window.windowNumber, let shaped = window.contentView as? ShapedView {
      let local = shaped.convert(window.convertPoint(fromScreen: point), from: nil)
      overDocument = shaped.windowMask.contains(local, in: shaped.bounds)
    } else { overDocument = false }
    let toolbarFrame = toolbar?.frame ?? .zero
    let overToolbar = (toolbar?.isVisible == true) && front == toolbar?.windowNumber
    let gap = NSRect(x: max(window.frame.minX, toolbarFrame.minX), y: window.frame.maxY,
                     width: max(0, min(window.frame.maxX, toolbarFrame.maxX) - max(window.frame.minX, toolbarFrame.minX)),
                     height: max(0, toolbarFrame.minY - window.frame.maxY))
    let nearToolbar = (toolbar?.isVisible == true) &&
      (toolbarFrame.insetBy(dx: -4, dy: -4).contains(point) || gap.contains(point)) &&
      (front == 0 || front == window.windowNumber || front == toolbar?.windowNumber)
    let show = toolbarVisibility.shouldShow(
      inside: overDocument || overToolbar || nearToolbar,
      interacting: toolbarMenuTracking || toolbarInteracting,
      visible: (toolbar?.isVisible == true), now: now)
    if show {
      if !(toolbar?.isVisible == true) { showToolbar() }
    } else { hideToolbar() }
    publishControlsVisibility(toolbar?.isVisible == true)
  }
  public var isPinned: Bool { window?.level == .floating }
  public var documentTitle: String { window?.title ?? SlopDocumentIdentity(url: packageURL).filename }
  public var dockMenuImage: NSImage {
    slopDockMenuImage(iconURL: session.package.iconURL, fallbackURL: packageURL)
  }
  public func owns(_ candidate: NSWindow?) -> Bool {
    guard let candidate else { return false }
    return candidate === window || candidate === toolbar || candidate === openingProgress?.panel
  }
  public func updatePresentation(pinned: Bool, commandsEnabled: Bool, runtimeError: String?) {
    updateRuntimeFailure(runtimeError)
    guard isPinned != pinned || self.commandsEnabled != commandsEnabled else { return }
    window?.level = pinned ? .floating : .normal
    self.commandsEnabled = commandsEnabled
    toolbarHost?.rootView = toolbarView()
  }
  private func request(_ command: SlopDocumentCommand) {
    if !isContentReady {
      switch command {
      case .exportPNG, .exportPDF, .duplicate: return
      default: break
      }
    }
    if let onCommand {
      onCommand(command)
      return
    }
    Task {
      do { _ = try await perform(command) } catch {
        present("Could not complete command", error)
      }
    }
  }
  public func perform(_ command: SlopDocumentCommand) async throws -> URL? {
    switch command {
    case .pin(let pinned):
      window?.level = pinned ? .floating : .normal
      toolbarHost?.rootView = toolbarView()
    case .duplicate:
      let panel = NSSavePanel()
      panel.allowedContentTypes = [.slop]
      panel.nameFieldStringValue =
        packageURL.deletingPathExtension().lastPathComponent + " copy.slop"
      panel.directoryURL = FileManager.default.urls(for: .desktopDirectory, in: .userDomainMask)[0]
      let response = await withCheckedContinuation { continuation in
        if let window {
          panel.beginSheetModal(for: window) { continuation.resume(returning: $0) }
        } else {
          panel.begin { continuation.resume(returning: $0) }
        }
      }
      return try await duplicateDocument(to: response == .OK ? panel.url : nil)

    case .exportPNG: try await export(.png)
    case .exportPDF: try await export(.pdf)
    case .reveal: reveal()
    case .copyPath: copyPath()
    case .openEditor(let app): try await openInEditor(app)
    case .retry:
      telemetry.send(.breadcrumb(.recovery, .started))
      do {
        if session.isReady && !session.engine.rendererDead {
          try await session.engine.reloadInterface()
        } else {
          try await session.reopenSavedDocument()
        }
        if let content = window?.contentView {
          session.webView.frame = content.bounds
          session.webView.autoresizingMask = [.width, .height]
          content.addSubview(session.webView, positioned: .below, relativeTo: failedOverlay)
        }
        updateRuntimeFailure(nil)
        startLoading()
        telemetry.send(.breadcrumb(.recovery, .completed))
      } catch { reportLifecycleFailure(.recovery, error: error); throw error }
    case .close:
      try await prepareToClose()
      do { try await finishClose() } catch {
        if isLoading { startLoading() }
        throw error
      }
    }
    return nil
  }
  public func revealFromDock() { showWindow(nil) }

  func duplicateDocument(to target: URL?) async throws -> URL? {
    guard let target else { telemetry.send(.breadcrumb(.duplicate, .cancelled)); return nil }
    telemetry.send(.breadcrumb(.duplicate, .started))
    do { try await session.flush() }
    catch { reportLifecycleFailure(.duplicate, error: error); throw error }
    do {
      let source = session.package.rootURL
      let destination = try await SlopPreparation.run { try SlopDuplicator.duplicate(from: source, to: target) }
      telemetry.send(.breadcrumb(.duplicate, .completed))
      return destination
    } catch { telemetry.failure(.duplicate, error: error, runtime: session.engine.telemetryRuntime); throw error }
  }

  /// Save status and renderer callbacks own their incidents; outer operations add only context.
  private func reportLifecycleFailure(_ operation: SlopTelemetryEvent.Failure, error: Error,
                                      format: SlopTelemetryEvent.ExportFormat? = nil) {
    if SlopFailureContext.isCancellation(error) { telemetry.send(.breadcrumb(operation, .cancelled)); return }
    if reportedSaveFailure || reportedRendererFailure { telemetry.send(.breadcrumb(operation, .failed)); return }
    telemetry.failure(operation, error: error, runtime: session.engine.telemetryRuntime, format: format)
  }

  public func finishClose(operation: SlopTelemetryEvent.Failure = .close) async throws {
    guard !closePrepared else { return }
    do {
      try await session.finish()
      closePrepared = true
      window?.close()
      telemetry.send(.breadcrumb(operation, .completed))
    } catch { reportLifecycleFailure(operation, error: error); throw error }
  }

  private func export(_ format: SlopTelemetryEvent.ExportFormat) async throws {
    let panel = NSSavePanel()
    panel.allowedContentTypes = [format == .png ? .png : .pdf]
    panel.nameFieldStringValue = packageURL.deletingPathExtension().lastPathComponent + "." + format.rawValue
    let output = panel.runModal() == .OK ? panel.url : nil
    try await exportDocument(format: format, to: output)
  }

  /// A cancelled picker has no output and emits no success event.
  func exportDocument(format: SlopTelemetryEvent.ExportFormat, to output: URL?) async throws {
    guard let output else { telemetry.send(.breadcrumb(.export, .cancelled)); return }
    telemetry.send(.breadcrumb(.export, .started))
    await waitForPresentation()
    do {
      guard isContentReady, session.isReady, presentedRuntimeError == nil else {
        throw SlopPackageError.invalid("The document is not ready to export")
      }
      try await SlopRenderer.exportDocument(session: session, format: format.rawValue, output: output)
      telemetry.send(.breadcrumb(.export, .completed))
      telemetry.send(.exported(format))
    } catch {
      reportLifecycleFailure(.export, error: error, format: format)
      throw error
    }
  }
  private func reveal() { NSWorkspace.shared.activateFileViewerSelecting([packageURL]) }
  private func copyPath() {
    NSPasteboard.general.clearContents()
    NSPasteboard.general.setString(packageURL.path, forType: .string)
  }
  private func installedEditors() -> [(String, URL)] {
    slopOpenInCatalog().compactMap { title, bundleID, fallbackPath in
      if let url = NSWorkspace.shared.urlForApplication(withBundleIdentifier: bundleID) {
        return (title, url)
      }
      return FileManager.default.fileExists(atPath: fallbackPath)
        ? (title, URL(fileURLWithPath: fallbackPath)) : nil
    }
  }
  private func openInEditor(_ app: URL) async throws {
    let directory = URL(fileURLWithPath: packageURL.path, isDirectory: true)
    _ = try await NSWorkspace.shared.open(
      [directory], withApplicationAt: app, configuration: NSWorkspace.OpenConfiguration())
  }

  private func updateRuntimeFailure(_ message: String?) {
    guard presentedRuntimeError != message else { return }
    presentedRuntimeError = message
    failedOverlay?.removeFromSuperview()
    failedOverlay = nil
    guard let message, let content = window?.contentView else { return }
    stopLoading()
    if session.engine.rendererDead, let panel = documentAttention {
      window?.endSheet(panel)
      panel.orderOut(nil)
      documentAttention = nil
      attentionMessage = nil
    }
    let overlay = NSHostingView(
      rootView: FailureOverlay(message: message, retry: { [weak self] in self?.request(.retry) }))
    overlay.frame = content.bounds
    overlay.autoresizingMask = [.width, .height]
    content.addSubview(overlay)
    failedOverlay = overlay
    if presentationRequested { revealReadyWindow() }
  }
  public func windowDidMove(_ notification: Notification) {
    if toolbar?.isVisible == true { showToolbar() }
  }
  private var closePrepared = false
  private var preparingClose = false
  public override func close() {
    guard let window, windowShouldClose(window) else { return }
    super.close()
  }
  public func prepareToClose(operation: SlopTelemetryEvent.Failure = .close) async throws {
    telemetry.send(.breadcrumb(operation, .started))
    loadingTask?.cancel()
    openingProgress?.finish()
    if session.engine.rendererDead || !session.isReady { return }
    window?.makeFirstResponder(nil)
    do {
      try await session.engine.prepareClose()
    } catch {
      reportLifecycleFailure(operation, error: error)
      await cancelPreparedClose()
      throw error
    }
    // Nothing was editable during startup; closing an unfinished open does
    // not need to launch another WebView to refresh artwork.
    guard isContentReady else { return }
    // The render reads the saved document into memory after this window closes.
    SlopDocumentAssetRefreshQueue.schedule(presentedURL: packageURL, telemetry: telemetry)
  }
  public func cancelPreparedClose() async {
    await session.engine.cancelClose()
    if isLoading { startLoading() }
  }
  public static func finishAssetRefreshesForTermination() async {
    await SlopDocumentAssetRefreshQueue.finishForTermination()
  }
  public func windowShouldClose(_ sender: NSWindow) -> Bool {
    if closePrepared { return true }
    if let onCommand {
      onCommand(.close)
      return false
    }
    guard !preparingClose else { return false }
    preparingClose = true
    Task {
      defer { preparingClose = false }
      do {
        try await prepareToClose()
        try await finishClose()
      } catch {
        if isLoading { startLoading() }
        if Self.isCapacityFailure(error.localizedDescription) { offerDiscardAndClose(error) }
        else { present("Changes could not be saved", error) }
      }
    }
    return false
  }
  public func windowDidResize(_ notification: Notification) {
    if toolbar?.isVisible == true { showToolbar() }
  }
  public func windowWillMiniaturize(_ notification: Notification) {
    hideToolbar()
  }
  public func windowWillClose(_ notification: Notification) {
    SlopToolbarPointerSampler.shared.remove(self)
    isContentReady = false
    stopLoading()
    documentAttention?.close()
    documentAttention = nil
    hideToolbar()
    if let toolbar { window?.removeChildWindow(toolbar) }
    toolbar?.close()
    toolbar = nil
    onClose?()
  }
  /// Save failures for storage capacity; the runtime never drops unsaved edits silently.
  static func isCapacityFailure(_ message: String) -> Bool { message.contains("Document is full") }

  private func offerDiscardAndClose(_ error: Error) {
    let alert = NSAlert(error: error)
    alert.messageText = "This document is full"
    alert.informativeText = "Your unsaved changes don't fit. Discard them to close, or cancel to keep this document open."
    alert.addButton(withTitle: "Discard Unsaved Edits and Close")
    alert.addButton(withTitle: "Cancel")
    guard alert.runModal() == .alertFirstButtonReturn else { return }
    Task {
      do {
        try await session.engine.discardPending()
        window?.performClose(nil)
      } catch {
        present("Unsaved changes could not be discarded", error)
      }
    }
  }

  private func present(_ title: String, _ error: Error) {
    let alert = NSAlert(error: error)
    alert.messageText = title
    alert.runModal()
  }
}

private struct FailureOverlay: View {
  let message: String, retry: () -> Void
  var body: some View {
    VStack(spacing: 12) {
      Image(systemName: "exclamationmark.triangle").font(.title)
      Text("This slop stopped responding").font(.headline)
      Text(message).font(.caption).foregroundStyle(.secondary).multilineTextAlignment(.center)
      Button("Reopen saved document", action: retry).buttonStyle(.borderedProminent)
    }.padding(24).frame(maxWidth: .infinity, maxHeight: .infinity).background(.regularMaterial)
  }
}

/// Launch Services first, then these fallback paths, so non-/Applications installs still appear.
func slopOpenInCatalog() -> [(String, String, String)] {
  [
    ("Open in Cursor", "com.todesktop.230313mzl4w4u92", "/Applications/Cursor.app"),
    ("Open in Visual Studio Code", "com.microsoft.VSCode", "/Applications/Visual Studio Code.app"),
    (
      "Open in VS Code Insiders", "com.microsoft.VSCodeInsiders",
      "/Applications/Visual Studio Code - Insiders.app"
    ),
    ("Open in Terminal", "com.apple.Terminal", "/System/Applications/Utilities/Terminal.app"),
    ("Open in iTerm", "com.googlecode.iterm2", "/Applications/iTerm.app"),
    ("Open in Warp", "dev.warp.Warp-Stable", "/Applications/Warp.app"),
    ("Open in Wave", "dev.commandline.waveterm", "/Applications/Wave.app"),
    ("Open in Ghostty", "com.mitchellh.ghostty", "/Applications/Ghostty.app"),
  ]
}

extension UTType {
  public static let slop = UTType(exportedAs: "com.hitslop.slop", conformingTo: .package)
}
