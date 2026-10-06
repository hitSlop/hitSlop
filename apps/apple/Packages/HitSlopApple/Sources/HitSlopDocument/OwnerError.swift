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

/// What a failed page request or native export means for its caller. Every error is classified
/// here once; nothing else inspects error types or messages.
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
    case CoreError.Rejected(let code, _, let opIndex):
      self = .rejected(reason: CoreErrorCode(rawValue: code) ?? .engineError, opIndex: opIndex.map(Int.init))
    case CoreError.Invalidated: self = .invalidated
    case let error as OwnerError:
      switch error {
      case .closed, .closing: self = .closing
      case .readOnly, .rejected: self = .rejected(reason: .invalidRequest, opIndex: nil)
      }
    case is SaveFailure: self = .saveFailed
    default: self = .unknown
    }
  }

  var code: OutcomeCode {
    switch self {
    case .rejected: .rejected
    case .replaced: .ownerReplaced
    case .closing: .closing
    case .invalidated: .ownerInvalidated
    case .saveFailed: .saveFailed
    case .unknown: .unknownOutcome
    }
  }
  private var refusal: (reason: CoreErrorCode?, opIndex: Int?) {
    if case .rejected(let reason, let opIndex) = self { return (reason, opIndex) }
    return (nil, nil)
  }

  /// The page's failure reply for `error`.
  static func page(_ error: Error) -> [String: Any] {
    let outcome = RequestOutcome(error)
    return PageFailure(
      code: outcome.code, error: error.localizedDescription, reason: outcome.refusal.reason,
      opIndex: outcome.refusal.opIndex
    ).json
  }
  /// Native rendering reports the same outcome categories as page requests.
  static func native(_ error: Error) -> OwnerFailure {
    let outcome = RequestOutcome(error)
    let kind: OwnerFailureKind
    switch outcome {
    case .rejected: kind = .rejected
    case .replaced: kind = .replaced
    case .closing: kind = .closing
    case .invalidated: kind = .invalidated
    case .saveFailed: kind = .saveFailed
    case .unknown: kind = .failed
    }
    return OwnerFailure(
      kind: kind, message: error.localizedDescription,
      reason: outcome.refusal.reason?.rawValue, opIndex: outcome.refusal.opIndex.map(UInt32.init))
  }
}
