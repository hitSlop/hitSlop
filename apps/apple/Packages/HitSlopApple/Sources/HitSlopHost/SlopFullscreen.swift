import AppKit
import HitSlopCore
import WebKit

/// A fullscreen surface owns the backdrop, while the existing composition retains its
/// authored page coordinates, mask, WebView and input handling. WebKit zoom paints
/// text at the display's resolution instead of stretching a small backing image.
final class SlopFullscreenSurface: NSView {
  let composition: ShapedView
  let authoredSize: NSSize
  let fit: Bool

  init(composition: ShapedView, authoredSize: NSSize, fit: Bool) {
    self.composition = composition
    self.authoredSize = authoredSize
    self.fit = fit
    super.init(frame: composition.frame)
    wantsLayer = true
    layer?.backgroundColor = NSColor.black.cgColor
    composition.autoresizingMask = []
    addSubview(composition)
  }

  required init?(coder: NSCoder) { nil }

  override func layout() {
    super.layout()
    composition.autoresizesSubviews = false
    let scale = fit ? min(bounds.width / authoredSize.width, bounds.height / authoredSize.height) : 1
    guard scale > 0 else {
      composition.autoresizesSubviews = true
      return
    }
    if fit {
      let size = NSSize(width: authoredSize.width * scale, height: authoredSize.height * scale)
      composition.frame = NSRect(
        x: bounds.midX - size.width / 2, y: bounds.midY - size.height / 2,
        width: size.width, height: size.height)
    } else {
      composition.frame = bounds
    }
    composition.bounds = NSRect(origin: .zero, size: composition.frame.size)
    composition.compositionScale = scale
    for child in composition.subviews {
      child.frame = composition.bounds
      if let page = child as? WKWebView, page.pageZoom != scale { page.pageZoom = scale }
    }
    composition.autoresizesSubviews = true
  }
}

struct SlopFullscreenRestore {
  let frame: NSRect
  let style: NSWindow.StyleMask
  let level: NSWindow.Level
  let aspect: NSSize
  let shadow: Bool
}

extension SlopDocumentWindowController {
  public var isFullscreen: Bool { window?.styleMask.contains(.fullScreen) == true }
  public var canToggleFullscreen: Bool {
    session.file.isFullscreenable && (isFullscreen || isContentReady) && !isHiddenForClose && !fullscreenTransition
  }
  var documentComposition: ShapedView? {
    (window?.contentView as? ShapedView) ?? (window?.contentView as? SlopFullscreenSurface)?.composition
  }

  public func toggleFullscreen() {
    guard canToggleFullscreen, let window else { return }
    if !isFullscreen { prepareFullscreen() }
    fullscreenTransition = true
    toolbar.update()
    window.toggleFullScreen(nil)
  }

  private func prepareFullscreen() {
    guard fullscreenRestore == nil, let window, let composition = documentComposition else { return }
    fullscreenRestore = SlopFullscreenRestore(
      frame: window.frame, style: window.styleMask, level: window.level,
      aspect: window.contentAspectRatio, shadow: window.hasShadow)
    closeThemePanel()
    toolbar.hide()
    window.level = .normal
    window.contentResizeIncrements = NSSize(width: 1, height: 1)
    // Detach the old content before reparenting it. Install the opaque surface
    // before AppKit constructs a titlebar, so that titlebar never inherits the
    // shaped document's backing (which otherwise bleeds above the page).
    window.contentView = nil
    let surface = SlopFullscreenSurface(
      composition: composition,
      authoredSize: NSSize(width: session.file.width, height: session.file.height),
      fit: session.file.fitsFullscreen)
    window.contentView = surface
    window.isOpaque = true
    window.backgroundColor = .black
    // AppKit's native transition needs a titled, resizable window. The titlebar is
    // hidden; desktop style and constraints are restored after leaving the Space.
    window.styleMask.insert([.titled, .resizable, .fullSizeContentView])
    window.titleVisibility = .hidden
    window.titlebarAppearsTransparent = false
    window.titlebarSeparatorStyle = .none
    for button in [NSWindow.ButtonType.closeButton, .miniaturizeButton, .zoomButton] {
      window.standardWindowButton(button)?.isHidden = true
    }
    window.hasShadow = false
    surface.needsLayout = true
  }

  private func restoreDesktop() {
    guard let window, let restore = fullscreenRestore, let composition = documentComposition else { return }
    toolbar.hide()
    composition.removeFromSuperview()
    composition.compositionScale = 1
    session.webView.pageZoom = 1
    composition.bounds = NSRect(origin: .zero, size: restore.frame.size)
    window.contentView = composition
    window.styleMask = restore.style
    if restore.aspect.width > 0 && restore.aspect.height > 0 {
      window.contentAspectRatio = restore.aspect
    } else {
      window.contentResizeIncrements = NSSize(width: 1, height: 1)
    }
    window.level = restore.level
    window.hasShadow = restore.shadow
    window.isOpaque = false
    window.backgroundColor = .clear
    window.setFrame(restore.frame, display: true)
    composition.frame = NSRect(origin: .zero, size: window.contentLayoutRect.size)
    composition.bounds = NSRect(origin: .zero, size: composition.frame.size)
    for child in composition.subviews { child.frame = composition.bounds }
    fullscreenRestore = nil
    fullscreenTransition = false
    toolbar.update()
    toolbar.refresh()
  }

  public func windowWillEnterFullScreen(_ notification: Notification) {
    prepareFullscreen()
    fullscreenTransition = true
    toolbar.hide()
    toolbar.update()
  }
  public func windowDidEnterFullScreen(_ notification: Notification) {
    fullscreenTransition = false
    window?.contentView?.needsLayout = true
    toolbar.update()
    toolbar.refresh()
  }
  public func windowWillExitFullScreen(_ notification: Notification) {
    fullscreenTransition = true
    toolbar.hide()
    toolbar.update()
  }
  public func windowDidExitFullScreen(_ notification: Notification) {
    // AppKit finishes its style/frame bookkeeping after delivering this notification.
    // Restore our borderless style on the next main-loop turn.
    DispatchQueue.main.async { [weak self] in self?.restoreDesktop() }
  }
  public func windowDidFailToEnterFullScreen(_ window: NSWindow) {
    DispatchQueue.main.async { [weak self] in self?.restoreDesktop() }
  }
  public func windowDidFailToExitFullScreen(_ window: NSWindow) {
    fullscreenTransition = false
    toolbar.update()
    toolbar.refresh()
  }
}
