import Foundation
import HitSlopCore
import HitSlopCoreBinding

extension DocumentOwner {
  static func validates(_ value: Any, contract: String) -> Bool {
    guard let properties = ownerContractsSchema["properties"] as? [String: Any],
      let schema = properties[contract] as? [String: Any] else { return false }
    return PlatformContract.valid(value, against: schema)
  }

  /// The bridge accepts validated envelopes; Rust remains the semantic authority.
  @MainActor func bridge(_ args: [String: Any]) async throws -> [String: Any] {
    guard Self.validates(args, contract: "request"), let method = args["method"] as? String else {
      throw failure("Invalid owner request")
    }
    if let requested = args["session"] as? String, requested != session { throw failure("session_changed") }
    switch method {
    case "state": return ["state": try JSONSerialization.jsonObject(with: Data(await state().utf8))]
    case "flush": try await flush(); return [:]
    case "apply":
      let batch = String(decoding: try JSONSerialization.data(withJSONObject: args["batch"]!), as: UTF8.self)
      let reply = try await apply(id: args["id"] as! String, session: args["session"] as! String, batch: batch)
      return ["publication": try JSONSerialization.jsonObject(with: Data(reply.utf8))]
    case "text":
      let request = try JSONSerialization.data(withJSONObject: args["request"]!)
      let reply = try await text(String(decoding: request, as: UTF8.self))
      return ["publication": try JSONSerialization.jsonObject(with: Data(reply.utf8))]
    case "releaseDraft": try await releaseDraft(args["draft"] as! String); return [:]
    default: throw failure("Unsupported owner method")
    }
  }

  @MainActor func request(_ request: SocketRequest) async -> SocketReply {
    guard request.documentPath == package.rootURL.path else {
      return .init(ok: false, error: "Document path mismatch", code: .rejected)
    }
    if request.requiresEpoch, request.json["epoch"] as? String != session {
      return .init(ok: false, epoch: session, error: "Owner session changed", code: .sessionChanged)
    }
    var accepted = false
    do {
      if request.method == .hello { return .init(ok: true, epoch: session) }
      if request.method == .apply || request.method == .batch {
        let intents = request.method == .apply ? [request.json["op"]!] : request.json["ops"]!
        let batch: [String: Any] = ["intents": intents]
        guard Self.validates(batch, contract: "command") else {
          return .init(ok: false, epoch: session, error: "Invalid contract-4 command", code: .rejected)
        }
        _ = try await apply(id: request.json["id"] as! String, session: session,
          batch: String(decoding: JSONSerialization.data(withJSONObject: batch), as: UTF8.self), current: true)
      }
      accepted = true
      if request.method == .compact { try await compact() }
      try await flush()
      switch request.method {
      case .get, .apply, .batch, .compact, .snapshot:
        let frame = try JSONSerialization.jsonObject(with: Data(await state().utf8)) as! [String: Any]
        let result: Any = request.method == .snapshot ? ["data": frame["value"]!, "version": frame["version"]!, "issues": frame["issues"]!, "schema": try JSONSerialization.jsonObject(with: SlopFile.read(package.dataSchemaURL, within: package.rootURL))] : frame["value"]!
        return .init(ok: true, epoch: session, state: result)
      case .schema:
        return .init(ok: true, epoch: session, state: try JSONSerialization.jsonObject(with: SlopFile.read(package.dataSchemaURL, within: package.rootURL)))
      case .themeGet, .themeSet, .themeReset:
        let defaults = try JSONSerialization.jsonObject(with: SlopFile.read(package.rootURL.appendingPathComponent("assets/theme.json"), within: package.rootURL, maximumBytes: 65536)) as! [String: String]
        let loaded = try JSONSerialization.jsonObject(with: await ancillary(Data(#"{"method":"theme.load"}"#.utf8))) as! [String: Any]
        var overrides = loaded["values"] as? [String: String] ?? [:]
        if request.method == .themeSet {
          guard let values = request.json["values"] as? [String: String] else { throw failure("Invalid theme values") }
          for (key, value) in values {
            guard defaults[key] != nil, !value.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty,
              value.count <= 4096, value.rangeOfCharacter(from: CharacterSet(charactersIn: "{};")) == nil else { throw failure("Invalid theme token or value") }
            let regex = try NSRegularExpression(pattern: #"var\(\s*--slop-([a-zA-Z0-9-]+)"#)
            for match in regex.matches(in: value, range: NSRange(value.startIndex..., in: value)) {
              guard let range = Range(match.range(at: 1), in: value), defaults[String(value[range])] != nil else { throw failure("Unknown theme reference") }
            }
            overrides[key] = value
          }
        } else if request.method == .themeReset {
          if let token = request.json["token"] as? String {
            guard defaults[token] != nil else { throw failure("Unknown theme token") }
            overrides.removeValue(forKey: token)
          } else { overrides.removeAll() }
        }
        if request.method != .themeGet {
          _ = try await ancillary(JSONSerialization.data(withJSONObject: ["method": "theme.save", "values": overrides]))
        }
        return .init(ok: true, epoch: session, state: ["defaults": defaults, "overrides": overrides, "effective": defaults.merging(overrides) { _, new in new }])
      case .attachmentsPut, .attachmentsRead, .attachmentsList:
        var input = request.json
        input.removeValue(forKey: "id"); input.removeValue(forKey: "documentPath"); input.removeValue(forKey: "epoch")
        let result = try JSONSerialization.jsonObject(with: await ancillary(JSONSerialization.data(withJSONObject: input))) as! [String: Any]
        return .init(ok: true, epoch: session, state: request.method == .attachmentsList ? result["files"] : result)
      default: return .init(ok: false, epoch: session, error: "Unsupported owner command", code: .rejected)
      }
    } catch {
      if !accepted, case BridgeError.Failure(let message) = error,
        !message.contains("engine_panic"), !message.contains("owner_poisoned") {
        return .init(ok: false, epoch: session, error: message, code: .rejected)
      }
      return .init(ok: false, epoch: session, error: error.localizedDescription, code: .failed)
    }
  }
}
