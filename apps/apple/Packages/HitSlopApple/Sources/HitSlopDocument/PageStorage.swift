import Foundation
import HitSlopCore
import HitSlopCoreBinding

extension PageRequest {
  /// A page message, once the core finds its envelope matches the contract.
  static func checked(_ message: Any) -> PageRequest? {
    guard let object = message as? [String: Any], Envelope.valid(.pageRequest, object: object) else { return nil }
    return try? PageRequest(json: object)
  }
}

extension DocumentSession {
  /// A document request from the page. The owner checks and answers it, so document
  /// bytes and edits never pass through Swift; a refused attachment read or write that
  /// failed in storage is also the window's to report.
  func servePage(_ body: [String: Any], storage: Bool, reply: @escaping @MainActor @Sendable (Any?, String?) -> Void) {
    guard let json = try? JSONSerialization.data(withJSONObject: body) else {
      return reply(RequestOutcome.page(OwnerError.rejected("Invalid page request")), nil)
    }
    owner.page(json: String(decoding: json, as: UTF8.self), view: view) { answer, error in
      DispatchQueue.main.async { [weak self] in
        if storage, let error, let self { self.reportStorage(error) }
        reply(try? JSONSerialization.jsonObject(with: Data(answer.utf8)), nil)
      }
    }
  }

  /// Only persistence failures are storage failures; a closing, replaced or invalidated
  /// owner is reported through its own lifecycle.
  private func reportStorage(_ error: Error) {
    switch RequestOutcome(error) {
    case .saveFailed, .unknown:
      guard !SlopFailureContext.isCancellation(error) else { return }
      let diagnostic = SlopFailureContext.classify(error)
      delegate?.pageSession(self, storageFailure: diagnostic.reason == .unknown ? .init(reason: .storage) : diagnostic)
    case .rejected, .replaced, .closing, .invalidated: break
    }
  }
}
