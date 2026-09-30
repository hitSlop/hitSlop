import Foundation
import HitSlopCore
import HitSlopCoreBinding

/// A validated host bridge message (everything but document requests). Every bridge
/// contract is a flat object, so anything nested is refused before it is serialized; the
/// core then checks the JSON against the generated bridge contract.
struct BridgeRequest: @unchecked Sendable {
  let value: [String: Any]
  let method: BridgeMethod

  init?(_ value: [String: Any]) {
    guard value.count <= 8, value.values.allSatisfy({ $0 is String || $0 is NSNumber }),
      let raw = value["method"] as? String, let method = BridgeMethod(rawValue: raw),
      Envelope.valid(.bridgeRequest, object: value)
    else { return nil }
    self.value = value
    self.method = method
  }
}

extension DocumentSession {
  /// Page attachment and theme calls. They go through the owner, so they honor its closed
  /// and closing guards; document bytes never reach the page this way.
  func servePageStorage(_ request: BridgeRequest, reply: @escaping @MainActor @Sendable (Any?, String?) -> Void) {
    let owner = owner
    let args = request.value
    let method = request.method
    Task { @MainActor [weak self] in
      do {
        let value: [String: Any]
        switch method {
        case .attachmentsList: value = ["files": try await owner.listAttachments().map(\.json)]
        case .attachmentsRead: value = ["bytes": try await owner.readAttachment(args["attachmentID"] as? String ?? "")]
        case .attachmentsPut: value = try await owner.putAttachment(base64: args["bytes"] as? String ?? "").json
        case .themeLoad:
          let values = try JSONSerialization.jsonObject(with: Data(try await owner.loadTheme().utf8))
          value = ["values": values]
        default: throw OwnerError.rejected("Unsupported storage request")
        }
        reply(value, nil)
      } catch {
        switch RequestOutcome(error) {
        case let .rejected(reason, opIndex):
          var failure: [String: Any] = ["rejected": error.localizedDescription, "reason": reason.rawValue]
          if let opIndex { failure["opIndex"] = opIndex }
          return reply(failure, nil)
        case .saveFailed, .unknown:
          // Only persistence failures are storage failures; a closing, replaced or
          // invalidated owner is reported through its own lifecycle.
          if !SlopFailureContext.isCancellation(error) {
            let diagnostic = SlopFailureContext.classify(error)
            if let self {
              self.delegate?.pageSession(self, storageFailure: diagnostic.reason == .unknown || error is SlopPackageError
                ? .init(reason: .storage) : diagnostic)
            }
          }
        case .replaced, .closing, .invalidated: break
        }
        reply(nil, error.localizedDescription)
      }
    }
  }
}
