import AppKit
import HitSlopCore
import HitSlopDocument
import SwiftUI
import WebKit

/// The hover toolbar above a document window: its panel, when it shows, and the controls
/// visibility it publishes to the page. One native hover decision drives both.
@MainActor final class SlopHoverToolbar {
  private weak var window: NSWindow?
  private let session: DocumentSession
  private let identity: SlopDocumentIdentity
  /// What the controls show now; `update()` reads it again.
  private let controls: () -> SlopToolbar.Controls
  /// The window is loading a page; the toolbar stays hidden meanwhile.
  private let isLoading: () -> Bool
  private let act: (SlopToolbar.Action) -> Void
  private(set) var panel: SlopToolbarPanel?
  private var host: NSHostingView<SlopToolbar>?
  private var menuTracking = false
  private var interacting = false
  private var visibility = SlopToolbarVisibility()
  private weak var controlsWebView: WKWebView?
  private var publishedControlsVisible: Bool?

  init(
    window: NSWindow?, session: DocumentSession, identity: SlopDocumentIdentity,
    controls: @escaping () -> SlopToolbar.Controls, isLoading: @escaping () -> Bool,
    act: @escaping (SlopToolbar.Action) -> Void
  ) {
    self.window = window
    self.session = session
    self.identity = identity
    self.controls = controls
    self.isLoading = isLoading
    self.act = act
    SlopToolbarPointerSampler.shared.add(self) { [weak self] point, front in
      self?.refresh(point: point, front: front)
      return self?.isVisible == true
    }
  }

  var isVisible: Bool { panel?.isVisible == true }

  /// The controls changed: shows them as they are now.
  func update() { host?.rootView = view() }

  func show() {
    guard let window, window.isVisible, !window.isMiniaturized else { return }
    let panel = panel ?? makePanel()
    // One level above the document, so no window at its level (another app's window
    // clicked beside a pinned slop) can cover the toolbar.
    panel.level = NSWindow.Level(rawValue: window.level.rawValue + 1)
    panel.setFrame(slopToolbarFrame(document: window.frame, visible: window.screen?.visibleFrame), display: true)
    panel.orderFrontRegardless()
    publishControls(true)
    SlopToolbarPointerSampler.shared.start()
  }

  func hide() {
    if isVisible { panel?.orderOut(nil) }
    publishControls(false)
  }

  /// The window moved, resized or changed level: a shown toolbar follows it.
  func relayout() { if isVisible { show() } }

  /// The page is new or recovered: tells it again whether its controls show.
  func republishControls() { publishControls(isVisible, force: true) }

  /// The window is closing: the toolbar goes with it.
  func close() {
    SlopToolbarPointerSampler.shared.remove(self)
    hide()
    panel?.close()
    panel = nil
    host = nil
  }

  /// Decides whether the toolbar shows for the pointer at `point`, `front` being the window
  /// the window server reports there and `below` the one under this document.
  func refresh(
    point: NSPoint = NSEvent.mouseLocation, front: Int? = nil, below: Int? = nil,
    now: TimeInterval = ProcessInfo.processInfo.systemUptime
  ) {
    guard let window else { return }
    guard window.isVisible, !window.isMiniaturized, window.isOnActiveSpace, !NSApp.isHidden, !isLoading() else {
      visibility = SlopToolbarVisibility()
      hide()
      return
    }
    let front = front ?? NSWindow.windowNumber(at: point, belowWindowWithWindowNumber: 0)
    let fullscreen = window.styleMask.contains(.fullScreen)
    let composition =
      (window.contentView as? ShapedView)
      ?? (window.contentView as? SlopFullscreenSurface)?.composition
    let overDocument: Bool
    // Clicks pass through transparent pixels, where the window server reports the window
    // behind; the shape decides unless another window covers this one at the point.
    if window.frame.contains(point), let shaped = composition,
      front == window.windowNumber
        || front == (below ?? NSWindow.windowNumber(at: point, belowWindowWithWindowNumber: window.windowNumber))
    {
      let local = shaped.convert(window.convertPoint(fromScreen: point), from: nil)
      // In fullscreen only the top edge wakes the toolbar; moving within the app
      // should not leave controls over an otherwise unattended display.
      overDocument =
        fullscreen
        ? point.y >= window.frame.maxY - 12
        : shaped.windowMask.contains(local, in: shaped.bounds)
    } else {
      overDocument = false
    }
    let toolbarFrame = panel?.frame ?? .zero
    let overToolbar = isVisible && front == panel?.windowNumber
    let gap = NSRect(
      x: max(window.frame.minX, toolbarFrame.minX), y: window.frame.maxY,
      width: max(0, min(window.frame.maxX, toolbarFrame.maxX) - max(window.frame.minX, toolbarFrame.minX)),
      height: max(0, toolbarFrame.minY - window.frame.maxY))
    let nearToolbar =
      isVisible && (toolbarFrame.insetBy(dx: -4, dy: -4).contains(point) || gap.contains(point))
      && (front == 0 || front == window.windowNumber || front == panel?.windowNumber)
    // A visible panel is not proof it is on top: a pointer returning during the grace
    // period orders it front again.
    let returning = visibility.outsideSince != nil
    let show = visibility.shouldShow(
      inside: overDocument || overToolbar || nearToolbar, interacting: menuTracking || interacting,
      visible: isVisible, now: now)
    if show {
      if !isVisible || (returning && visibility.outsideSince == nil) { self.show() }
    } else {
      hide()
    }
    publishControls(isVisible)
  }

  private func makePanel() -> SlopToolbarPanel {
    let frame = slopToolbarFrame(document: window?.frame ?? .zero, visible: window?.screen?.visibleFrame)
    let panel = SlopToolbarPanel(
      contentRect: frame, styleMask: [.borderless, .nonactivatingPanel], backing: .buffered, defer: false)
    panel.isOpaque = false
    panel.backgroundColor = .clear
    panel.hasShadow = true
    panel.collectionBehavior = [.fullScreenAuxiliary]
    panel.hidesOnDeactivate = false
    panel.isReleasedWhenClosed = false
    panel.isExcludedFromWindowsMenu = true
    let tracking = HoverView(frame: panel.contentView?.bounds ?? NSRect(origin: .zero, size: frame.size))
    panel.contentView = tracking
    let host = NSHostingView(rootView: view())
    host.frame = tracking.bounds
    host.autoresizingMask = [.width, .height]
    tracking.addSubview(host)
    tracking.changed = { [weak self] _ in self?.refresh() }
    panel.drag = { [weak self] in self?.drag(with: $0) }
    panel.interactionChanged = { [weak self] active in
      self?.interacting = active
      self?.refresh()
    }
    self.panel = panel
    self.host = host
    return panel
  }

  private func view() -> SlopToolbar {
    SlopToolbar(
      identity: identity, controls: controls(),
      menuTrackingChanged: { [weak self] tracking in
        guard let self, !tracking || isVisible else { return }
        menuTracking = tracking
        refresh()
      },
      drag: { [weak self] event in self?.drag(with: event) },
      act: { [weak self] action in
        guard let self else { return }
        if case .minimize = action {
          guard controls().desktopControls else { return }
          hide()
          window?.miniaturize(nil)
        } else {
          act(action)
        }
      },
      editors: SlopEditors.installed)
  }

  private func drag(with event: NSEvent) {
    guard controls().desktopControls else { return }
    window?.performDrag(with: event)
    show()
  }

  /// Publishes transitions only; the pointer sampler must not send JavaScript every tick.
  private func publishControls(_ visible: Bool, force: Bool = false) {
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
        self.publishedControlsVisible == visible
      {
        self.publishedControlsVisible = nil
      }
    }
  }
}

extension SlopDocumentWindowController {
  /// What the toolbar's controls show now.
  var toolbarControls: SlopToolbar.Controls {
    SlopToolbar.Controls(
      pinned: isPinned, canPin: canPin,
      fullscreenable: session.file.isFullscreenable, fullscreen: isFullscreen,
      canFullscreen: canToggleFullscreen, desktopControls: fullscreenRestore == nil && !fullscreenTransition,
      themeShown: isThemeShown, canToggleTheme: canToggleTheme,
      commandsEnabled: isAvailable(.duplicate))
  }

  func toolbarAction(_ action: SlopToolbar.Action) {
    switch action {
    case .document(let command): request(command)
    case .close: request(.close)
    case .minimize: break  // The toolbar minimizes its window itself.
    case .toggleFullscreen: toggleFullscreen()
    case .togglePin: togglePin()
    case .toggleTheme: toggleTheme()
    case .reveal: reveal()
    case .copyPath: copyPath()
    case .openEditor(let app): openInEditor(app)
    }
  }
}
