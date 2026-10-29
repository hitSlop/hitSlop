import Foundation
import TinysCore
import TinysTestSupport

@testable import TinysHost

/// Windows in tests that are not about lifecycle coordination run their commands directly,
/// one at a time, as the app's coordinator would. Coordination is tested in
/// `TinysFeaturesTests`.
extension TinyDocumentWindowController {
  static func open(
    url: URL, presentsWindow: Bool = false, telemetry: TinyTelemetry = .disabled
  ) async throws -> TinyDocumentWindowController {
    let commands = DirectCommands()
    let controller = try await open(
      url: url, routing: commands.routing, progress: presentsWindow ? TinyOpeningProgress() : nil, telemetry: telemetry)
    commands.controller = controller
    controller.window?.moveOffScreen()
    return controller
  }
}

@MainActor final class DirectCommands {
  weak var controller: TinyDocumentWindowController?
  private var last: Task<Void, Never>?
  var routing: TinyDocumentRouting {
    TinyDocumentRouting(command: { [self] command in
      let previous = last
      last = Task { @MainActor [weak self] in
        await previous?.value
        _ = try? await self?.controller?.perform(command)
      }
    })
  }
}
