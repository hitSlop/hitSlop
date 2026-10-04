import AppKit
import Foundation
import HitSlopCore
import Testing
import WebKit
import HitSlopTestSupport
@testable import HitSlopDocument

@Suite(.serialized) struct DocumentFileSaverTests {
  let offer = """
    { const a = document.createElement('a');
    a.href = URL.createObjectURL(new Blob([new Uint8Array([104,105,33])], { type: 'application/octet-stream' }));
    a.download = '../Totals: March.xlsx';
    document.body.append(a); a.click(); a.remove(); }
    """

  @Test @MainActor func blobDownloadSavesOnlyAfterConfirmationAndHonorsGuards() async throws {
    _ = NSApplication.shared
    let root = try Fixtures.native()
    let folder = try Fixtures.folder()
    defer { try? FileManager.default.removeItem(at: root); try? FileManager.default.removeItem(at: folder) }
    let engine = try await DocumentSession.open(url: root)
    let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 400, height: 500), styleMask: [.borderless], backing: .buffered, defer: false)
    window.contentView = engine.webView
    window.orderFront(nil)
    defer { window.orderOut(nil); window.contentView = nil }
    engine.load()
    try await engine.waitUntilReady()

    var names: [String] = []
    var reply: ((URL?) -> Void)?
    var saved: [URL] = []
    engine.fileSaver = DocumentFileSaver { parent, name, completion in
      #expect(parent === window)
      names.append(name)
      reply = completion
      return { completion(nil) }
    }
    engine.fileSaver.onSaved = { saved.append($0) }
    func offerAndWait() async throws {
      reply = nil
      _ = try await engine.webView.evaluateJavaScript(offer)
      for _ in 0..<100 where !engine.fileSaver.hasPendingSave { try await Task.sleep(for: .milliseconds(20)) }
    }

    // Cancelling the panel writes nothing.
    try await offerAndWait()
    #expect(engine.fileSaver.hasPendingSave)
    #expect(names == ["_Totals_ March.xlsx"])  // WebKit sanitizes first; native cleanup is defense in depth.
    let cancelledReply = reply
    reply?(nil)
    cancelledReply?(folder.appendingPathComponent("late.xlsx"))
    #expect(!engine.fileSaver.hasActiveTransfers)
    #expect(!engine.fileSaver.hasPendingSave)
    #expect(try FileManager.default.contentsOfDirectory(atPath: folder.path).isEmpty)

    // Confirming writes the bytes, replacing an existing file only once complete.
    let target = folder.appendingPathComponent("Totals.xlsx")
    try Data("old".utf8).write(to: target)
    try await offerAndWait()
    reply?(target)
    for _ in 0..<100 where saved.isEmpty { try await Task.sleep(for: .milliseconds(20)) }
    #expect(saved == [target])
    #expect(try Data(contentsOf: target) == Data("hi!".utf8))

    // Cancelling an accepted transfer must not install its bytes or report failure.
    var failures: [String] = []
    let events = SessionEvents()
    events.issue = { failures.append($0.message) }
    engine.delegate = events
    try await offerAndWait()
    reply?(folder.appendingPathComponent("cancelled.xlsx"))
    engine.fileSaver.cancel()
    #expect(!engine.fileSaver.hasActiveTransfers)
    try await Task.sleep(for: .milliseconds(300))
    #expect(!FileManager.default.fileExists(atPath: folder.appendingPathComponent("cancelled.xlsx").path))
    #expect(saved == [target])
    #expect(failures.isEmpty)

    // Filesystem failures reach the native issue handler and leave the old file intact.
    try await offerAndWait()
    reply?(target.appendingPathComponent("impossible.xlsx"))
    for _ in 0..<100 where failures.isEmpty { try await Task.sleep(for: .milliseconds(20)) }
    #expect(failures.count == 1)
    #expect(!engine.fileSaver.hasActiveTransfers)
    #expect(try Data(contentsOf: target) == Data("hi!".utf8))

    // Capture and disabled selection never present a panel.
    engine.capturing = true
    _ = try await engine.webView.evaluateJavaScript(offer)
    try await Task.sleep(for: .milliseconds(300))
    #expect(!engine.fileSaver.hasPendingSave)
    engine.capturing = false
    engine.allowsFileSelection = false
    _ = try await engine.webView.evaluateJavaScript(offer)
    try await Task.sleep(for: .milliseconds(300))
    #expect(!engine.fileSaver.hasPendingSave)
    engine.allowsFileSelection = true

    // Closing dismisses a pending panel.
    try await offerAndWait()
    #expect(engine.fileSaver.hasPendingSave)
    try await engine.close()
    #expect(!engine.fileSaver.hasPendingSave)
    #expect(names.count == 5)
    reply?(target)
    #expect(!engine.fileSaver.hasActiveTransfers)
    #expect(try Data(contentsOf: target) == Data("hi!".utf8))
  }

  @Test @MainActor func installationPreservesDestinationOnFailureAndSizeRejection() throws {
    let manager = FileManager.default
    let folder = try Fixtures.folder()
    defer { try? manager.removeItem(at: folder) }
    let staging = folder.appendingPathComponent("staging")
    let target = folder.appendingPathComponent("existing")
    try Data("old".utf8).write(to: target)
    try Data("new contents".utf8).write(to: staging)
    #expect(throws: (any Error).self) { try DocumentFileSaver.install(staging, at: target, limit: 2) }
    #expect(try Data(contentsOf: target) == Data("old".utf8))
    #expect(throws: (any Error).self) {
      try DocumentFileSaver.install(staging, at: target.appendingPathComponent("impossible"))
    }
    #expect(try Data(contentsOf: target) == Data("old".utf8))
    // A quarantine write failure must happen before any destination is replaced.
    for destination in [target, folder.appendingPathComponent("unpublished")] {
      #expect(throws: CocoaError.self) {
        try DocumentFileSaver.install(staging, at: destination, quarantine: { _ in
          throw CocoaError(.fileWriteNoPermission)
        })
      }
    }
    #expect(try Data(contentsOf: target) == Data("old".utf8))
    #expect(!manager.fileExists(atPath: folder.appendingPathComponent("unpublished").path))
    try DocumentFileSaver.install(staging, at: target)
    #expect(try Data(contentsOf: target) == Data("new contents".utf8))
    #expect(try target.resourceValues(forKeys: [.quarantinePropertiesKey]).quarantineProperties != nil)
    let fresh = folder.appendingPathComponent("fresh")
    try DocumentFileSaver.install(staging, at: fresh)
    #expect(try Data(contentsOf: fresh) == Data("new contents".utf8))
    #expect(try fresh.resourceValues(forKeys: [.quarantinePropertiesKey]).quarantineProperties != nil)
  }

  @Test func suggestedNamesStayPlainFileNames() {
    #expect(DocumentFileSaver.filename("../../etc/passwd") == "passwd")
    #expect(DocumentFileSaver.filename("a:b\u{0}c.csv") == "a-b-c.csv")
    #expect(DocumentFileSaver.filename("...") == "Download")
    #expect(DocumentFileSaver.filename("") == "Download")
  }
}
