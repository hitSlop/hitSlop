import CryptoKit
import Foundation
import HitSlopCore
import HitSlopDocument

/// Runs one document request as `hitslop-native request` does, and maps its reply.
@MainActor public func command(_ method: String, url: URL, _ fields: [String: Any] = [:]) async throws -> DecodedReply {
  var request = fields
  request["method"] = method
  request["documentPath"] = url.path
  return try decodeReply(await DocumentCommand.run(json: JSONSerialization.data(withJSONObject: request)))
}
/// A successful request's state, as JSON data.
@MainActor public func commandState(_ method: String, url: URL, _ fields: [String: Any] = [:]) async throws -> Data {
  let reply = try await command(method, url: url, fields)
  guard reply.ok else { throw SlopFailure(reply.error ?? "Request failed") }
  return try JSONSerialization.data(withJSONObject: reply.state ?? [:], options: [.fragmentsAllowed, .sortedKeys])
}

/// Stores `bytes` and sets `path` to its ID in one agent batch, as `slop apply --attach`
/// does, and returns the ID.
@MainActor public func attach(_ bytes: Data, at path: [String], url: URL) async throws -> String {
  let id = SHA256.hash(data: bytes).map { String(format: "%02x", $0) }.joined()
  let ops = String(decoding: try JSONSerialization.data(withJSONObject: [["type": "set", "path": path, "value": id]]), as: UTF8.self)
  let reply = try await command("batch", url: url, ["ops": ops, "attachments": [bytes.base64EncodedString()]])
  guard reply.ok else { throw SlopFailure(reply.error ?? "Attach failed") }
  return id
}
/// Sets palette colors as an agent's batch; `nil` returns a color to the template's.
@MainActor public func setTheme(_ values: [String: String?], url: URL, replace: Bool = false) async throws -> DecodedReply {
  try await themeCommand(["type": "setTheme", "values": values.mapValues { $0 ?? NSNull() as Any }, "replace": replace], url: url)
}
/// One palette intent (`setTheme` or `importTheme`), as an agent's batch.
@MainActor public func themeCommand(_ intent: [String: Any], url: URL) async throws -> DecodedReply {
  let ops = String(decoding: try JSONSerialization.data(withJSONObject: [intent]), as: UTF8.self)
  return try await command("batch", url: url, ["ops": ops])
}
/// The effective palette, as `get` reports it.
@MainActor public func effectiveTheme(url: URL) async throws -> [String: String] {
  let reply = try await command("get", url: url)
  guard reply.ok, let frame = reply.state as? [String: Any], let state = frame["state"] as? [String: Any],
    let theme = state["theme"] as? [String: String] else { throw SlopFailure(reply.error ?? "Request failed") }
  return theme
}

/// A value shared with concurrently running handlers in tests.
public final class Locked<Value>: @unchecked Sendable {
  private let lock = NSLock()
  private var stored: Value
  public init(_ value: Value) { stored = value }
  public var value: Value { lock.withLock { stored } }
  public func modify(_ change: (inout Value) -> Void) { lock.withLock { change(&stored) } }
}

/// Tests inspect payloads as well as the header; production routing never parses them.
public struct DecodedReply {
  public let header: SocketReplyHeader
  public let state: Any?
  public let ids: [String]?
  public let sequence: Int?
  public let output: String?
  public var ok: Bool { header.ok }
  public var error: String? { header.error }
  public var code: OutcomeCode? { header.code }
  public var reason: CoreErrorCode? { header.reason }
  public var opIndex: Int? { header.opIndex }
}
public func decodeReply(_ data: Data) throws -> DecodedReply {
  let header = try JSONDecoder().decode(SocketReplyHeader.self, from: data)
  let value = try JSONSerialization.jsonObject(with: data) as! [String: Any]
  return DecodedReply(header: header, state: value["state"], ids: value["ids"] as? [String],
    sequence: value["sequence"] as? Int, output: value["output"] as? String)
}
