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

/// Serves host-owned themes and attachments; document persistence bypasses this bridge; WebKit lifecycle stays in DocumentSession.
@MainActor final class StorageBridge {
  private struct Reply: @unchecked Sendable { let value: [String: Any] }
  private let storage: Storage
  var onFailure: ((SlopFailureContext) -> Void)?

  init(storage: Storage) { self.storage = storage }

  func call(_ request: StorageRequest, method: String,
    reply: @escaping @MainActor @Sendable (Any?, String?) -> Void
  ) {
    storage.queue.async { [storage, weak self] in
      let result: Result<Reply, Error> = Result {
        return Reply(value: try storage.call(request.value))
      }
      DispatchQueue.main.async { [weak self] in
        do {
          let value = try result.get().value
          reply(value, nil)
        } catch let error as SlopRejection {
          reply(["rejected": error.localizedDescription], nil)
        } catch {
          if !SlopFailureContext.isCancellation(error) {
            let diagnostic = SlopFailureContext.classify(error)
            self?.onFailure?(diagnostic.reason == .unknown || error is SlopPackageError
              ? .init(reason: .storage) : diagnostic)
          }
          reply(nil, error.localizedDescription)
        }
      }
    }
  }
}
