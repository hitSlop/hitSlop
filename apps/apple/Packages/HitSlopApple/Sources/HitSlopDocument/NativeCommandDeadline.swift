import Foundation
import HitSlopCore

/// Captures may finish after the socket stops waiting; expired work cannot publish a file.
public struct NativeCommandDeadline: Sendable {
  private let end: ContinuousClock.Instant
  public init(timeout: Duration = Timeouts.command) { end = .now.advanced(by: timeout) }
  public func check() throws {
    guard ContinuousClock.now < end else {
      throw SlopFailure("Command timed out; output was not published")
    }
    try Task.checkCancellation()
  }
}
