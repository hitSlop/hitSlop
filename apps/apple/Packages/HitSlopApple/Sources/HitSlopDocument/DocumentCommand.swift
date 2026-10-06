import Foundation
import HitSlopCore
import HitSlopCoreBinding

/// The helper forwards to the same Rust routing used by the CLI engine. Swift supplies
/// only the renderer for closed exports; data commands never start WebKit.
@MainActor public enum DocumentCommand {
  public typealias Export = @MainActor @Sendable (URL, ExportFormat, URL, NativeCommandDeadline) async throws -> Void
  /// One request written in command `protocol`.
  public static func run(json: Data, protocol version: Int = HelperProtocol.version, export: Export? = nil) async
    -> Data
  {
    _ = SlopRegistry.prepared
    return await withCheckedContinuation { done in
      commandRequest(
        json: String(decoding: json, as: UTF8.self), protocol: UInt64(version),
        exporter: export.map(NativeExports.init),
        completion: CommandCompletion { done.resume(returning: $0) })
    }
  }
}
