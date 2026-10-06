import Foundation
import HitSlopCore
import HitSlopTestSupport
import Testing

@testable import HitSlopDocument
@testable import HitSlopHost

extension HostTests {
  // An acquired export snapshot is independent of its window: it renders after the
  // window's owner discards and closes.
  @Test @MainActor func acquiredExportSnapshotMayFinishAfterDiscardAndClose() async throws {
    let root = try contractFixture()
    let output = root.deletingLastPathComponent().appendingPathComponent(UUID().uuidString + ".pdf")
    defer {
      try? FileManager.default.removeItem(at: root)
      try? FileManager.default.removeItem(at: output)
    }
    let session = try await DocumentSession.open(url: root)
    session.load()
    try await session.waitUntilReady()
    try await session.withCaptureSnapshot { source in
      try await session.discardPending()
      try await session.close()
      _ = try await SlopRenderer.exportClosed(source, format: .pdf, output: output)
    }
    #expect(FileManager.default.fileExists(atPath: output.path))
  }
}
