import Foundation
import HitSlopCore
import HitSlopDocument
import WebKit

@MainActor public protocol SlopRuntimeSessionDelegate: AnyObject {
  func runtimeSessionDidBecomeReady(_ session: SlopRuntimeSession)
  func runtimeSession(_ session: SlopRuntimeSession, resizeContentTo size: CGSize) throws -> CGSize
  func runtimeSessionRecovered(_ session: SlopRuntimeSession)
  func runtimeSession(_ session: SlopRuntimeSession, didReport issue: SlopRuntimeIssue)
  func runtimeSession(_ session: SlopRuntimeSession, didFail error: Error)
  func runtimeSession(_ session: SlopRuntimeSession, saveStatus: DocumentSaveStatus)
  func runtimeSession(_ session: SlopRuntimeSession, storageFailure: SlopFailureContext)
}
extension SlopRuntimeSessionDelegate {
  public func runtimeSessionDidBecomeReady(_ session: SlopRuntimeSession) {}
  public func runtimeSessionRecovered(_ session: SlopRuntimeSession) {}
  public func runtimeSession(_ session: SlopRuntimeSession, resizeContentTo size: CGSize) throws
    -> CGSize
  { throw SlopPackageError.invalid("Dynamic resizing unavailable") }
  public func runtimeSession(_ session: SlopRuntimeSession, didReport issue: SlopRuntimeIssue) {}
  public func runtimeSession(_ session: SlopRuntimeSession, didFail error: Error) {}
  public func runtimeSession(_ session: SlopRuntimeSession, saveStatus: DocumentSaveStatus) {}
  public func runtimeSession(_ session: SlopRuntimeSession, storageFailure: SlopFailureContext) {}
}
@MainActor public final class SlopRuntimeSession {
  public let package: SlopPackage
  public let purpose: SlopRuntimePurpose
  public let engine: DocumentSession
  public var webView: WKWebView { engine.webView }
  public var isReady: Bool { engine.isReady }
  public weak var delegate: (any SlopRuntimeSessionDelegate)?
  public convenience init(
    packageURL: URL, renderTargetsEnabled: Bool = false, purpose: SlopRuntimePurpose = .interactive
  ) throws {
    try SlopLocalDocument.requireLocal(packageURL)
    let engine = try DocumentSession(
      package: SlopPackage(rootURL: packageURL), storage: purpose.storageMode)
    self.init(engine: engine, renderTargetsEnabled: renderTargetsEnabled, purpose: purpose)
  }
  private init(engine: DocumentSession, renderTargetsEnabled: Bool, purpose: SlopRuntimePurpose) {
    self.engine = engine
    package = engine.package
    self.purpose = purpose
    engine.allowsFileSelection = purpose == .interactive
    engine.onResize = { [weak self] size in
      guard let self, let delegate = self.delegate else {
        throw SlopPackageError.invalid("No window")
      }
      return try delegate.runtimeSession(self, resizeContentTo: size)
    }
    engine.onReady = { [weak self] in
      guard let self else { return }
      self.delegate?.runtimeSessionDidBecomeReady(self)
    }
    engine.onStorageFailure = { [weak self] diagnostic in
      guard let self else { return }
      self.delegate?.runtimeSession(self, storageFailure: diagnostic)
    }
    engine.onStatus = { [weak self] status in
      guard let self else { return }
      self.delegate?.runtimeSession(self, saveStatus: status)
    }
    engine.onRecovered = { [weak self] in
      guard let self else { return }
      self.delegate?.runtimeSessionRecovered(self)
    }
    engine.onIssue = { [weak self] message, operation in
      guard let self else { return }
      self.delegate?.runtimeSession(
        self,
        didReport: SlopRuntimeIssue(source: operation ? .document : .unhandled, message: message))
    }
    engine.onError = { [weak self] message in
      guard let self else { return }
      self.delegate?.runtimeSession(self, didFail: SlopDiagnosticError(
        SlopPackageError.invalid(message), diagnostic: .init(self.engine.failureClassification, reason: self.engine.failureReason ?? .startup)))
    }
    if renderTargetsEnabled {
      webView.configuration.userContentController.addUserScript(
        WKUserScript(
          source: "document.documentElement.setAttribute('data-slop-renderer','true')",
          injectionTime: .atDocumentStart, forMainFrameOnly: true))
    }
  }
  public static func open(
    packageURL: URL, renderTargetsEnabled: Bool = false, purpose: SlopRuntimePurpose = .interactive
  ) async throws -> SlopRuntimeSession {
    let engine = try await DocumentSession.open(packageURL: packageURL, storage: purpose.storageMode)
    return SlopRuntimeSession(
      engine: engine, renderTargetsEnabled: renderTargetsEnabled, purpose: purpose)
  }
  public func load() { engine.load() }
  public func waitUntilReady(timeout: Duration = .seconds(15)) async throws {
    try await engine.waitUntilReady(timeout: timeout)
  }
  public func flush() async throws { try await engine.flush() }
  public func finish() async throws { try await engine.close() }
  public func reopenSavedDocument() async throws { try await engine.reopenSavedDocument() }
}
