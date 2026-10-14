import Foundation
import HitSlopCoreBinding

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
  public init(_ failure: OwnerFailure) {
    self =
      switch failure.kind {
      case .full: .full
      case .busy: .busy
      case .moved: .moved
      case .invalidated: .invalidated
      default: .io(failure.message)
      }
  }
  public var errorDescription: String? {
    switch self {
    case .full:
      "Document is full (\(Limits.storageBytes >> 20) MiB limit); saved state is intact. Retry saving or explicitly discard unsaved edits."
    case .busy: "The document is busy in another process; retry saving."
    case .moved: "Document moved or replaced; close before moving a document"
    case .invalidated: "The document engine stopped; reload saved state. Unsaved edits may be lost."
    case .io(let message): message
    }
  }
}
