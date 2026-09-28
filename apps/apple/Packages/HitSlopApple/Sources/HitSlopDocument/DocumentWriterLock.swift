import Darwin
import Foundation
import HitSlopCore

/// One permanent lock inode is shared by editing and closed-document snapshots.
public final class DocumentWriterLock {
  public struct Busy: LocalizedError, SlopDiagnosticProviding {
    public var diagnostic: SlopFailureContext { .init(.rejection, reason: .busy) }
    public var errorDescription: String? { "Document has a live writer; retry through its socket" }
  }
  private var fd: Int32 = -1

  public init(root: URL) throws {
    let state = root.appendingPathComponent("state")
    try safeDirectory(state)
    fd = Darwin.open(
      state.appendingPathComponent("writer.lock").path, O_RDWR | O_CREAT | O_NOFOLLOW | O_CLOEXEC, 0o600)
    guard fd >= 0 else { throw failure("Cannot open writer lock") }
    if flock(fd, LOCK_EX | LOCK_NB) != 0 {
      let code = errno
      close()
      if code == EWOULDBLOCK || code == EAGAIN {
        throw Busy()
      }
      throw failure("Cannot acquire writer lock")
    }
  }

  public static func isBusy(_ error: Error) -> Bool { error is Busy }

  public func close() {
    if fd >= 0 {
      Darwin.close(fd)
      fd = -1
    }
  }
  deinit { close() }
}

/// Captures may finish after the socket stops waiting; expired work cannot publish a file.
public struct NativeCommandDeadline: Sendable {
  private let end: ContinuousClock.Instant
  public init(timeout: Duration = .seconds(30)) { end = .now.advanced(by: timeout) }
  public func check() throws {
    guard ContinuousClock.now < end else {
      throw failure("Command timed out; output was not published")
    }
    try Task.checkCancellation()
  }
}
