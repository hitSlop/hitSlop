import Foundation
import HitSlopCore

/// Captures may finish after the socket stops waiting; expired work cannot publish a file.
public struct NativeCommandDeadline: Sendable {
  /// The live owner admitted by the native command. Checked only while acquiring a
  /// saved capture; an acquired snapshot may finish after that owner is replaced.
  public let expectedEpoch: String?
  private let end: ContinuousClock.Instant
  private let active: @Sendable () -> Bool
  public init(timeout: Duration = Timeouts.command, active: @escaping @Sendable () -> Bool = { true }, expectedEpoch: String? = nil) {
    end = .now.advanced(by: timeout)
    self.active = active
    self.expectedEpoch = expectedEpoch
  }
  public func check() throws {
    guard ContinuousClock.now < end, active() else {
      throw SlopFailure("Command timed out; output was not published")
    }
    try Task.checkCancellation()
  }
}
