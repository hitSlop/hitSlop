import Foundation
import HitSlopCore
import HitSlopDocument

/// Runs one CLI document command by its socket method name, as `hitslop-native` does.
@MainActor public func command(
  _ method: String, url: URL, operation: Data? = nil, operations: Data? = nil,
  themeValues: Data? = nil, themeToken: String? = nil, attachmentBytes: Data? = nil, attachmentID: String? = nil
) async throws -> Data {
  let op = operation.map { String(decoding: $0, as: UTF8.self) }
  let ops = operations.map { String(decoding: $0, as: UTF8.self) }
  let values = try themeValues.map { try JSONSerialization.jsonObject(with: $0) as! [String: String] }
  return try await DocumentCommand.run(url: url) { path in
    let id = UUID().uuidString
    switch method {
    case "apply": return .apply(.init(id: id, documentPath: path, epoch: "", op: op ?? ""))
    case "batch": return .batch(.init(id: id, documentPath: path, epoch: "", ops: ops ?? ""))
    case "compact": return .compact(.init(id: id, documentPath: path, epoch: ""))
    case "snapshot": return .snapshot(.init(id: id, documentPath: path))
    case "theme.get": return .themeGet(.init(id: id, documentPath: path))
    case "theme.set": return .themeSet(.init(id: id, documentPath: path, epoch: "", values: values ?? [:]))
    case "theme.reset": return .themeReset(.init(id: id, documentPath: path, epoch: "", token: themeToken))
    case "attachments.list": return .attachmentsList(.init(id: id, documentPath: path))
    case "attachments.read": return .attachmentsRead(.init(id: id, documentPath: path, attachmentID: attachmentID ?? ""))
    case "attachments.put": return .attachmentsPut(.init(id: id, documentPath: path, epoch: "", bytes: attachmentBytes?.base64EncodedString() ?? ""))
    default: return .get(.init(id: id, documentPath: path))
    }
  }
}

/// A value shared with concurrently running handlers in tests.
public final class Locked<Value>: @unchecked Sendable {
  private let lock = NSLock()
  private var stored: Value
  public init(_ value: Value) { stored = value }
  public var value: Value { lock.withLock { stored } }
  public func modify(_ change: (inout Value) -> Void) { lock.withLock { change(&stored) } }
}

/// A socket reply line as the client maps it.
public func decodeReply(_ data: Data) throws -> SocketReply {
  try SocketReply(json: JSONSerialization.jsonObject(with: data) as! [String: Any])
}
