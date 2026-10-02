import AppKit
import HitSlopCore
import HitSlopDocument
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

class HoverView: NSView {
  var changed: ((Bool) -> Void)?
  private var area: NSTrackingArea?
  override func updateTrackingAreas() {
    if let area { removeTrackingArea(area) }
    // Moves re-check too: an entry outside the window's shape, or before the window
    // server reports this window under the pointer, must not leave the toolbar hidden.
    let next = NSTrackingArea(
      rect: bounds, options: [.mouseEnteredAndExited, .mouseMoved, .activeAlways, .inVisibleRect], owner: self)
    addTrackingArea(next)
    area = next
    super.updateTrackingAreas()
  }
  override func mouseEntered(with event: NSEvent) { changed?(true) }
  override func mouseMoved(with event: NSEvent) { changed?(true) }
  override func mouseExited(with event: NSEvent) { changed?(false) }
}

final class ShapedView: HoverView {
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

@MainActor
public final class SlopDocumentWindowController: NSWindowController, NSWindowDelegate, DocumentSessionDelegate
{
  public let packageURL: URL
  public let session: DocumentSession
  public var onClose: (() -> Void)?
  public var onCommand: ((SlopDocumentCommand) -> Void)?
  public var onPageReady: (() -> Void)?
  public var telemetry: SlopTelemetry = .disabled
  var reportedSaveFailure = false
  var reportedRendererFailure = false
  /// Issue kinds already reported (operations, authored); each is reported once.
  var reportedIssueKinds = Set<Bool>()
  public var onPageFailure: ((String) -> Void)?
  var toolbar: NSPanel?, toolbarHost: NSHostingView<SlopToolbar>?
  var toolbarMenuTracking = false
  var toolbarInteracting = false
  var toolbarVisibility = SlopToolbarVisibility()
  weak var controlsWebView: WKWebView?
  var publishedControlsVisible: Bool?
  var failedOverlay: NSHostingView<FailureOverlay>?
  var presentedPageError: String?
  var documentAttention: NSPanel?
  var attentionMessage: String?
  var attentionFailure: SaveFailure?
  var guestIssue: SlopPageIssue?
  /// The red dot shown while `guestIssue` is set.
  var issueBadge: NSPanel?
  var commandsEnabled = true
  var openingProgress: SlopOpeningProgress?
  var isLoading = false
  var presentationRequested = false
  public internal(set) var isContentReady = false
  var loadingTask: Task<Void, Never>?
  weak var loadingWebView: NSView?
  let startupStarted: ContinuousClock.Instant

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
      let session = try await DocumentSession.open(packageURL: packageURL)
      do {
        try Task.checkCancellation()
        return try SlopDocumentWindowController(packageURL: packageURL, session: session, started: started, telemetry: telemetry)
      } catch {
        try await session.close()
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

  private init(packageURL: URL, session: DocumentSession, started: ContinuousClock.Instant, telemetry: SlopTelemetry = .disabled) throws {
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
    window.minSize = NSSize(width: WindowBounds.minWidth, height: WindowBounds.minHeight)
    window.isOpaque = false
    window.backgroundColor = .clear
    window.hasShadow = !session.package.usesTransparentBackground || session.package.isSkinned
    window.isReleasedWhenClosed = false
    window.tabbingMode = .disallowed
    window.representedURL = self.packageURL
    window.miniwindowTitle = window.title
    window.miniwindowImage = NSImage(contentsOf: session.package.iconURL)
    if spec.lockAspect == true {
      window.contentAspectRatio = size
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
    SlopToolbarPointerSampler.shared.add(self) { [weak self] point, front in
      self?.refreshToolbarHover(point: point, front: front)
      return self?.toolbar?.isVisible == true
    }
    // Editor discovery queries Launch Services; warm it before the first hover.
    Task.detached(priority: .utility) { _ = SlopEditors.installed }
    SlopDocumentAssetRefreshQueue.invalidate(self.packageURL)
    startLoading()
    recordStartup("native-prepared")
    // Finder icon metadata is cosmetic; keep its disk writes off the opening path.
    Task { @MainActor [weak self, package = session.package] in
      await self?.waitForPresentation()
      guard self?.isContentReady == true else { return }
      SlopPreviewWriter.installAuthoredIcon(for: package, telemetry: self?.telemetry ?? .disabled)
    }
  }
  /// A discard or recovery replaced the page: show the new one and wait for it, clearing
  /// any failure overlay left by the renderer it replaced.
  public func pageSession(_ session: DocumentSession, didReplace view: WKWebView) {
    guard let content = window?.contentView else { return }
    view.frame = content.bounds
    view.autoresizingMask = [.width, .height]
    content.addSubview(view, positioned: .below, relativeTo: failedOverlay)
    updatePageFailure(nil)
    startLoading()
  }
  required init?(coder: NSCoder) { nil }

  deinit {
    loadingTask?.cancel()
    Task { @MainActor [progress = openingProgress] in progress?.finish() }
  }

  public override func showWindow(_ sender: Any?) {
    presentationRequested = true
    if isContentReady || presentedPageError != nil {
      openingProgress?.finish()
      openingProgress = nil
      revealReadyWindow()
    } else { showOpeningProgress() }
  }

  public func pageSession(_ session: DocumentSession, resizeContentTo requested: CGSize)
    throws -> CGSize
  {
    guard let window else { throw SlopPackageError.invalid("document window is unavailable") }
    var requested = requested
    let spec = session.package.manifest.presentation
    if spec.lockAspect == true {
      let ratio = CGFloat(spec.width) / CGFloat(spec.height)
      requested.width = max(CGFloat(WindowBounds.minWidth), CGFloat(WindowBounds.minHeight) * ratio, requested.width)
      requested.height = requested.width / ratio
      let visible = window.screen?.visibleFrame ?? NSScreen.main?.visibleFrame
      let factor = min(1, (visible?.width ?? requested.width) / requested.width, (visible?.height ?? requested.height) / requested.height)
      requested.width *= factor; requested.height *= factor
    }
    let frame = dynamicSlopWindowFrame(
      current: window.frame, requested: requested,
      visible: window.screen?.visibleFrame ?? NSScreen.main?.visibleFrame)
    window.setFrame(frame, display: true, animate: true)
    if toolbar?.isVisible == true { showToolbar() }
    return frame.size
  }
  public var isPinned: Bool { window?.level == .floating }
  /// The one pin path: a visible toolbar moves to the document's new level at once.
  func setPinned(_ pinned: Bool) {
    window?.level = pinned ? .floating : .normal
    toolbarHost?.rootView = toolbarView()
    if toolbar?.isVisible == true { showToolbar() }
  }
  public var documentTitle: String { window?.title ?? SlopDocumentIdentity(url: packageURL).filename }
  /// The window's icon, read once at open, at menu size.
  public var dockMenuImage: NSImage {
    let source = window?.miniwindowImage ?? NSWorkspace.shared.icon(forFile: packageURL.path)
    let image = (source.copy() as? NSImage) ?? source
    image.size = NSSize(width: 16, height: 16)
    return image
  }
  public func owns(_ candidate: NSWindow?) -> Bool {
    guard let candidate else { return false }
    return candidate === window || candidate === toolbar || candidate === openingProgress?.panel
  }
  public func updatePresentation(pinned: Bool, commandsEnabled: Bool, pageError: String?) {
    updatePageFailure(pageError)
    guard isPinned != pinned || self.commandsEnabled != commandsEnabled else { return }
    self.commandsEnabled = commandsEnabled
    setPinned(pinned)
  }
  public func revealFromDock() { showWindow(nil) }

  /// The one close sequence, for the window's close button and the app's Close command:
  /// the barrier, then the save, release and window close. A failure leaves the document
  /// open and editable; the caller presents it.
  func closeDocument() async throws {
    try await prepareToClose()
    do { try await finishClose() } catch {
      if isLoading { startLoading() }
      throw error
    }
  }

  public func finishClose(operation: SlopTelemetryEvent.Failure = .close) async throws {
    guard !closePrepared else { return }
    do {
      try await session.close()
      closePrepared = true
      window?.close()
      telemetry.send(.breadcrumb(operation, .completed))
    } catch { reportLifecycleFailure(operation, error: error); throw error }
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
    if session.rendererDead || !session.isReady { return }
    window?.makeFirstResponder(nil)
    do {
      try await session.prepareClose()
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
    await session.cancelClose()
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
        try await closeDocument()
      } catch {
        // A full document never drops unsaved edits silently.
        if error as? SaveFailure == .full { offerDiscardAndClose(error) }
        else { present("Changes could not be saved", error) }
      }
    }
    return false
  }
  public func windowDidResize(_ notification: Notification) {
    if toolbar?.isVisible == true { showToolbar() }
    refreshIssueBadge()
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
    guestIssue = nil
    refreshIssueBadge()
    hideToolbar()
    toolbar?.close()
    toolbar = nil
    onClose?()
  }
}

extension UTType {
  public static let slop = UTType(exportedAs: "com.hitslop.slop", conformingTo: .package)
}
