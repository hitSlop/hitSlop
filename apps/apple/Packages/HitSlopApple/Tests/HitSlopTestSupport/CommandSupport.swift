import Foundation
import HitSlopCore
import HitSlopDocument

/// Runs one document request as `hitslop-native request` does (the CLI sends no epoch),
/// and maps its reply.
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
  public var epoch: String? { header.epoch }
  public var opIndex: Int? { header.opIndex }
}
public func decodeReply(_ data: Data) throws -> DecodedReply {
  let header = try JSONDecoder().decode(SocketReplyHeader.self, from: data)
  let value = try JSONSerialization.jsonObject(with: data) as! [String: Any]
  return DecodedReply(header: header, state: value["state"], ids: value["ids"] as? [String],
    sequence: value["sequence"] as? Int, output: value["output"] as? String)
}
