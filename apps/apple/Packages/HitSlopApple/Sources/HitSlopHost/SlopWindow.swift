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

/// How a document window reaches the app that coordinates it. Every document operation,
/// including close and the save-failure sheet's choices, goes to `command`, so the app runs
/// them one at a time; a window never runs one itself.
public struct SlopDocumentRouting {
  public var command: @MainActor (SlopDocumentCommand) -> Void
  /// The page is ready for the first time, or again after a recovery.
  public var pageReady: @MainActor () -> Void
  public init(
    command: @escaping @MainActor (SlopDocumentCommand) -> Void, pageReady: @escaping @MainActor () -> Void = {}
  ) {
    self.command = command
    self.pageReady = pageReady
  }
}

@MainActor
public final class SlopDocumentWindowController: NSWindowController, NSWindowDelegate, DocumentSessionDelegate {
  public let url: URL
  public let session: DocumentSession
  let routing: SlopDocumentRouting
  public let telemetry: SlopTelemetry
  var reportedSaveFailure = false
  var reportedRendererFailure = false
  /// Issue kinds already reported (operations, authored); each is reported once.
  var reportedIssueKinds = Set<Bool>()
  /// The hover toolbar above the window.
  private(set) lazy var toolbar = SlopHoverToolbar(
    window: window, session: session, identity: SlopDocumentIdentity(url: url),
    controls: { [weak self] in self?.toolbarControls ?? .init() },
    isLoading: { [weak self] in self?.isLoading ?? false },
    act: { [weak self] action in self?.toolbarAction(action) })
  var failedOverlay: NSHostingView<FailureOverlay>?
  var presentedPageError: String?
  var documentAttention: NSPanel?
  var attentionFailure: SaveFailure?
  var guestIssue: SlopPageIssue?
  /// The red dot shown while `guestIssue` is set.
  var issueBadge: NSPanel?
  /// The theme panel beside the window, while shown.
  var themePanel: NSPanel?
  var themeEditor: SlopThemeEditorModel?
  var commandsEnabled = true
  var openingProgress: SlopOpeningProgress?
  /// Undo for this window's document; see `DocumentUndoManager`.
  lazy var documentUndo = DocumentUndoManager(session: session)
  var isLoading = false
  var presentationRequested = false
  public internal(set) var isContentReady = false
  var loadingTask: Task<Void, Never>?
  weak var loadingWebView: NSView?
  let startupStarted: ContinuousClock.Instant

  /// Opens the document at `url`. With `progress`, the window is shown once ready, and the
  /// progress panel (its caller's, shown if opening is slow) can cancel the open.
  public static func open(
    url: URL, routing: SlopDocumentRouting, progress: SlopOpeningProgress? = nil, telemetry: SlopTelemetry = .disabled
  ) async throws -> SlopDocumentWindowController {
    let started = ContinuousClock.now
    let preparation = Task { @MainActor in
      let session = try await DocumentSession.open(url: url)
      do {
        try Task.checkCancellation()
        return SlopDocumentWindowController(
          url: url, session: session, routing: routing, started: started, telemetry: telemetry)
      } catch {
        try await session.close()
        throw error
      }
    }
    progress?.onCancel = { preparation.cancel() }
    do {
      let controller = try await withTaskCancellationHandler {
        try await preparation.value
      } onCancel: {
        preparation.cancel()
      }
      if Task.isCancelled || preparation.isCancelled {
        try await controller.finishClose()
        throw CancellationError()
      }
      if let progress {
        controller.openingProgress = progress
        controller.showWindow(nil)
      }
      return controller
    } catch {
      progress?.finish()
      throw error
    }
  }

  private init(
    url: URL, session: DocumentSession, routing: SlopDocumentRouting, started: ContinuousClock.Instant,
    telemetry: SlopTelemetry = .disabled
  ) {
    self.routing = routing
    self.telemetry = telemetry
    startupStarted = started
    self.url = url.standardizedFileURL
    self.session = session
    // Start WebKit before building native chrome; bridge messages arrive only
    // after this initializer returns to the run loop.
    session.load()
    let windowMask = SlopWindowMask(file: session.file)
    let spec = session.file.manifest.presentation
    let size = NSSize(width: spec.width, height: spec.height)
    let window = FramelessDocumentWindow(
      contentRect: NSRect(origin: .zero, size: size),
      styleMask: slopDocumentWindowStyleMask(resizable: session.file.isResizable),
      backing: .buffered, defer: false)
    window.title = SlopDocumentIdentity(url: self.url).filename
    window.minSize = NSSize(width: WindowBounds.minWidth, height: WindowBounds.minHeight)
    window.isOpaque = false
    window.backgroundColor = .clear
    window.hasShadow = !session.file.usesTransparentBackground || session.file.isSkinned
    window.isReleasedWhenClosed = false
    window.tabbingMode = .disallowed
    window.representedURL = self.url
    window.miniwindowTitle = window.title
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
    // `slop export` of this open document exports its live view, as the window does.
    session.onExport = { [weak self] format, output, deadline in
      guard let self else { throw SlopFailure("Document closed") }
      try await self.exportDocument(format: format, to: output, deadline: deadline)
    }
    container.changed = { [weak self] _ in self?.toolbar.refresh() }
    _ = toolbar  // It follows the pointer from now on.
    // Editor discovery queries Launch Services; warm it before the first hover.
    Task.detached(priority: .utility) { _ = SlopEditors.installed }
    startLoading()
    recordStartup("native-prepared")
    // The minimized window shows the document's icon, read by its owner rather than by a
    // new open on the main thread.
    Task { [weak self, session] in
      guard let icon = await session.artwork(.icon) else { return }
      self?.window?.miniwindowImage = NSImage(data: icon)
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
    } else {
      showOpeningProgress()
    }
  }

  public func pageSession(_ session: DocumentSession, resizeContentTo requested: CGSize)
    throws -> CGSize
  {
    guard let window else { throw SlopFailure("document window is unavailable") }
    var requested = requested
    let spec = session.file.manifest.presentation
    if spec.lockAspect == true {
      let ratio = CGFloat(spec.width) / CGFloat(spec.height)
      requested.width = max(CGFloat(WindowBounds.minWidth), CGFloat(WindowBounds.minHeight) * ratio, requested.width)
      requested.height = requested.width / ratio
      let visible = window.screen?.visibleFrame ?? NSScreen.main?.visibleFrame
      let factor = min(
        1, (visible?.width ?? requested.width) / requested.width,
        (visible?.height ?? requested.height) / requested.height)
      requested.width *= factor
      requested.height *= factor
    }
    let frame = dynamicSlopWindowFrame(
      current: window.frame, requested: requested,
      visible: window.screen?.visibleFrame ?? NSScreen.main?.visibleFrame)
    window.setFrame(frame, display: true, animate: true)
    toolbar.relayout()
    layoutThemePanel()
    return frame.size
  }
  public var isPinned: Bool { window?.level == .floating }
  /// The one pin path: a visible toolbar moves to the document's new level at once.
  func setPinned(_ pinned: Bool) {
    window?.level = pinned ? .floating : .normal
    toolbar.update()
    toolbar.relayout()
    layoutThemePanel()
  }
  public var documentTitle: String { window?.title ?? SlopDocumentIdentity(url: url).filename }
  /// The window's icon, read once at open, at menu size.
  public var dockMenuImage: NSImage {
    let source = window?.miniwindowImage ?? NSWorkspace.shared.icon(forFile: url.path)
    let image = (source.copy() as? NSImage) ?? source
    image.size = NSSize(width: 16, height: 16)
    return image
  }
  public func owns(_ candidate: NSWindow?) -> Bool {
    guard let candidate else { return false }
    return candidate === window || candidate === toolbar.panel || candidate === themePanel
      || candidate === openingProgress?.panel
  }
  /// Whether the coordinator accepts commands now; the toolbar follows it.
  public func setCommandsEnabled(_ enabled: Bool) {
    guard commandsEnabled != enabled else { return }
    commandsEnabled = enabled
    toolbar.update()
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

  /// Saves and releases the document, writing artwork rendered from its saved state first, so
  /// Finder, Quick Look and the catalog show it as it closed. The window leaves the screen
  /// at once; a failed close shows it again, open and editable.
  public func finishClose(operation: SlopTelemetryEvent.Failure = .close) async throws {
    guard !closePrepared else { return }
    let shown = window?.isVisible == true
    toolbar.hide()
    window?.orderOut(nil)
    let artwork = await closingArtwork()
    do {
      try await session.close(artwork: artwork)
      if artwork != nil { SlopPreviewWriter.announce(url) }
      closePrepared = true
      window?.close()
      telemetry.send(.breadcrumb(operation, .completed))
    } catch {
      if shown { window?.orderFront(nil) }
      reportLifecycleFailure(operation, error: error)
      throw error
    }
  }
  /// The page's preview and icon, when this session changed the document or it has no
  /// preview yet. Rendering uses an independent snapshot, never the window.
  private func closingArtwork() async -> SlopRenderedArtwork? {
    guard isContentReady, session.isReady, !session.rendererDead else { return nil }
    let edited = session.edited
    let preview = edited ? nil : await session.artwork(.preview)
    guard edited || preview == nil else { return nil }
    return await SlopRenderer.artwork(session: session, telemetry: telemetry)
  }

  public func windowDidMove(_ notification: Notification) { toolbar.relayout() }
  private var closePrepared = false
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
  }
  public func cancelPreparedClose() async {
    await session.cancelClose()
    if isLoading { startLoading() }
  }
  public func windowWillReturnUndoManager(_ window: NSWindow) -> UndoManager? { documentUndo }
  /// Closing is a command: the coordinator runs it after any command in progress, and
  /// the window closes once the document has saved and released.
  public func windowShouldClose(_ sender: NSWindow) -> Bool {
    if closePrepared { return true }
    routing.command(.close)
    return false
  }
  public func windowDidResize(_ notification: Notification) {
    toolbar.relayout()
    refreshIssueBadge()
    layoutThemePanel()
  }
  public func windowDidChangeScreen(_ notification: Notification) {
    layoutThemePanel()
  }
  public func windowWillMiniaturize(_ notification: Notification) { toolbar.hide() }
  public func windowWillClose(_ notification: Notification) {
    isContentReady = false
    stopLoading()
    documentAttention?.close()
    documentAttention = nil
    guestIssue = nil
    refreshIssueBadge()
    closeThemePanel()
    toolbar.close()
  }
}

extension UTType {
  /// A `.slop`: one file (the app's Info.plist declares it as `public.data`).
  public static let slop = UTType(exportedAs: "com.hitslop.slop", conformingTo: .data)
}
