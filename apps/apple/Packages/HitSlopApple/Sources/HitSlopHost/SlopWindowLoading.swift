import AppKit
import HitSlopCore
import HitSlopDocument
import SwiftUI
import WebKit

/// Opening a document window: waiting for the page, progress, and the first reveal.
extension SlopDocumentWindowController {
  public func pageSessionDidBecomeReady(_ session: DocumentSession) {
    publishControlsVisibility(toolbar?.isVisible == true, force: true)
    if reportedRendererFailure { telemetry.send(.breadcrumb(.renderer, .recovered)) }
    reportedRendererFailure = false
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
      print("[hitSlop startup] \(stage) \(startupStarted.duration(to: .now))")
    }
    #endif
  }

  func startLoading() {
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
        // The page reports ready once it has mounted and its fonts have settled. Let the
        // visible window paint normally instead of waiting for animation frames in an
        // ordered-out WebView.
        try await session.waitUntilReady()
        try Task.checkCancellation()
        guard view != nil, session.isReady else { return }
        guard let self, self.isLoading else { return }
        self.isContentReady = true
        self.finishLoading()
        if self.presentationRequested { self.revealReadyWindow() }
        if let onPageReady = self.onPageReady { onPageReady() }
        else { self.updatePageFailure(nil) }
      } catch is CancellationError {
      } catch {
        guard !Task.isCancelled else { return }
        self?.pageSession(session, didFail: error)
      }
    }
  }

  func finishLoading() {
    isLoading = false
    openingProgress?.finish()
    openingProgress = nil
    if let view = loadingWebView { view.setAccessibilityHidden(false) }
    loadingWebView = nil
    toolbarHost?.rootView = toolbarView()
  }

  func stopLoading() {
    loadingTask?.cancel()
    loadingTask = nil
    finishLoading()
  }

  func showOpeningProgress() {
    if openingProgress == nil { openingProgress = SlopOpeningProgress(started: startupStarted) }
    openingProgress?.onCancel = { [weak self] in
      self?.loadingTask?.cancel()
      self?.request(.close)
    }
    openingProgress?.focus()
  }

  func revealReadyWindow() {
    super.showWindow(nil)
    window?.deminiaturize(nil)
    window?.makeKeyAndOrderFront(nil)
    refreshIssueBadge()
    if isContentReady {
      window?.makeFirstResponder(session.webView)
      recordStartup("content-visible")
    }
  }

  func waitForPresentation() async { await loadingTask?.value }
}
