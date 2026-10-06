import HitSlopCore
import Testing

@testable import HitSlopFeatures

@Test func aCommandRunsAloneAndAnythingButACloseOrRecoveryIsDropped() {
  var queue = CommandQueue()
  #expect(queue.admit(.exportPNG) == .exportPNG)
  #expect(queue.admit(.duplicate) == nil)
  #expect(queue.admit(.exportPNG) == nil)
  #expect(queue.finish(failed: false) == nil)
  #expect(queue.isIdle)
}

@Test func aCloseRequestedTwiceRunsOnceAfterTheCommand() {
  var queue = CommandQueue()
  _ = queue.admit(.exportPNG)
  #expect(queue.admit(.close) == nil)
  #expect(queue.admit(.close) == nil)
  #expect(!queue.isIdle)
  #expect(queue.finish(failed: false) == .close)
  #expect(queue.finish(failed: false) == nil)
  #expect(queue.isIdle)
}

@Test func aRecoveryRunsBeforeTheQueuedClose() {
  var queue = CommandQueue()
  _ = queue.admit(.exportPNG)
  _ = queue.admit(.close)
  _ = queue.admit(.retrySave)
  #expect(queue.admit(.discardUnsaved) == nil, "the latest answer to the sheet wins")
  #expect(queue.finish(failed: false) == .discardUnsaved)
  #expect(queue.finish(failed: false) == .close)
}

@Test func aFailureDropsTheQueuedCloseButNotTheRecovery() {
  var queue = CommandQueue()
  _ = queue.admit(.exportPNG)
  _ = queue.admit(.close)
  #expect(queue.finish(failed: true) == nil)
  #expect(queue.isIdle)
  _ = queue.admit(.exportPNG)
  _ = queue.admit(.close)
  _ = queue.admit(.retrySave)
  #expect(queue.finish(failed: true) == .retrySave)
  #expect(queue.finish(failed: false) == nil, "the close waited for the failed command and is dropped")
}

@Test func aClosedDocumentRunsNothingMore() {
  var queue = CommandQueue()
  _ = queue.admit(.close)
  _ = queue.admit(.retrySave)
  #expect(queue.finish(failed: false) == nil)
  #expect(queue.pendingRecovery == nil)
}
