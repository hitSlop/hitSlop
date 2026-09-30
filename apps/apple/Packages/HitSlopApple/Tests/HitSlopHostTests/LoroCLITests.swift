import AppKit
import Foundation
import HitSlopCore
import HitSlopRuntime
import PDFKit
import Testing

@testable import HitSlopHost
@testable import HitSlopDocument

extension LoroClientTests {
  @Test @MainActor func jsonImportIsExplicitlyUnsupportedWithoutMutatingStateOrDraft() async throws {
    let root = try captureFixture()
    let file = root.deletingLastPathComponent().appendingPathComponent(UUID().uuidString + ".json")
    defer { try? FileManager.default.removeItem(at: root); try? FileManager.default.removeItem(at: file) }
    let controller = try await SlopDocumentWindowController.open(packageURL: root)
    try await controller.session.waitUntilReady()
    let before = try await DocumentCommand.run(method: "get", url: root)
    let saved = try Data(contentsOf: root.appendingPathComponent("state/document.sqlite"))
    try Data("{}".utf8).write(to: file)
    let view = controller.session.webView
    _ = try await view.callAsyncJavaScript("""
      const input = document.querySelector('#draft');
      input.dispatchEvent(new CompositionEvent('compositionstart'));
      input.value = 'User is still typing';
      input.dispatchEvent(new InputEvent('input', {bubbles:true, isComposing:true}));
      return true;
      """, arguments: [:], in: nil, contentWorld: .page)
    let rejected = try await cli(["apply", root.path, "--op", #"{"type":"set","path":["missing"],"value":true}"#])
    #expect(rejected.0 != 0)
    #expect(try Data(contentsOf: root.appendingPathComponent("state/document.sqlite")) == saved)
    #expect(try await view.evaluateJavaScript("document.querySelector('#draft').value") as? String == "User is still typing")
    // Close commits a composition in progress instead of refusing to close.
    try await controller.session.finish()
    let after = try await DocumentCommand.run(method: "get", url: root)
    #expect(after != before)
    #expect(String(decoding: after, as: UTF8.self).contains("User is still typing"))
  }

  @Test @MainActor func executableExportsLiveSelectionAndClosedDefaultView() async throws {
    _ = NSApplication.shared
    let root = try captureFixture()
    let folder = root.deletingLastPathComponent().appendingPathComponent(UUID().uuidString)
    try FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true)
    defer {
      try? FileManager.default.removeItem(at: root)
      try? FileManager.default.removeItem(at: folder)
    }
    let controller = try await SlopDocumentWindowController.open(packageURL: root)
    try await controller.session.waitUntilReady()
    controller.window?.setContentSize(CGSize(width: 560, height: 620))
    let view = controller.session.webView
    let originalSize = view.frame.size
    _ = try await view.callAsyncJavaScript(
      """
      const input = document.querySelector('#draft');
      input.dispatchEvent(new CompositionEvent('compositionstart'));
      input.value = 'CLI export pending title';
      input.dispatchEvent(new InputEvent('input', {bubbles:true, isComposing:true}));
      input.dispatchEvent(new CompositionEvent('compositionend'));
      globalThis.selectedView = 'Selected view';
      return true;
      """, arguments: [:], in: nil, contentWorld: .page)
    let pdf = folder.appendingPathComponent("live.pdf")
    let png = folder.appendingPathComponent("live.png")
    for (format, output) in [("pdf", pdf), ("png", png)] {
      let result = try await cli(["export", root.path, "--format", format, "--output", output.path])
      #expect(result.0 == 0, "\(result.2)")
      #expect(result.1.trimmingCharacters(in: .whitespacesAndNewlines) == output.path)
    }
    let text = try #require(PDFDocument(data: Data(contentsOf: pdf))?.string)
    #expect(text.contains("Selected view"))
    #expect(text.contains("CLI export pending title"))
    #expect(NSImage(data: try Data(contentsOf: png)) != nil)
    #expect(view.frame.size == originalSize)
    #expect(
      try NSBitmapImageRep(data: Data(contentsOf: png))?.pixelsWide == Int(originalSize.width * 2))
    #expect(
      try await view.evaluateJavaScript(
        "globalThis.selectedView === 'Selected view'")
        as? Bool == true)
    #expect(
      try await view.evaluateJavaScript(
        "[...document.querySelectorAll('[data-slop-capture-target]')].every(e=>e.hidden && e.childElementCount===0)"
      ) as? Bool == true)
    let previous = try Data(contentsOf: pdf)
    _ = try await view.callAsyncJavaScript(
      "globalThis.stopFailure=globalThis.__hitslopCapture.onPrepare(()=>{throw new Error('intentional capture failure')});return true",
      arguments: [:], in: nil, contentWorld: .page)
    let failed = try await cli(["export", root.path, "--format", "pdf", "--output", pdf.path])
    #expect(failed.0 != 0)
    #expect(try Data(contentsOf: pdf) == previous)
    #expect(controller.session.engine.capturing == false)
    _ = try await view.evaluateJavaScript("globalThis.stopFailure()")
    let rejected = try await cli([
      "export", root.path, "--format", "pdf", "--output",
      root.appendingPathComponent("bad.pdf").path,
    ])
    #expect(rejected.0 != 0)
    try await controller.session.finish()
    let saved = try Data(contentsOf: root.appendingPathComponent("state/document.sqlite"))
    let closed = folder.appendingPathComponent("closed.pdf")
    let result = try await cli(["export", root.path, "--format", "pdf", "--output", closed.path])
    #expect(result.0 == 0, "\(result.2)")
    #expect(
      PDFDocument(data: try Data(contentsOf: closed))?.string?.contains("Selected view") == false)
    #expect(try Data(contentsOf: root.appendingPathComponent("state/document.sqlite")) == saved)
  }

  @Test @MainActor func removedRetryFlagsAndUnavailableOwnersFailSafely() async throws {
    _ = NSApplication.shared
    let root = try fixture()
    defer { try? FileManager.default.removeItem(at: root) }
    let op = String(decoding: try setTitle("must not apply"), as: UTF8.self)
    let incomplete = try await cli(["apply", root.path, "--op", op, "--id", "original"])
    #expect(incomplete.0 != 0)
    #expect(incomplete.2.contains("Unknown"))
    #expect(!FileManager.default.fileExists(atPath: root.appendingPathComponent("state").path))
    let controller = try await SlopDocumentWindowController.open(packageURL: root)
    try await controller.session.waitUntilReady()
    let old = try await cli(["apply", root.path, "--op", op, "--id", "original", "--epoch", "old"])
    #expect(old.0 != 0)
    #expect(!old.2.contains("Retry identity:"))
    // Removed CLI flags never reach transport. Send a stale envelope over the actual live socket.
    let documentRoot = controller.session.engine.package.rootURL
    let before = try await DocumentCommand.run(method: "get", url: root)
    let path = try DocumentCommand.liveSocket(for: documentRoot)
    let stale = try JSONSerialization.data(withJSONObject: [
      "id": "stale", "method": "apply", "documentPath": documentRoot.path, "epoch": "old",
      "op": try JSONSerialization.jsonObject(with: setTitle("must not apply")),
    ])
    let response = try await Task.detached { try SocketClient.call(path: path, request: stale) }.value
    let refusal = try #require(try JSONSerialization.jsonObject(with: response) as? [String: Any])
    #expect(refusal["ok"] as? Bool == false)
    #expect(refusal["code"] as? String == "session_changed")
    #expect(try await DocumentCommand.run(method: "get", url: root) == before)
    try await controller.session.finish()
    let ended = try await cli([
      "apply", root.path, "--op", op, "--id", "original", "--epoch", "old",
    ])
    #expect(ended.0 != 0)
    let lock = try DocumentWriterLock(root: root)
    defer { lock.close() }
    let output = root.deletingLastPathComponent().appendingPathComponent(UUID().uuidString + ".pdf")
    defer { try? FileManager.default.removeItem(at: output) }
    let busy = try await cli(["export", root.path, "--format", "pdf", "--output", output.path])
    #expect(busy.0 != 0)
    #expect(busy.2.contains("busy"))
    #expect(!FileManager.default.fileExists(atPath: output.path))
  }

  @Test @MainActor func socketRefusalCodesPreserveCLIRetryGuidance() async throws {
    // A controlled peer at the real transport boundary supplies independently specified wire codes.
    let root = try fixture()
    defer { try? FileManager.default.removeItem(at: root) }
    let canonical = try SlopPackage(rootURL: root).rootURL
    let lock = try DocumentWriterLock(root: canonical)
    defer { lock.close() }
    let cases: [(String?, String)] = [
      ("rejected", "Not applied."), ("unavailable", "Not applied."),
      ("session_changed", "Not applied. Run slop get before issuing another edit."),
      ("closing", "Not applied. Run slop get before issuing another edit."),
      ("failed", "Outcome unknown. Run slop get before issuing another edit."),
      (nil, "Outcome unknown. Run slop get before issuing another edit."),
    ]
    for (code, expected) in cases {
      var envelope: [String: Any] = ["ok": false, "error": "Peer refusal"]
      envelope["code"] = code
      let refusal = envelope
      let server = try SocketServer { request, _, respond in
        if case .hello = request { respond(.init(ok: true, epoch: "peer")); return }
        do { respond(try SocketReply(json: refusal)) }
        catch { respond(.init(ok: false, error: "Invalid peer reply")) }
      }
      defer { server.stop() }
      try JSONSerialization.data(withJSONObject: [
        "socket": server.path, "documentPath": canonical.path,
      ]).write(to: canonical.appendingPathComponent("state/host.lock"))
      let result = try await cli(["apply", root.path, "--op", String(decoding: setTitle("refused"), as: UTF8.self)])
      #expect(result.0 != 0)
      #expect(result.2.contains("Peer refusal\n" + expected), "\(result.2)")
    }
    #expect(!FileManager.default.fileExists(atPath: canonical.appendingPathComponent("state/document.sqlite").path))
  }

  @Test @MainActor func closedOwnerDoesNotLoadAuthoredCode() async throws {
    let root = try fixture()
    defer { try? FileManager.default.removeItem(at: root) }
    try Data(
      "webkit.messageHandlers.hitslop.postMessage({method:'failed',error:'AUTHORED CODE RAN'});"
        .utf8
    ).write(to: root.appendingPathComponent("assets/app.js"))
    let data = try await DocumentCommand.run(
      method: "apply", url: root, operation: setTitle("Engine only"))
    #expect(String(decoding: data, as: UTF8.self).contains("Engine only"))
  }
}
