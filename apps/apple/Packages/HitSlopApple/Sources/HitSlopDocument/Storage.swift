import Foundation
import HitSlopCore
import HitSlopCoreBinding

/// The writer-lock registry the core keeps (`~/.hitslop/live`).
public enum SlopRegistry {
  /// Debug builds use `HITSLOP_TEST_REGISTRY` when it is set, so test runs never fill a
  /// person's registry. Release builds never read it, so the app and the CLI always
  /// share one registry.
  public static let prepared: Void = {
    #if DEBUG
      if let folder = ProcessInfo.processInfo.environment["HITSLOP_TEST_REGISTRY"], !folder.isEmpty {
        try? FileManager.default.createDirectory(atPath: folder, withIntermediateDirectories: true)
        try? useRegistryFolder(path: folder)
      }
    #endif
  }()
  /// Removes discovery files a crashed owner left behind. Run at launch.
  public static func sweep() {
    _ = prepared
    _ = try? sweepRegistry()
  }
}

/// Unwraps binding errors while preserving the core's typed failure information.
func storeCall<T>(_ body: () throws -> T) throws -> T {
  _ = SlopRegistry.prepared
  do { return try body() } catch {
    switch error {
    case CoreError.Failure(let failure), let failure as OwnerFailure: throw failure
    default: throw error
    }
  }
}
