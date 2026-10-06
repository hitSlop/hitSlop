import HitSlopCore

/// One document's commands: one runs at a time. A close requested meanwhile runs after it,
/// and so does an answer to the save-failure sheet (before the close); anything else
/// requested meanwhile is dropped.
struct CommandQueue: Equatable, Sendable {
  private(set) var running: SlopDocumentCommand?
  private(set) var closeRequested = false
  /// A save recovery chosen while another command ran.
  private(set) var pendingRecovery: SlopDocumentCommand?

  /// Nothing runs and no close waits.
  var isIdle: Bool { running == nil && !closeRequested }

  /// Admits `command`: returns it when it runs now, nil when it waits or is dropped.
  mutating func admit(_ command: SlopDocumentCommand) -> SlopDocumentCommand? {
    guard running != nil else {
      running = command
      return command
    }
    if command == .close { closeRequested = true }
    if command.isSaveRecovery { pendingRecovery = command }
    return nil
  }

  /// The running command ended; returns the next one to run. A closed document has nothing
  /// left to run, and a failure drops the close that waited for it.
  mutating func finish(failed: Bool) -> SlopDocumentCommand? {
    let finished = running
    running = nil
    if finished == .close && !failed {
      pendingRecovery = nil
      return nil
    }
    if failed { closeRequested = false }
    if let recovery = pendingRecovery {
      pendingRecovery = nil
      running = recovery
    } else if closeRequested {
      closeRequested = false
      running = .close
    }
    return running
  }
}
