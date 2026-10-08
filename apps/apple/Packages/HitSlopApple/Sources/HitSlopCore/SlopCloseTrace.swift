import os

/// Local Instruments timings only. One ephemeral signpost ID links a close's phases;
/// paths, titles, document content and uploaded telemetry never enter this trace.
@MainActor public final class SlopCloseTrace {
  private static let signposter = OSSignposter(subsystem: "com.hitslop", category: "DocumentClose")
  private let id: OSSignpostID
  private var total: OSSignpostIntervalState?

  public init() {
    id = Self.signposter.makeSignpostID()
    total = Self.signposter.beginInterval("Close", id: id)
  }

  public func begin(_ phase: StaticString) -> OSSignpostIntervalState {
    Self.signposter.beginInterval(phase, id: id)
  }

  public func end(_ phase: StaticString, _ interval: OSSignpostIntervalState?) {
    if let interval { Self.signposter.endInterval(phase, interval) }
  }

  public func finish() {
    end("Close", total)
    total = nil
  }

  public static func browserHandoff() {
    signposter.emitEvent("Browser handoff")
  }
}
