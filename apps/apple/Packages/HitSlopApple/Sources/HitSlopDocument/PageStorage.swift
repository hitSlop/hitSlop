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
  /// Page attachment calls. They go through the owner, so they pass its admission;
  /// document bytes never reach the page this way.
  func servePageStorage(_ request: PageRequest, reply: @escaping @MainActor @Sendable (Any?, String?) -> Void) {
    let owner = owner, view = view
    Task { @MainActor [weak self] in
      do {
        let result: PageResult
        switch request {
        case .attachmentsRead(let r):
          result = .attachmentsRead(.init(bytes: try await owner.readAttachment(r.attachmentID)))
        case .attachmentsPut(let r):
          result = .attachmentsPut(try await owner.putAttachment(base64: r.bytes, view: view))
        default: throw OwnerError.rejected("Unsupported storage request")
        }
        reply(result.json, nil)
      } catch {
        switch RequestOutcome(error) {
        case .saveFailed, .unknown:
          // Only persistence failures are storage failures; a closing, replaced or
          // invalidated owner is reported through its own lifecycle.
          if !SlopFailureContext.isCancellation(error) {
            let diagnostic = SlopFailureContext.classify(error)
            if let self {
              self.delegate?.pageSession(self, storageFailure: diagnostic.reason == .unknown ? .init(reason: .storage) : diagnostic)
            }
          }
        case .rejected, .replaced, .closing, .invalidated: break
        }
        reply(RequestOutcome.page(error), nil)
      }
    }
  }
}
