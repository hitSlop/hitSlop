import Foundation
import HitSlopCore
import HitSlopCoreBinding

/// Admission failures are distinct from storage failures and uncertain outcomes.
enum OwnerError: LocalizedError, Sendable {
  case closed, closing, invalidated, readOnly, tooLarge
  case rejected(String)

  var errorDescription: String? {
    switch self {
    case .closed: "Document owner is closed"
    case .closing: "Document is closing"
    case .invalidated: "Owner invalidated; explicit recovery is required"
    case .readOnly: "Read-only capture cannot edit"
    case .tooLarge: "Request exceeds size limit"
    case .rejected(let message): message
    }
  }
}

/// What a failed page, socket or page-storage request means for its caller. Every
/// error is classified here once; nothing else inspects error types or messages.
enum RequestOutcome: Equatable {
  /// Refused before applying: `reason` is a core error code, `opIndex` the refused intent.
  case rejected(reason: CoreErrorCode, opIndex: Int?)
  /// Captured for state that was discarded or a view that was replaced: not applied.
  case replaced
  /// The owner is closing or closed: not applied; retry against the next owner.
  case closing
  /// The core refuses every call until saved state is reloaded.
  case invalidated
  /// Applied, but saving failed; the edit stays live and unsaved.
  case saveFailed
  /// Anything else: the caller must read state before relying on the outcome.
  case unknown

  init(_ error: Error) {
    switch error {
    case is OwnerReplaced: self = .replaced
    case let CoreError.Rejected(code, _, opIndex):
      self = .rejected(reason: CoreErrorCode(rawValue: code) ?? .engine_error, opIndex: opIndex.map(Int.init))
    case CoreError.Invalidated: self = .invalidated
    case let error as OwnerError:
      switch error {
      case .closed, .closing: self = .closing
      case .invalidated: self = .invalidated
      case .tooLarge: self = .rejected(reason: .too_large, opIndex: nil)
      case .readOnly, .rejected: self = .rejected(reason: .invalid_request, opIndex: nil)
      }
    case is any SlopRejection: self = .rejected(reason: .invalid_request, opIndex: nil)
    case is SaveFailure: self = .saveFailed
    default: self = .unknown
    }
  }

  var pageCode: PageErrorCode {
    switch self {
    case .rejected: .rejected
    case .replaced: .owner_replaced
    case .closing: .closing
    case .invalidated: .owner_invalidated
    case .saveFailed: .save_failed
    case .unknown: .unknown_outcome
    }
  }
  /// The CLI socket's reply code. Only `failed` leaves the outcome unknown.
  var socketCode: SocketReplyCode {
    switch self {
    case .rejected: .rejected
    case .replaced: .sessionChanged
    case .closing: .closing
    case .invalidated: .unavailable
    case .saveFailed, .unknown: .failed
    }
  }
}
