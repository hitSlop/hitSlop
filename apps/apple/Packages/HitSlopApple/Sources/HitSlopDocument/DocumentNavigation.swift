import AppKit
import HitSlopCore
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
