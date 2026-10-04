import Foundation
import HitSlopCore
@testable import HitSlopHost

/// Windows in tests that are not about lifecycle coordination run their commands directly,
/// one at a time, as the app's coordinator would. Coordination is tested in
/// `HitSlopFeaturesTests`.
extension SlopDocumentWindowController {
  static func open(
    url: URL, presentsWindow: Bool = false, telemetry: SlopTelemetry = .disabled
  ) async throws -> SlopDocumentWindowController {
    let commands = DirectCommands()
    let controller = try await open(
      url: url, routing: commands.routing, presentsWindow: presentsWindow, telemetry: telemetry)
    commands.controller = controller
    return controller
  }
}

@MainActor final class DirectCommands {
  weak var controller: SlopDocumentWindowController?
  private var last: Task<Void, Never>?
  var routing: SlopDocumentRouting {
    SlopDocumentRouting(command: { [self] command in
      let previous = last
      last = Task { @MainActor [weak self] in
        await previous?.value
        _ = try? await self?.controller?.perform(command)
      }
    })
  }
}
