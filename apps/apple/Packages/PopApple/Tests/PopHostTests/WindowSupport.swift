import Foundation
import PopCore
import PopTestSupport

@testable import PopHost

/// Windows in tests that are not about lifecycle coordination run their commands directly,
/// one at a time, as the app's coordinator would. Coordination is tested in
/// `PopFeaturesTests`.
extension PopDocumentWindowController {
  static func open(
    url: URL, presentsWindow: Bool = false, telemetry: PopTelemetry = .disabled
  ) async throws -> PopDocumentWindowController {
    let commands = DirectCommands()
    let controller = try await open(
      url: url, routing: commands.routing, progress: presentsWindow ? PopOpeningProgress() : nil, telemetry: telemetry)
    commands.controller = controller
    controller.window?.moveOffScreen()
    return controller
  }
}

@MainActor final class DirectCommands {
  weak var controller: PopDocumentWindowController?
  private var last: Task<Void, Never>?
  var routing: PopDocumentRouting {
    PopDocumentRouting(command: { [self] command in
      let previous = last
      last = Task { @MainActor [weak self] in
        await previous?.value
        _ = try? await self?.controller?.perform(command)
      }
    })
  }
}
