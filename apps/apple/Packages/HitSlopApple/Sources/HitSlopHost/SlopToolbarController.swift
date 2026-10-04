import AppKit
import HitSlopCore
import HitSlopDocument
import SwiftUI
import WebKit

/// The hover toolbar: its panel, when it shows, and the controls it publishes to the page.
extension SlopDocumentWindowController {
  func setupToolbar() {
    let frame = slopToolbarFrame(document: window?.frame ?? .zero, visible: window?.screen?.visibleFrame)
    let panel = SlopToolbarPanel(
      contentRect: frame,
      styleMask: [.borderless, .nonactivatingPanel], backing: .buffered, defer: false)
    panel.isOpaque = false
    panel.backgroundColor = .clear
    panel.hasShadow = true
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
    toolbar = panel
    toolbarHost = host
    panel.drag = { [weak self] in self?.dragWindow(with: $0) }
    panel.interactionChanged = { [weak self] active in
      self?.toolbarInteracting = active
      self?.refreshToolbarHover()
    }
  }
  func toolbarView() -> SlopToolbar {
    SlopToolbar(
      identity: SlopDocumentIdentity(url: url),
      menuTrackingChanged: { [weak self] tracking in
        guard let self else { return }
        guard !tracking || toolbar?.isVisible == true else { return }
        toolbarMenuTracking = tracking
        refreshToolbarHover()
      },
      drag: { [weak self] event in self?.dragWindow(with: event) }, pinned: isPinned,
      commandsEnabled: commandsEnabled && isContentReady,
      themeShown: isThemeShown, themeEnabled: commandsEnabled && isContentReady && session.canEditTheme,
      minimize: { [weak self] in self?.miniaturizeFromToolbar() },
      send: { [weak self] command in self?.request(command) }, editors: SlopEditors.installed)
  }
  func dragWindow(with event: NSEvent) {
    window?.performDrag(with: event)
    showToolbar()
  }
  func miniaturizeFromToolbar() {
    hideToolbar()
    window?.miniaturize(nil)
  }
  func showToolbar() {
    guard let window, window.isVisible, !window.isMiniaturized else { return }
    if toolbar == nil { setupToolbar() }
    guard let panel = toolbar else { return }
    // One level above the document, so no window at its level (another app's window
    // clicked beside a pinned slop) can cover the toolbar.
    panel.level = NSWindow.Level(rawValue: window.level.rawValue + 1)
    panel.setFrame(slopToolbarFrame(document: window.frame, visible: window.screen?.visibleFrame), display: true)
    panel.orderFrontRegardless()
    publishControlsVisibility(true)
    SlopToolbarPointerSampler.shared.start()
  }

  func hideToolbar() {
    if toolbar?.isVisible == true { toolbar?.orderOut(nil) }
    publishControlsVisibility(false)
  }

  /// One native hover decision drives both the toolbar and opt-in authored controls.
  /// Publish transitions only; the pointer sampler must not send JavaScript every tick.
  func publishControlsVisibility(_ visible: Bool, force: Bool = false) {
    guard session.isReady, !session.rendererDead else {
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

  func refreshToolbarHover(point: NSPoint = NSEvent.mouseLocation, front: Int? = nil, below: Int? = nil,
                           now: TimeInterval = ProcessInfo.processInfo.systemUptime) {
    guard let window else { return }
    guard window.isVisible, !window.isMiniaturized, window.isOnActiveSpace, !NSApp.isHidden, !isLoading else {
      toolbarVisibility = SlopToolbarVisibility()
      hideToolbar()
      return
    }
    let front = front ?? NSWindow.windowNumber(at: point, belowWindowWithWindowNumber: 0)
    let overDocument: Bool
    // Clicks pass through transparent pixels, where the window server reports the window
    // behind; the shape decides unless another window covers this one at the point.
    if window.frame.contains(point), let shaped = window.contentView as? ShapedView,
       front == window.windowNumber
        || front == (below ?? NSWindow.windowNumber(at: point, belowWindowWithWindowNumber: window.windowNumber)) {
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
    // A visible panel is not proof it is on top: a pointer returning during the grace
    // period orders it front again.
    let returning = toolbarVisibility.outsideSince != nil
    let show = toolbarVisibility.shouldShow(
      inside: overDocument || overToolbar || nearToolbar,
      interacting: toolbarMenuTracking || toolbarInteracting,
      visible: (toolbar?.isVisible == true), now: now)
    if show {
      if !(toolbar?.isVisible == true) || (returning && toolbarVisibility.outsideSince == nil) { showToolbar() }
    } else { hideToolbar() }
    publishControlsVisibility(toolbar?.isVisible == true)
  }
}
