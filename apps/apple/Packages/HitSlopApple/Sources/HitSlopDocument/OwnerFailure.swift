import Foundation
import HitSlopCore
import HitSlopCoreBinding

extension OwnerFailure {
  static var closed: Self { .init(kind: .closed, message: "Document owner is closed", reason: nil, opIndex: nil) }
  static var closing: Self { .init(kind: .closing, message: "Document is closing", reason: nil, opIndex: nil) }
  static var replaced: Self {
    .init(
      kind: .replaced, message: "The document or page was replaced; the request was not applied", reason: nil,
      opIndex: nil)
  }
  static func rejected(_ message: String) -> Self {
    .init(kind: .rejected, message: message, reason: .invalidRequest, opIndex: nil)
  }

  /// Keep typed core failures intact when a native host operation returns to the owner.
  init(_ error: Error) {
    switch error {
    case let failure as OwnerFailure, CoreError.Failure(let failure): self = failure
    case is SlopRequiresUpdate:
      self.init(kind: .rejected, message: error.localizedDescription, reason: .requiresUpdate, opIndex: nil)
    default: self.init(kind: .failed, message: error.localizedDescription, reason: nil, opIndex: nil)
    }
  }
}

/// The page's reply refusing with `error`, encoded as the core encodes its own.
func pageFailure(_ error: Error) -> String {
  failureReply(failure: OwnerFailure(error))
}
