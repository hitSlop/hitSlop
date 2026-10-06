import Foundation
import HitSlopCore
import HitSlopCoreBinding

/// Another process owns the document's writer lock.
public struct DocumentLocked: LocalizedError, SlopDiagnosticProviding {
  public var diagnostic: SlopFailureContext { .init(.rejection, reason: .busy) }
  public var errorDescription: String? { "Document has a live writer; retry through its socket" }
}

/// The writer-lock registry the core keeps (`~/.hitslop/live`).
public enum SlopRegistry {
  /// Debug builds use `HITSLOP_TEST_REGISTRY` when it is set, so test runs never fill a
  /// person's registry. Release builds never read it, so the app and its helper always
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

/// Runs a call into the Rust store, rethrowing its storage failures as host errors.
func storeCall<T>(_ body: () throws -> T) throws -> T {
  _ = SlopRegistry.prepared
  do { return try body() } catch let error as CoreError {
    switch error {
    case .Locked: throw DocumentLocked()
    case .Busy: throw SaveFailure.busy
    case .Full: throw SaveFailure.full
    case .Moved: throw SaveFailure.moved
    case .Closed: throw OwnerError.closed
    case .Failed(let message): throw SlopFailure(message)
    case .Rejected where SlopRequiresUpdate.matches(error): throw SlopRequiresUpdate()
    case .Rejected, .Invalidated: throw error
    }
  }
}

/// Why a save did not commit. Every case keeps ownership, the live state and all edits.
public enum SaveFailure: Error, LocalizedError, Equatable {
  /// The write would exceed the storage limits; saved state is intact.
  case full
  /// Another process held the database (for example a backup); retrying can succeed.
  case busy
  /// The document file was moved or replaced while open.
  case moved
  /// The core refused every call; only discarding unsaved edits and reloading recovers.
  case invalidated
  case io(String)
  init(_ error: Error) { self = error as? SaveFailure ?? .io(error.localizedDescription) }
  public var errorDescription: String? {
    switch self {
    case .full: "Document is full (\(Limits.storageBytes >> 20) MiB limit); saved state is intact. Retry saving or explicitly discard unsaved edits."
    case .busy: "The document is busy in another process; retry saving."
    case .moved: "Document moved or replaced; close before moving a document"
    case .invalidated: "The document engine stopped; reload saved state. Unsaved edits may be lost."
    case .io(let message): message
    }
  }
}
