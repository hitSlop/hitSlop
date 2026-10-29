import Foundation
import TinysCore
import TinysCoreBinding

/// The writer-lock registry the core keeps (`~/.tinys/live`).
public enum TinyRegistry {
  private static let configuration: Result<Void, Error> = Result {
    #if os(iOS)
      let folder = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]
        .appendingPathComponent("WriterRegistry", isDirectory: true)
      try FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true)
      try useRegistryFolder(path: folder.path)
    #else
      #if DEBUG
        if let folder = ProcessInfo.processInfo.environment["TINYS_TEST_REGISTRY"], !folder.isEmpty {
          try FileManager.default.createDirectory(atPath: folder, withIntermediateDirectories: true)
          try useRegistryFolder(path: folder)
        }
      #endif
    #endif
  }
  /// Debug builds use `TINYS_TEST_REGISTRY` when it is set, so test runs never fill a
  /// person's registry. Release builds never read it, so the app and the CLI always
  /// share one registry.
  public static let prepared: Void = {
    _ = configuration
  }()
  static func prepare() throws { try configuration.get() }
  /// Removes discovery files a crashed owner left behind. Run at launch.
  public static func sweep() {
    guard (try? prepare()) != nil else { return }
    _ = try? sweepRegistry()
  }
}

/// Unwraps binding errors while preserving the core's typed failure information.
func storeCall<T>(_ body: () throws -> T) throws -> T {
  try TinyRegistry.prepare()
  do { return try body() } catch {
    switch error {
    case CoreError.Failure(let failure), let failure as OwnerFailure: throw failure
    default: throw error
    }
  }
}
