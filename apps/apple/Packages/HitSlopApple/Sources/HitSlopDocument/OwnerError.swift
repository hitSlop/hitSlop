import Foundation
import HitSlopCore
import HitSlopCoreBinding

/// Admission failures are distinct from storage failures and uncertain outcomes.
enum OwnerError: LocalizedError, Sendable {
  case closed, closing, readOnly
  case rejected(String)

  var errorDescription: String? {
    switch self {
    case .closed: "Document owner is closed"
    case .closing: "Document is closing"
    case .readOnly: "Read-only capture cannot edit"
    case .rejected(let message): message
    }
  }
}

extension OwnerFailure {
  /// The core's failure in the host's terms: the one place a failure becomes a host error.
  /// A refusal, and an owner that must reload, stay the core's failure.
  var hostError: Error {
    switch kind {
    case .replaced: OwnerReplaced()
    case .closing: OwnerError.closing
    case .closed: OwnerError.closed
    case .readOnly: OwnerError.readOnly
    case .locked: DocumentLocked()
    case .busy: SaveFailure.busy
    case .full: SaveFailure.full
    case .moved: SaveFailure.moved
    case .saveFailed: SaveFailure.io(message)
    case .failed: SlopFailure(message)
    case .rejected where refusal == .requiresUpdate: SlopRequiresUpdate()
    case .rejected, .invalidated: self
    }
  }

  /// A failed save as the window shows it; an invalidated owner reloads instead of retrying.
  var saveFailure: SaveFailure { kind == .invalidated ? .invalidated : SaveFailure(hostError) }

  /// `error`, raised in the host, as the core classifies failures: what a native export
  /// reports, and what a window reply refuses with (`pageFailure`). The inverse of
  /// `hostError`.
  init(_ error: Error) {
    let message = error.localizedDescription
    switch error {
    case let failure as OwnerFailure: self = failure
    case is OwnerReplaced: self.init(kind: .replaced, message: message, reason: nil, opIndex: nil)
    case is DocumentLocked: self.init(kind: .locked, message: message, reason: nil, opIndex: nil)
    case is SlopRequiresUpdate:
      self.init(kind: .rejected, message: message, reason: CoreErrorCode.requiresUpdate.rawValue, opIndex: nil)
    case let error as OwnerError:
      switch error {
      case .closed, .closing: self.init(kind: .closing, message: message, reason: nil, opIndex: nil)
      case .readOnly, .rejected:
        self.init(kind: .rejected, message: message, reason: CoreErrorCode.invalidRequest.rawValue, opIndex: nil)
      }
    case is SaveFailure: self.init(kind: .saveFailed, message: message, reason: nil, opIndex: nil)
    default: self.init(kind: .failed, message: message, reason: nil, opIndex: nil)
    }
  }
}

/// The page's reply refusing with `error`, encoded as the core encodes its own.
func pageFailure(_ error: Error) -> String {
  failureReply(failure: OwnerFailure(error))
}
