import AppKit
import HitSlopCore
import HitSlopCoreBinding
@preconcurrency import WebKit

/// WebKit navigation, download and file-selection policy for the current page.
extension DocumentSession: WKNavigationDelegate, WKUIDelegate {
  public func webViewWebContentProcessDidTerminate(_ webView: WKWebView) {
    guard isCurrent(webView) else { return }
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
    guard isCurrent(webView), allowsFileSelection, !capturing,
      acceptsPageWork,
      frame.isMainFrame, frame.securityOrigin.protocol == "slop",
      frame.securityOrigin.host == "app", let window = webView.window,
      window.isVisible
    else {
      completionHandler(nil)
      return
    }
    filePicker.present(
      in: window, multiple: parameters.allowsMultipleSelection,
      directories: parameters.allowsDirectories, completion: completionHandler)
  }

  /// Camera and microphone requests (`getUserMedia`). macOS still asks the person once for
  /// hitSlop; this decides which pages may ask at all.
  public func webView(
    _ webView: WKWebView, requestMediaCapturePermissionFor origin: WKSecurityOrigin,
    initiatedByFrame frame: WKFrameInfo, type: WKMediaCaptureType,
    decisionHandler: @escaping @MainActor (WKPermissionDecision) -> Void
  ) {
    let admitted = admitsMediaCapture(
      type, isMainFrame: frame.isMainFrame,
      origins: [(origin.protocol, origin.host), (frame.securityOrigin.protocol, frame.securityOrigin.host)],
      in: webView)
    decisionHandler(admitted ? .grant : .deny)
  }

  /// The interactive page of an owned document, shown in a visible window, may capture what
  /// its app declared. Snapshot, export and icon renderers, embedded frames, a page being
  /// captured or replaced, and any device the app did not declare are refused.
  func admitsMediaCapture(
    _ type: WKMediaCaptureType, isMainFrame: Bool, origins: [(protocol: String, host: String)],
    in webView: WKWebView
  ) -> Bool {
    guard isCurrent(webView), admitsPageServices, isMainFrame,
      origins.allSatisfy({ $0.protocol == "slop" && $0.host == "app" }),
      webView.window?.isVisible == true,
      let required = Self.capabilities(capturing: type)
    else { return false }
    return required.isSubset(of: file.capabilities)
  }

  /// Device and host services (capture, speech) serve only the ready page of an owned
  /// document: never a snapshot, export or icon renderer, a page being captured, or one
  /// that is opening, closing or retired.
  var admitsPageServices: Bool {
    acceptsPageWork && !capturing && owner.mode == .document
  }

  /// The declaration a capture needs: both devices for a combined request.
  static func capabilities(capturing type: WKMediaCaptureType) -> Set<Capability>? {
    switch type {
    case .camera: [.camera]
    case .microphone: [.microphone]
    case .cameraAndMicrophone: [.camera, .microphone]
    @unknown default: nil
    }
  }

  public func webView(
    _ webView: WKWebView, navigationAction: WKNavigationAction, didBecome download: WKDownload
  ) {
    guard isCurrent(webView), allowsFileSelection, !capturing,
      acceptsPageWork
    else {
      download.cancel { _ in }
      return
    }
    fileSaver.begin(download)
  }

  public func webView(
    _ webView: WKWebView, didFailProvisionalNavigation navigation: WKNavigation!,
    withError error: Error
  ) {
    guard isCurrent(webView), isActive,
      !SlopFailureContext.isCancellation(error)
    else { return }
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
      isCurrent(webView), allowsFileSelection, !capturing, acceptsPageWork,
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
