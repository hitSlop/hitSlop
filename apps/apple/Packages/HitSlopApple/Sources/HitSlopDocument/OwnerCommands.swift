import Foundation
import HitSlopCore
import HitSlopCoreBinding

extension DocumentOwner {
  static func validates(_ value: Any, contract: String) -> Bool {
    guard let properties = ownerContractsSchema["properties"] as? [String: Any],
      let schema = properties[contract] as? [String: Any] else { return false }
    return PlatformContract.valid(value, against: schema)
  }

  /// Page requests. Envelopes are validated here; Rust remains the semantic authority.
  /// Every reply carries the request `id`; failures carry a typed code, never a guess.
  @MainActor func bridge(_ args: [String: Any]) async -> [String: Any] {
    let id = args["id"] as? String ?? "invalid"
    guard Self.validates(args, contract: "request"), let method = args["method"] as? String,
      let view = args["view"] as? String
    else { return ["id": id, "ok": false, "code": "rejected", "error": "Invalid page request"] }
    do {
      switch method {
      case "open":
        let opened = try await open(view: view)
        return ["id": id, "ok": true, "state": try JSONSerialization.jsonObject(with: Data(opened.state.utf8)),
          "savedSequence": opened.savedSequence, "saveFailure": opened.saveFailure as Any? ?? NSNull()]
      case "apply":
        let batch = String(decoding: try JSONSerialization.data(withJSONObject: args["batch"]!), as: UTF8.self)
        let applied = try await apply(batch: batch, view: view)
        return ["id": id, "ok": true, "sequence": applied.sequence, "ids": applied.ids]
      case "text":
        let request = String(decoding: try JSONSerialization.data(withJSONObject: args["request"]!), as: UTF8.self)
        let edit = try await editText(request, view: view)
        return ["id": id, "ok": true, "sequence": edit.sequence, "authored": edit.authored,
          "selectionStart": edit.selectionStart, "selectionEnd": edit.selectionEnd]
      default:
        try await flush()
        return ["id": id, "ok": true]
      }
    } catch {
      return ["id": id, "ok": false, "code": Self.pageCode(error, flushing: method == "flush"),
        "error": error.localizedDescription]
    }
  }
  /// `rejected`, `owner_replaced` and `closing` were not applied; the others leave the
  /// page to read state before relying on the outcome.
  static func pageCode(_ error: Error, flushing: Bool) -> String {
    if error is OwnerReplaced { return "owner_replaced" }
    if error.isOwnerInvalidation { return "owner_invalidated" }
    if case CoreError.Rejected? = error as? CoreError { return "rejected" }
    if error is SaveFailure || flushing { return "save_failed" }
    if error.localizedDescription.contains("closing") || error.localizedDescription.contains("closed") { return "closing" }
    return "unknown_outcome"
  }

  @MainActor func request(_ request: SocketRequest) async -> SocketReply {
    guard request.documentPath == package.rootURL.path else {
      return .init(ok: false, error: "Document path mismatch", code: .rejected)
    }
    if request.requiresEpoch, request.json["epoch"] as? String != epoch {
      return .init(ok: false, epoch: epoch, error: "Owner session changed", code: .sessionChanged)
    }
    var accepted = false
    var applied: (ids: [String], sequence: Int)?
    do {
      if request.method == .hello { return .init(ok: true, epoch: epoch) }
      if request.method == .apply || request.method == .batch {
        let intents = request.method == .apply ? [request.json["op"]!] : request.json["ops"]!
        let batch: [String: Any] = ["intents": intents]
        guard Self.validates(batch, contract: "batch") else {
          return .init(ok: false, epoch: epoch, error: "Invalid document command", code: .rejected)
        }
        let result = try await apply(batch: String(decoding: JSONSerialization.data(withJSONObject: batch), as: UTF8.self),
          epoch: request.json["epoch"] as? String)
        applied = (result.ids, result.sequence)
      }
      accepted = true
      if request.method == .compact { try await compact() }
      try await flush()
      switch request.method {
      case .get, .apply, .batch, .compact, .snapshot:
        let frame = try JSONSerialization.jsonObject(with: Data(await state().utf8)) as! [String: Any]
        let result: Any = request.method == .snapshot ? ["data": frame["value"]!, "version": frame["version"]!, "issues": frame["issues"]!, "schema": try JSONSerialization.jsonObject(with: SlopFile.read(package.dataSchemaURL, within: package.rootURL))] : frame["value"]!
        return .init(ok: true, epoch: epoch, state: result, ids: applied?.ids, sequence: applied?.sequence)
      case .schema:
        return .init(ok: true, epoch: epoch, state: try JSONSerialization.jsonObject(with: SlopFile.read(package.dataSchemaURL, within: package.rootURL)))
      case .themeGet, .themeSet, .themeReset:
        let defaults = try themeDefaults()
        var overrides = try await loadTheme()
        if request.method == .themeSet {
          guard let values = request.json["values"] as? [String: String] else { throw failure("Invalid theme values") }
          overrides.merge(values) { _, new in new }
        } else if request.method == .themeReset {
          if let token = request.json["token"] as? String {
            guard defaults[token] != nil else { throw failure("Unknown theme token") }
            overrides.removeValue(forKey: token)
          } else { overrides.removeAll() }
        }
        if request.method != .themeGet {
          try await saveTheme(overrides)
        }
        return .init(ok: true, epoch: epoch, state: ["defaults": defaults, "overrides": overrides, "effective": defaults.merging(overrides) { _, new in new }])
      case .attachmentsPut, .attachmentsRead, .attachmentsList:
        let state: Any
        switch request.method {
        case .attachmentsList: state = try await listAttachments()
        case .attachmentsRead:
          guard let id = request.json["attachmentID"] as? String else { throw failure("Missing attachment ID") }
          state = ["bytes": try await readAttachment(id).base64EncodedString()]
        default:
          guard let encoded = request.json["bytes"] as? String, encoded.utf8.count <= 13_981_016,
            let bytes = Data(base64Encoded: encoded) else { throw failure("Invalid attachment bytes") }
          state = try await putAttachment(bytes)
        }
        return .init(ok: true, epoch: epoch, state: state)
      default: return .init(ok: false, epoch: epoch, error: "Unsupported owner command", code: .rejected)
      }
    } catch {
      if !accepted, error is OwnerReplaced {
        return .init(ok: false, epoch: epoch, error: error.localizedDescription, code: .sessionChanged)
      }
      if !accepted, case CoreError.Rejected = error {
        return .init(ok: false, epoch: epoch, error: error.localizedDescription, code: .rejected)
      }
      return .init(ok: false, epoch: epoch, error: error.localizedDescription, code: .failed)
    }
  }
}
