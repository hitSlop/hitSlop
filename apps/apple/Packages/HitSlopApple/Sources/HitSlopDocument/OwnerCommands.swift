import Foundation
import HitSlopCore
import HitSlopCoreBinding

extension DocumentOwner {
  /// A document request from the page `view`. Batches and text edits arrive as JSON text
  /// that only the core parses, and the opened state returns as the core's JSON text.
  @MainActor func admitPage(_ request: PageRequest, view: String, reply: @escaping @MainActor @Sendable ([String: Any]) -> Void) {
    let command: PageCommand
    switch request {
    case .apply(let r): command = .apply(r.batch)
    case .text(let r): command = .text(r.request)
    case .open: command = .open
    case .flush: command = .flush
    case .undo: command = .undo
    case .redo: command = .redo
    default: return reply(RequestOutcome.page(OwnerError.rejected("Not a document request")))
    }
    enqueuePage(command, view: view) { outcome in
      DispatchQueue.main.async {
        switch outcome {
        case .failure(let error): reply(RequestOutcome.page(error))
        case .success(let result): reply(result.json)
        }
      }
    }
  }

  /// A socket command, off the main actor. The reply is one JSON line; document state is
  /// the core's JSON, spliced in unparsed. Mutations carry the epoch the client read, and
  /// the owner admits them against it.
  func request(_ request: SocketRequest) async -> Data {
    guard request.documentPath == file.url.path else {
      return RequestOutcome.socket(OwnerError.rejected("Document path mismatch")).encoded()
    }
    // A command accepted before its flush failed was applied: its outcome is not a refusal.
    var accepted = false
    do {
      switch request {
      case .hello:
        return SocketReply(ok: true, epoch: epoch, coreBuildId: Self.coreBuildID).encoded()
      case .get:
        try await flush()
        return SocketReply(ok: true, epoch: epoch).encoded(state: #"{"schema":"# + file.descriptor + #","state":"# + (try await state()) + "}")
      case .batch(let r):
        let applied = try await apply(batch: #"{"intents":"# + r.ops + "}", epoch: r.epoch)
        accepted = true
        try await flush()
        return SocketReply(ok: true, epoch: epoch, ids: applied.ids, sequence: applied.sequence).encoded()
      case .compact(let r):
        try await compact(epoch: r.epoch)
        return SocketReply(ok: true, epoch: epoch).encoded()
      case .themeGet:
        return try await theme(.get, epoch: nil, accepted: &accepted)
      case .themeSet(let r):
        let values = String(decoding: try JSONSerialization.data(withJSONObject: r.values), as: UTF8.self)
        return try await theme(.set(valuesJson: values), epoch: r.epoch, accepted: &accepted)
      case .themeReset(let r):
        return try await theme(.reset(token: r.token), epoch: r.epoch, accepted: &accepted)
      case .themeImport(let r):
        return try await theme(.import(fileJson: r.file), epoch: r.epoch, accepted: &accepted)
      case .themeExport:
        let file = String(decoding: try JSONEncoder().encode(["file": try await exportTheme()]), as: UTF8.self)
        return SocketReply(ok: true, epoch: epoch).encoded(state: file)
      case .attachmentsList:
        return SocketReply(ok: true, epoch: epoch).encoded(state: try json(try await listAttachments()))
      case .attachmentsRead(let r):
        return SocketReply(ok: true, epoch: epoch).encoded(state: try json(["bytes": try await readAttachment(r.attachmentID)]))
      case .attachmentsPut(let r):
        return SocketReply(ok: true, epoch: epoch).encoded(state: try json(try await putAttachment(base64: r.bytes, epoch: r.epoch)))
      case .export:
        return RequestOutcome.socket(OwnerError.rejected("Unsupported owner command"), epoch: epoch).encoded()
      }
    } catch {
      // OwnerReplaced says "not applied", which only describes admission: a command already
      // accepted may even be durable when discard rejects its flush.
      let failure = accepted && !(error is SaveFailure) ? failure("Command was accepted, but its final state could not be confirmed.") : error
      return RequestOutcome.socket(failure, epoch: epoch).encoded()
    }
  }
  /// A theme command; like a batch, a change replies once it is durable.
  private func theme(_ change: ThemeChange, epoch: String?, accepted: inout Bool) async throws -> Data {
    let theme = try await applyTheme(change, epoch: epoch)
    accepted = theme.changed
    try await flush()
    return SocketReply(ok: true, epoch: self.epoch).encoded(
      state: #"{"defaults":"# + theme.defaults + #","overrides":"# + theme.overrides + #","effective":"# + theme.effective + "}")
  }
  private func json<T: Encodable>(_ value: T) throws -> String {
    String(decoding: try JSONEncoder().encode(value), as: UTF8.self)
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

enum PageCommand: Sendable { case open, apply(String), text(String), flush, undo, redo }
/// Whether Edit ▸ Undo and Redo have anything to do.
public struct UndoAvailability: Sendable, Equatable {
  public var canUndo = false
  public var canRedo = false
}
