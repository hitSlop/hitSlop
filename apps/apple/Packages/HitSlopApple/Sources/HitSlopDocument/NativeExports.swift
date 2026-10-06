import Foundation
import HitSlopCore
import HitSlopCoreBinding

/// The only Rust command that needs native UI. Its lifetime extends beyond the editor
/// once it has acquired a saved snapshot; the Rust deadline still gates publication.
final class NativeExports: NativeExportHandler {
  typealias Render = @MainActor @Sendable (URL, ExportFormat, URL, NativeCommandDeadline) async throws -> Void
  private let render: Render
  init(_ render: @escaping Render) { self.render = render }
  func export(request: NativeExportRequest, completion: NativeExportCompletion) {
    let deadline = NativeCommandDeadline(active: { completion.isActive() })
    Task { @MainActor in
      do {
        try deadline.check()
        guard let format = ExportFormat(rawValue: request.format) else {
          throw OwnerError.rejected("Invalid export format")
        }
        try await render(
          URL(fileURLWithPath: request.documentPath), format, URL(fileURLWithPath: request.output), deadline)
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
