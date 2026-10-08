import CoreGraphics
import HitSlopCore
import HitSlopDocument
import WebKit

/// A session delegate for tests: an event goes to its closure when one is set, and
/// otherwise to `next` (usually the window controller). The session holds its delegate
/// weakly, so a test keeps this alive for as long as it listens.
@MainActor public final class SessionEvents: DocumentSessionDelegate {
  public weak var next: DocumentSessionDelegate?
  public var status: ((DocumentSaveStatus) -> Void)?
  public var issue: ((SlopPageIssue) -> Void)?
  public var failure: ((Error) -> Void)?
  public var recovered: (() -> Void)?
  public var resize: ((CGSize) throws -> CGSize)?
  public var theme: ((SlopThemeState) -> Void)?
  public init(next: DocumentSessionDelegate? = nil) { self.next = next }

  public func pageSessionDidBecomeReady(_ session: DocumentSession) { next?.pageSessionDidBecomeReady(session) }
  public func pageSessionRecovered(_ session: DocumentSession) {
    if let recovered { recovered() } else { next?.pageSessionRecovered(session) }
  }
  public func pageSession(_ session: DocumentSession, saveStatus: DocumentSaveStatus) {
    if let status { status(saveStatus) } else { next?.pageSession(session, saveStatus: saveStatus) }
  }
  public func pageSession(_ session: DocumentSession, storageFailure: SlopFailureContext) {
    next?.pageSession(session, storageFailure: storageFailure)
  }
  public func pageSession(_ session: DocumentSession, themeChanged theme: SlopThemeState) {
    if let observe = self.theme { observe(theme) } else { next?.pageSession(session, themeChanged: theme) }
  }
  public func pageSession(_ session: DocumentSession, didReport issue: SlopPageIssue) {
    if let report = self.issue { report(issue) } else { next?.pageSession(session, didReport: issue) }
  }
  public func pageSession(_ session: DocumentSession, didFail error: Error) {
    if let failure { failure(error) } else { next?.pageSession(session, didFail: error) }
  }
  public func pageSession(_ session: DocumentSession, didReplace webView: WKWebView) {
    next?.pageSession(session, didReplace: webView)
  }
  public func pageSession(_ session: DocumentSession, resizeContentTo size: CGSize) throws -> CGSize {
    if let resize { return try resize(size) }
    guard let next else { return size }
    return try next.pageSession(session, resizeContentTo: size)
  }
}
