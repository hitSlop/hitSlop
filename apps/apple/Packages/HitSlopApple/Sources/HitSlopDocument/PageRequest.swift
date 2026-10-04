import Foundation
import HitSlopCore
import HitSlopCoreBinding

/// Bounded, flat page envelopes. Only the core parses batch and text payloads.
struct PageRequest: @unchecked Sendable {
  let value: [String: Any]
  let method: PageMethod

  init(_ value: [String: Any]) throws {
    guard value.count <= 8, value.values.allSatisfy({ $0 is String || $0 is NSNumber }),
      let raw = value["method"] as? String, let method = PageMethod(rawValue: raw)
    else { throw OwnerError.rejected("Invalid page request") }
    for key in ["batch", "request"] {
      if let text = value[key] as? String, text.utf8.count > Limits.pagePayload { throw OwnerError.tooLarge }
    }
    guard Envelope.valid(.pageRequest, object: value) else { throw OwnerError.rejected("Invalid page request") }
    self.value = value
    self.method = method
  }
}

extension DocumentSession {
  /// Page attachment calls. They go through the owner, so they honor its closed
  /// and closing guards; document bytes never reach the page this way.
  func servePageStorage(_ request: PageRequest, reply: @escaping @MainActor @Sendable (Any?, String?) -> Void) {
    let owner = owner
    let args = request.value
    let method = request.method
    Task { @MainActor [weak self] in
      do {
        let result: PageResult
        switch method {
        case .attachmentsRead:
          result = .attachmentsRead(.init(bytes: try await owner.readAttachment(args["attachmentID"] as? String ?? "")))
        case .attachmentsPut:
          let stored = try await owner.putAttachment(base64: args["bytes"] as? String ?? "")
          result = .attachmentsPut(.init(id: stored.id, byteLength: stored.byteLength))
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
              self.delegate?.pageSession(self, storageFailure: diagnostic.reason == .unknown || error is SlopError
                ? .init(reason: .storage) : diagnostic)
            }
          }
        case .rejected, .replaced, .closing, .invalidated: break
        }
        reply(DocumentOwner.pageFailure(error), nil)
      }
    }
  }
}
