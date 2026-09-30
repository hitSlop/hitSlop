import Foundation

/// An issue the page reported while it keeps running: an authored error, or a document
/// operation the owner refused.
public struct SlopPageIssue: Sendable, Equatable {
  public let message: String
  /// A refused document operation, rather than an authored or rendering failure.
  public let isOperation: Bool
  public init(message: String, isOperation: Bool) { self.message = message; self.isOperation = isOperation }
}
