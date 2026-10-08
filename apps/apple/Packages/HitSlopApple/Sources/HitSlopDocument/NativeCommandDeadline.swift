import Foundation
import HitSlopCore

/// Captures may finish after the socket stops waiting; expired work cannot publish a file.
public struct NativeCommandDeadline: Sendable {
  private let end: ContinuousClock.Instant
  private let active: @Sendable () -> Bool
  public init(timeout: Duration = Timeouts.command, active: @escaping @Sendable () -> Bool = { true }) {
    end = .now.advanced(by: timeout)
    self.active = active
  }
  public func check() throws {
    guard ContinuousClock.now < end, active() else {
      throw SlopFailure("Command timed out; output was not published")
    }
    try Task.checkCancellation()
  }
}
