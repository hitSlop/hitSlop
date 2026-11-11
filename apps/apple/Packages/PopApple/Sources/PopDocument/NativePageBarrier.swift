import Foundation
import PopCore
import PopCoreBinding

/// Drains page drafts before Rust saves and acquires its independent capture source.
final class PageBarrier: NativePageBarrier, @unchecked Sendable {
  private let queueLock = NSLock()
  private var tail: Task<Void, Never>?
  private let settlePage: @MainActor @Sendable (NativePageSettlement) async -> Void
  private let resumePage: @MainActor @Sendable () async -> Void
  init(
    settle: @escaping @MainActor @Sendable (NativePageSettlement) async -> Void,
    resume: @escaping @MainActor @Sendable () async -> Void
  ) {
    settlePage = settle
    resumePage = resume
  }
  private func enqueue(_ work: @escaping @MainActor @Sendable () async -> Void) {
    queueLock.withLock {
      let previous = tail
      tail = Task { @MainActor in
        await previous?.value
        await work()
      }
    }
  }
  func settle(completion: NativePageSettlement) {
    enqueue { [settlePage] in await settlePage(completion) }
  }
  func resume() {
    enqueue { [resumePage] in await resumePage() }
  }
}
