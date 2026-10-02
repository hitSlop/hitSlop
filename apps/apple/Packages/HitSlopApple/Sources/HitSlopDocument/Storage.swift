import Foundation
import HitSlopCore
import HitSlopCoreBinding

func failure(_ message: String) -> NSError {
  NSError(domain: "hitSlop", code: 1, userInfo: [NSLocalizedDescriptionKey: message])
}

/// `document` owns the package and persists writes. `snapshot` reads the saved document
/// and theme into memory without ownership; renderer writes stay in memory.
public enum StorageMode: Sendable {
  case document, snapshot
  var store: StoreMode { self == .document ? .document : .snapshot }
}

/// Another process owns the document's writer lock.
public struct DocumentLocked: LocalizedError, SlopDiagnosticProviding {
  public var diagnostic: SlopFailureContext { .init(.rejection, reason: .busy) }
  public var errorDescription: String? { "Document has a live writer; retry through its socket" }
}

/// Runs a call into the Rust store, rethrowing its storage failures as host errors.
func storeCall<T>(_ body: () throws -> T) throws -> T {
  do { return try body() } catch let error as CoreError {
    switch error {
    case .Locked: throw DocumentLocked()
    case .Busy: throw SaveFailure.busy
    case .Full: throw SaveFailure.full
    case .Moved: throw SaveFailure.moved
    case .Closed: throw OwnerError.closed
    case .Failed(let message): throw failure(message)
    case .Rejected where SlopRequiresUpdate.matches(error): throw SlopRequiresUpdate()
    case .Rejected, .Invalidated: throw error
    }
  }
}

extension WriterLock {
  /// The package's writer lock alone; `DocumentLocked` while another process owns it.
  public static func acquire(_ root: URL) throws -> WriterLock {
    try storeCall { try acquire(root: root.path) }
  }
}

#if DEBUG
  /// Fault injection at the storage I/O boundary; see `StorePhases`.
  final class PhaseHook: StorePhases, @unchecked Sendable {
    private let body: (String) throws -> Void
    init(_ body: @escaping (String) throws -> Void) { self.body = body }
    func reached(phase: String) throws {
      do { try body(phase) } catch let error as CoreError { throw error } catch {
        throw CoreError.Failed(message: error.localizedDescription)
      }
    }
  }
#endif

/// Why a save did not commit. Every case keeps ownership, the live state and all edits.
public enum SaveFailure: Error, LocalizedError, Equatable {
  /// The write would exceed the storage limits; saved state is intact.
  case full
  /// Another process held the database (for example a backup); retrying can succeed.
  case busy
  /// The package directory was moved or replaced while open.
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
