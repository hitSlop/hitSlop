import Foundation
import HitSlopCore
import HitSlopTestSupport
import Testing
@testable import HitSlopDocument
@testable import HitSlopHost

extension HostTests {
  @Test @MainActor func acquiredExportSnapshotMayFinishAfterEpochReplacementAndClose() async throws {
    let root = try contractFixture()
    let output = root.deletingLastPathComponent().appendingPathComponent(UUID().uuidString + ".pdf")
    defer { try? FileManager.default.removeItem(at: root); try? FileManager.default.removeItem(at: output) }
    let session = try await DocumentSession.open(url: root)
    session.load()
    try await session.waitUntilReady()
    let admittedEpoch = session.epoch
    let deadline = NativeCommandDeadline(expectedEpoch: admittedEpoch)
    try await session.withCaptureSnapshot(expectedEpoch: admittedEpoch) { source in
      try await session.discardPending()
      #expect(session.epoch != admittedEpoch)
      try await session.close()
      _ = try await SlopRenderer.exportClosed(source, format: .pdf, output: output, deadline: deadline)
    }
    #expect(FileManager.default.fileExists(atPath: output.path))
  }

  // A native callback can wait on the main actor after Rust admits its epoch. Discard
  // must fence that queued render before it acquires the replacement owner's snapshot.
  @Test @MainActor func queuedNativeExportCannotCaptureAReplacementEpoch() async throws {
    let root = try contractFixture()
    let output = root.deletingLastPathComponent().appendingPathComponent(UUID().uuidString + ".pdf")
    defer { try? FileManager.default.removeItem(at: root); try? FileManager.default.removeItem(at: output) }
    let session = try await DocumentSession.open(url: root)
    session.load()
    try await session.waitUntilReady()
    var proceed: CheckedContinuation<Void, Never>?
    defer { proceed?.resume() }
    session.onExport = { format, output, deadline in
      await withCheckedContinuation { proceed = $0 }
      try await SlopRenderer.exportDocument(session: session, format: format, output: output, deadline: deadline)
    }
    let request = try JSONSerialization.data(withJSONObject: [
      "method": "export", "documentPath": root.path, "format": "pdf", "output": output.path,
    ])
    let pending = Task { await DocumentCommand.run(json: request) }
    let timeout = ContinuousClock.now.advanced(by: .seconds(5))
    while proceed == nil && ContinuousClock.now < timeout { try await Task.sleep(for: .milliseconds(10)) }
    let release = try #require(proceed, "Native export must reach the callback before discard")
    let oldEpoch = session.epoch
    try await session.discardPending()
    try await session.waitUntilReady()
    #expect(session.epoch != oldEpoch)
    proceed = nil
    release.resume()
    let reply = try decodeReply(await pending.value)
    #expect(reply.code == .ownerReplaced, "\(reply.error ?? "Export unexpectedly succeeded")")
    #expect(!FileManager.default.fileExists(atPath: output.path))
    session.onExport = nil
    try await session.close()
  }
}
