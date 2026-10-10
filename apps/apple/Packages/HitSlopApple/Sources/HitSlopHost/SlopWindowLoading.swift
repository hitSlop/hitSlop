import AppKit
import HitSlopCore
import HitSlopDocument
import SwiftUI
import WebKit

/// Opening a document window: waiting for the page, progress, and the first reveal.
extension SlopDocumentWindowController {
  public func pageSessionDidBecomeReady(_ session: DocumentSession) {
    toolbar.republishControls()
    failures.rendererRecovered(telemetry: telemetry)
    recordStartup("page-ready")
    #if DEBUG
      if ProcessInfo.processInfo.environment["HITSLOP_STARTUP_TIMINGS"] == "1" {
        // Page-relative milliseconds for the hitslop:* marks recorded by the runtime's boot.js.
        session.webView.evaluateJavaScript(
          "JSON.stringify(performance.getEntriesByType('mark').map(e => [e.name, Math.round(e.startTime)]))"
        ) { result, _ in print("[hitSlop startup] page \(result ?? "")") }
      }
    #endif
    // The bridge is ready before WebKit has necessarily painted. The loading
    // task owns the visual handoff and the coordinator's ready notification.
  }

  func recordStartup(_ stage: String) {
    #if DEBUG
      if ProcessInfo.processInfo.environment["HITSLOP_STARTUP_TIMINGS"] == "1" {
        print("[hitSlop startup] \(stage) \(presentation.started.duration(to: .now))")
      }
    #endif
  }

  func startLoading() {
    stopLoading()
    presentation.ready = false
    presentation.loading = true
    // A replaced page starts without the panel; it reopens once the new page is ready.
    closeThemePanel()
    window?.orderOut(nil)
    toolbar.hide()
    presentation.webView = session.webView
    session.webView.setAccessibilityHidden(true)
    toolbar.update()
    if presentation.requested { showOpeningProgress() }
    presentation.task = Task { @MainActor [weak self, session, weak view = session.webView] in
      do {
        // The page reports ready once it has mounted and its fonts have settled. Let the
        // visible window paint normally instead of waiting for animation frames in an
        // ordered-out WebView.
        try await session.waitUntilReady()
        try Task.checkCancellation()
        guard view != nil, session.isReady else { return }
        guard let self, self.presentation.loading else { return }
        self.presentation.ready = true
        self.finishLoading()
        if self.presentation.requested { self.revealReadyWindow() }
        self.updatePageFailure(nil)
        self.routing.pageReady()
      } catch is CancellationError {
      } catch {
        guard !Task.isCancelled else { return }
        self?.pageSession(session, didFail: error)
      }
    }
  }

  func finishLoading() {
    presentation.loading = false
    presentation.progress?.finish()
    presentation.progress = nil
    if let view = presentation.webView { view.setAccessibilityHidden(false) }
    presentation.webView = nil
    toolbar.update()
  }

  func stopLoading() {
    presentation.task?.cancel()
    presentation.task = nil
    finishLoading()
  }

  func showOpeningProgress() {
    if presentation.progress == nil { presentation.progress = SlopOpeningProgress(started: presentation.started) }
    presentation.progress?.onCancel = { [weak self] in
      self?.presentation.task?.cancel()
      self?.request(.close)
    }
    presentation.progress?.focus()
  }

  func revealReadyWindow() {
    guard !isHiddenForClose else { return }
    super.showWindow(nil)
    if window?.isMiniaturized == true { window?.deminiaturize(nil) }
    window?.makeKeyAndOrderFront(nil)
    refreshIssueBadge()
    if isContentReady {
      window?.makeFirstResponder(session.webView)
      recordStartup("content-visible")
    }
  }

  func waitForPresentation() async { await presentation.task?.value }
}
