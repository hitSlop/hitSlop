import Foundation
import HitSlopCore
import HitSlopCoreBinding

/// The only Rust command that needs native UI. Its lifetime extends beyond the editor
/// once it has acquired a saved snapshot; the Rust deadline still gates publication.
final class NativeExports: NativeExportHandler {
  typealias Render = @MainActor @Sendable (URL, ExportFormat, URL, NativeCommandDeadline) async throws -> Void
  typealias Copy = @MainActor @Sendable (URL, NativeCommandDeadline) async throws -> Void
  private let render: Render
  private let copy: Copy?
  init(copy: Copy? = nil, _ render: @escaping Render) { self.render = render; self.copy = copy }
  func export(request: NativeExportRequest, completion: NativeExportCompletion) {
    let deadline = NativeCommandDeadline(active: { completion.isActive() })
    Task { @MainActor in
      do {
        try deadline.check()
        if let format = request.format {
          try await render(
            URL(fileURLWithPath: request.documentPath), format, URL(fileURLWithPath: request.output), deadline)
        } else if let copy {
          try await copy(URL(fileURLWithPath: request.output), deadline)
        } else {
          throw SlopFailure("Document copying is unavailable in this host")
        }
        completion.complete(outcome: .success(output: request.output))
      } catch { completion.complete(outcome: .failure(failure: OwnerFailure(error))) }
    }
  }
}

final class CommandCompletion: NativeCommandCompletion {
  private let reply: @Sendable (Data) -> Void
  init(_ reply: @escaping @Sendable (Data) -> Void) { self.reply = reply }
  func complete(replyJson: String) { reply(Data(replyJson.utf8)) }
}
