import Foundation
import HitSlopCore
import HitSlopCoreBinding

extension DocumentSession {
  /// Swift carries opaque JSON. Rust routes it and admits it under the owner view fence.
  func servePage(_ json: String, reply: @escaping @MainActor @Sendable (Any?, String?) -> Void) {
    let requestedView = view
    owner.page(json: json, view: requestedView) { [weak self] result in
      DispatchQueue.main.async {
        switch result {
        case .reply(let json, let failure, let storage):
          if storage, let failure, let self { self.reportStorage(failure) }
          // An accepted document edit keeps its actual answer even if its page retired.
          reply(json, nil)
        case .host(let action):
          guard let self, self.view == requestedView else {
            return reply(pageFailure(OwnerReplaced()), nil)
          }
          self.serveHost(action, reply: reply)
        }
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
