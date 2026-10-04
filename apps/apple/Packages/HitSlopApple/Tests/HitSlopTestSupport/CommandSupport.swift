import Foundation
import HitSlopCore
import HitSlopDocument

/// Runs one document request as `hitslop-native request` does (the CLI sends no epoch),
/// and maps its reply.
@MainActor public func command(_ method: String, url: URL, _ fields: [String: Any] = [:]) async throws -> SocketReply {
  var request = fields
  request["method"] = method
  request["documentPath"] = url.path
  return try decodeReply(await DocumentCommand.run(json: JSONSerialization.data(withJSONObject: request)))
}
/// A successful request's state, as JSON data.
@MainActor public func commandState(_ method: String, url: URL, _ fields: [String: Any] = [:]) async throws -> Data {
  let reply = try await command(method, url: url, fields)
  guard reply.ok else { throw NSError(domain: "hitSlop", code: 1, userInfo: [NSLocalizedDescriptionKey: reply.error ?? "Request failed"]) }
  return try JSONSerialization.data(withJSONObject: reply.state ?? [:], options: [.fragmentsAllowed, .sortedKeys])
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
