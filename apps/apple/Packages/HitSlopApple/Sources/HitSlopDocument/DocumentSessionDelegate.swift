import CoreGraphics
import HitSlopCore
import WebKit

/// What a page session reports to the window that shows it, on the main actor.
@MainActor public protocol DocumentSessionDelegate: AnyObject {
  /// The page mounted the app and its fonts settled.
  func pageSessionDidBecomeReady(_ session: DocumentSession)
  /// A reloaded interface mounted again.
  func pageSessionRecovered(_ session: DocumentSession)
  func pageSession(_ session: DocumentSession, saveStatus: DocumentSaveStatus)
  /// An attachment request failed in storage.
  func pageSession(_ session: DocumentSession, storageFailure: SlopFailureContext)
  /// The page now shows this palette, after a change from the panel, the CLI or an agent.
  func pageSession(_ session: DocumentSession, themeChanged theme: SlopThemeState)
  /// An issue the app or a document operation reported; the page keeps running.
  func pageSession(_ session: DocumentSession, didReport issue: SlopPageIssue)
  /// The page could not open, or its renderer stopped. The error carries its diagnostic.
  func pageSession(_ session: DocumentSession, didFail error: Error)
  /// A new page replaced a failed one, or one showing discarded edits.
  func pageSession(_ session: DocumentSession, didReplace webView: WKWebView)
  /// The app asked for a content size; returns the size the window took.
  func pageSession(_ session: DocumentSession, resizeContentTo size: CGSize) throws -> CGSize
}
