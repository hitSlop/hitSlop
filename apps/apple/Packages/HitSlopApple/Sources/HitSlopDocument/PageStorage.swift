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
      return reply(pageFailure(OwnerError.rejected("Invalid page request")), nil)
    }
    owner.page(json: String(decoding: json, as: UTF8.self), view: view) { [weak self] answer, failure in
      DispatchQueue.main.async {
        if storage, let failure, let self { self.reportStorage(failure) }
        reply(try? JSONSerialization.jsonObject(with: Data(answer.utf8)), nil)
      }
    }
  }

  /// Only persistence failures are storage failures: a refusal changed nothing, and a
  /// closing, replaced or invalidated owner is reported through its own lifecycle.
  private func reportStorage(_ failure: OwnerFailure) {
    switch failure.kind {
    case .locked, .busy, .full, .moved, .saveFailed, .failed:
      let diagnostic = SlopFailureContext.classify(failure.hostError)
      delegate?.pageSession(self, storageFailure: diagnostic.reason == .unknown ? .init(reason: .storage) : diagnostic)
    case .rejected, .readOnly, .replaced, .closing, .closed, .invalidated: break
    }
  }
}
