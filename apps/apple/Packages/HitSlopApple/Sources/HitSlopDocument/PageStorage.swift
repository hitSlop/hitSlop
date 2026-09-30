import Foundation
import HitSlopCore

/// Immutable, validated WebKit values cross the storage queue without a JSON copy.
struct StorageRequest: @unchecked Sendable {
  let value: [String: Any]

  init?(_ value: [String: Any]) {
    var bytes = 48 * 1024 * 1024
    var nodes = 16_384
    func bounded(_ value: Any, depth: Int) -> Bool {
      guard depth <= 16, nodes > 0 else { return false }
      nodes -= 1
      bytes -= 2
      if let text = value as? String { bytes -= text.utf8.count }
      else if let object = value as? [String: Any] {
        for (key, child) in object {
          bytes -= key.utf8.count + 3
          guard bytes >= 0, bounded(child, depth: depth + 1) else { return false }
        }
      } else if let array = value as? [Any] {
        for child in array {
          guard bounded(child, depth: depth + 1) else { return false }
        }
      } else if value is NSNumber { bytes -= 32 }
      else if !(value is NSNull) { return false }
      return bytes >= 0
    }
    guard bounded(value, depth: 0),
      let variants = bridgeValidationSchema["anyOf"] as? [[String: Any]],
      let schema = variants.first(where: {
        let properties = $0["properties"] as? [String: [String: Any]]
        return properties?["method"]?["const"] as? String == value["method"] as? String
      }),
      let properties = schema["properties"] as? [String: Any],
      value.keys.allSatisfy({ properties[$0] != nil }),
      PlatformContract.valid(value, against: schema)
    else { return nil }
    self.value = value
  }
}

extension DocumentSession {
  /// Page attachment and theme calls. They go through the owner, so they honor its closed
  /// and closing guards; document bytes never reach the page this way.
  func servePageStorage(_ request: StorageRequest, method: String,
    reply: @escaping @MainActor @Sendable (Any?, String?) -> Void
  ) {
    let owner = owner
    let args = request.value
    let values = args["values"] as? [String: String], attachment = args["attachmentID"] as? String
    let encoded = args["bytes"] as? String
    Task { @MainActor [weak self] in
      do {
        let value: [String: Any]
        switch method {
        case "attachments.list": value = ["files": try await owner.listAttachments()]
        case "attachments.read":
          guard let attachment else { throw failure("Missing attachment ID") }
          value = ["bytes": try await owner.readAttachment(attachment).base64EncodedString()]
        case "attachments.put":
          guard let encoded, encoded.utf8.count <= 13_981_016, let bytes = Data(base64Encoded: encoded) else {
            throw failure("Invalid attachment bytes")
          }
          value = try await owner.putAttachment(bytes)
        case "theme.load": value = ["values": try await owner.loadTheme()]
        case "theme.save":
          guard let values else { throw failure("Invalid theme values") }
          try await owner.saveTheme(values)
          value = [:]
        default: throw failure("Unsupported storage request")
        }
        reply(value, nil)
      } catch let error as SlopRejection {
        reply(["rejected": error.localizedDescription], nil)
      } catch {
        if !SlopFailureContext.isCancellation(error) {
          let diagnostic = SlopFailureContext.classify(error)
          self?.onStorageFailure?(diagnostic.reason == .unknown || error is SlopPackageError
            ? .init(reason: .storage) : diagnostic)
        }
        reply(nil, error.localizedDescription)
      }
    }
  }
}
