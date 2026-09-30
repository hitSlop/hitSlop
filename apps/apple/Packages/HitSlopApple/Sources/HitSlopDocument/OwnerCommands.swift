import Foundation
import HitSlopCore
import HitSlopCoreBinding

extension DocumentOwner {
  /// Checks the envelope only. Batches and text edits arrive as JSON text that only the
  /// core parses, and the opened state returns as the core's JSON text.
  @MainActor func admitPage(_ args: [String: Any], reply: @escaping @MainActor @Sendable ([String: Any]) -> Void) {
    let id = args["id"] as? String ?? "invalid"
    let refuse = { (error: Error) in reply(Self.pageFailure(error, id: id)) }
    guard args["id"] is String, (1...128).contains(id.count), let view = args["view"] as? String,
      (1...128).contains(view.count), let method = args["method"] as? String
    else { return refuse(OwnerError.rejected("Invalid page request")) }
    func payload(_ key: String) throws -> String {
      guard args.count == 4, let text = args[key] as? String else { throw OwnerError.rejected("Invalid page request") }
      guard text.utf8.count <= Limits.pagePayload else { throw OwnerError.tooLarge }
      return text
    }
    do {
      let command: PageCommand
      switch method {
      case "apply": command = .apply(try payload("batch"))
      case "text": command = .text(try payload("request"))
      case "open", "flush":
        guard args.count == 3 else { throw OwnerError.rejected("Invalid page request") }
        command = method == "open" ? .open : .flush
      default: throw OwnerError.rejected("Invalid page request")
      }
      enqueuePage(command, view: view) { outcome in
        DispatchQueue.main.async {
          let fields: [String: Any]
          switch outcome {
          case .failure(let error): return reply(Self.pageFailure(error, id: id))
          case .success(.opened(let opened)):
            fields = ["state": opened.state, "savedSequence": opened.savedSequence, "saveFailure": opened.saveFailure as Any? ?? NSNull()]
          case .success(.applied(let applied)): fields = ["sequence": applied.sequence, "ids": applied.ids]
          case .success(.text(let edit)): fields = ["sequence": edit.sequence, "authored": edit.authored,
            "selectionStart": edit.selectionStart, "selectionEnd": edit.selectionEnd]
          case .success(.flushed): fields = [:]
          }
          reply(fields.merging(["id": id, "ok": true]) { _, new in new })
        }
      }
    } catch { refuse(error) }
  }
  static func pageFailure(_ error: Error, id: String) -> [String: Any] {
    let outcome = RequestOutcome(error)
    var fields: [String: Any] = ["id": id, "ok": false, "code": outcome.pageCode.rawValue, "error": error.localizedDescription]
    if case let .rejected(reason, opIndex) = outcome {
      fields["reason"] = reason.rawValue
      if let opIndex { fields["opIndex"] = opIndex }
    }
    return fields
  }

  /// A socket command, off the main actor. The reply is one JSON line; document state is
  /// the core's JSON, spliced in unparsed.
  func request(_ request: SocketRequest) async -> Data {
    let epoch = self.epoch
    guard request.documentPath == package.rootURL.path else {
      return SocketReply(ok: false, error: "Document path mismatch", code: .rejected).encoded()
    }
    if request.requiresEpoch, request.epoch != epoch {
      return SocketReply(ok: false, epoch: epoch, error: "Owner session changed", code: .sessionChanged).encoded()
    }
    // A lifecycle refusal only proves safe replay before the mutation is accepted.
    var mutationAccepted = false
    do {
      var reply = SocketReply(ok: true, epoch: epoch)
      let state: String
      switch request {
      case .hello:
        reply.coreBuildId = Self.coreBuildID
        return reply.encoded()
      case .get, .snapshot, .apply, .batch, .compact:
        switch request {
        case .apply(let r): try await accept(#"{"intents":["# + r.op + "]}", epoch: r.epoch, into: &reply)
        case .batch(let r): try await accept(#"{"intents":"# + r.ops + "}", epoch: r.epoch, into: &reply)
        case .compact: try await compact()
        default: break
        }
        mutationAccepted = reply.sequence != nil || request.method == .compact
        try await flush()
        if case .snapshot = request {
          let schema = String(decoding: try SlopFile.read(package.dataSchemaURL, within: package.rootURL, maximumBytes: 1_048_576), as: UTF8.self)
          state = #"{"schema":"# + schema + #","state":"# + (try await self.state()) + "}"
        } else {
          state = try await value()
        }
      case .themeGet, .themeSet, .themeReset:
        let change: ThemeChange
        switch request {
        case .themeSet(let r):
          change = .set(valuesJson: String(decoding: try JSONSerialization.data(withJSONObject: r.values), as: UTF8.self))
        case .themeReset(let r): change = .reset(token: r.token)
        default: change = .get
        }
        let theme = try await applyTheme(change)
        state = #"{"defaults":"# + theme.defaults + #","overrides":"# + theme.overrides + #","effective":"# + theme.effective + "}"
      case .attachmentsList:
        state = String(decoding: try JSONEncoder().encode(try await listAttachments()), as: UTF8.self)
      case .attachmentsRead(let r):
        state = String(decoding: try JSONEncoder().encode(["bytes": try await readAttachment(r.attachmentID)]), as: UTF8.self)
      case .attachmentsPut(let r):
        state = String(decoding: try JSONEncoder().encode(try await putAttachment(base64: r.bytes)), as: UTF8.self)
      case .export:
        return SocketReply(ok: false, epoch: epoch, error: "Unsupported owner command", code: .rejected).encoded()
      }
      return reply.encoded(state: state)
    } catch {
      if mutationAccepted {
        // OwnerReplaced's message says "not applied", which only describes admission.
        // A command already accepted may even be durable when discard rejects its flush.
        let message = (error as? SaveFailure)?.localizedDescription
          ?? "Command was accepted, but its final state could not be confirmed."
        return SocketReply(ok: false, epoch: epoch, error: message, code: .failed).encoded()
      }
      return SocketReply(ok: false, epoch: epoch, error: error.localizedDescription, code: RequestOutcome(error).socketCode).encoded()
    }
  }
  /// Applies a socket batch; the reply reports the inserted row IDs and the sequence.
  private func accept(_ batch: String, epoch: String, into reply: inout SocketReply) async throws {
    let applied = try await apply(batch: batch, epoch: epoch)
    reply.ids = applied.ids
    reply.sequence = applied.sequence
  }
}

extension SocketReply {
  /// The reply as JSON; `state`, already JSON text, is spliced in without parsing.
  func encoded(state: String? = nil) -> Data {
    guard var json = try? JSONSerialization.data(withJSONObject: self.json, options: .withoutEscapingSlashes)
    else { return Data(#"{"ok":false,"error":"Invalid response. Outcome unknown; run slop get before another edit."}"#.utf8) }
    if let state {
      json.removeLast()
      json.append(contentsOf: (json.count > 1 ? #","state":"# : #""state":"#).utf8)
      json.append(contentsOf: state.utf8)
      json.append(UInt8(ascii: "}"))
    }
    return json
  }
}

enum PageCommand: Sendable { case open, apply(String), text(String), flush }
enum PageResult: Sendable {
  case opened(DocumentOwner.Opened), applied(DocumentOwner.Applied), text(DocumentOwner.TextEdit), flushed
}
