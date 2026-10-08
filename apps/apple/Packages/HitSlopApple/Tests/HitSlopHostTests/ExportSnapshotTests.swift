import AppKit
import Foundation
import HitSlopCore
import HitSlopTestSupport
import Testing

@testable import HitSlopDocument
@testable import HitSlopHost

extension HostTests {
  @Test(arguments: [false, true]) @MainActor func copyKeepsTheStateUsedForItsArtwork(closeDuringRender: Bool)
    async throws
  {
    let root = try contractFixture()
    let destination = root.deletingLastPathComponent().appendingPathComponent(UUID().uuidString + ".slop")
    defer {
      try? FileManager.default.removeItem(at: root)
      try? FileManager.default.removeItem(at: destination)
    }
    let session = try await DocumentSession.open(url: root)
    session.load()
    try await session.waitUntilReady()
    #expect(try await command("batch", url: root, setTitle("Before rendering")).ok)
    let png = try Fixtures.png(width: 32, height: 32) {
      NSColor.red.setFill()
      $0.fill()
    }
    var capturedTitle: String?
    var renderError: Error?
    try await session.copy(to: destination) { source in
      do {
        capturedTitle = try await savedValue(source)?["title"] as? String
        #expect(try await command("batch", url: root, setTitle("After rendering began")).ok)
        if closeDuringRender { try await session.close() }
      } catch { renderError = error }
      return SlopRenderedArtwork(preview: png, icon: nil)
    }
    #expect(renderError == nil)
    #expect(capturedTitle == "Before rendering")
    #expect(try await savedValue(destination)?["title"] as? String == capturedTitle)
    #expect(Fixtures.pixels(SlopArtwork.png(destination, .preview)) == Fixtures.pixels(png))
    #expect(try await savedValue(root)?["title"] as? String == "After rendering began")
    try await session.close()
  }

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
      try await SlopRenderer.exportClosed(source, format: .pdf, output: output)
    }
    #expect(FileManager.default.fileExists(atPath: output.path))
  }
}
