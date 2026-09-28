import AppKit
import Foundation
import HitSlopCore
import HitSlopRuntime
import PDFKit
import Testing

@testable import HitSlopHost
@testable import HitSlopWasm

extension LoroClientTests {
  @Test @MainActor func jsonImportUpdatesOpenViewAndRejectsPendingDrafts() async throws {
    _ = NSApplication.shared
    let root = try captureFixture()
    let file = root.deletingLastPathComponent().appendingPathComponent(UUID().uuidString + ".json")
    defer { try? FileManager.default.removeItem(at: root); try? FileManager.default.removeItem(at: file) }
    let controller = try await SlopDocumentWindowController.open(packageURL: root)
    try await controller.session.waitUntilReady()
    let view = controller.session.webView
    let before = try await cli(["get", root.path, "--snapshot"])
    #expect(before.0 == 0, "\(before.2)")
    let snapshot = try #require(try JSONSerialization.jsonObject(with: Data(before.1.utf8)) as? [String: Any])
    let version = try #require(snapshot["version"] as? String)
    var data = try #require(snapshot["data"] as? [String: Any])
    data["title"] = "Imported into open window"
    try JSONSerialization.data(withJSONObject: data).write(to: file)
    let applied = try await cli(["import", root.path, "--replace", "--if-version", version, "--file", file.path])
    #expect(applied.0 == 0, "\(applied.2)")
    #expect(try await view.evaluateJavaScript("document.querySelector('#draft').value") as? String == "Imported into open window")
    let accepted = try #require(try JSONSerialization.jsonObject(with: Data(applied.1.utf8)) as? [String: Any])
    let nextVersion = try #require(accepted["version"] as? String)
    _ = try await view.callAsyncJavaScript("""
      const input = document.querySelector('#draft');
      input.dispatchEvent(new CompositionEvent('compositionstart'));
      input.value = 'User is still typing';
      input.dispatchEvent(new InputEvent('input', {bubbles:true, isComposing:true}));
      return true;
      """, arguments: [:], in: nil, contentWorld: .page)
    let rejected = try await cli(["import", root.path, "--replace", "--if-version", nextVersion, "--file", file.path])
    #expect(rejected.0 != 0)
    #expect(rejected.2.contains("Destination changed"))
    #expect(try await view.evaluateJavaScript("document.querySelector('#draft').value") as? String == "User is still typing")
    let fresh = try await DocumentCommand.run(method: "snapshot", url: root)
    let freshState = try #require(try JSONSerialization.jsonObject(with: fresh) as? [String: Any])
    let freshVersion = try #require(freshState["version"] as? String)
    let large = String(repeating: "bulk ", count: 230_000)
    data["title"] = large
    try JSONSerialization.data(withJSONObject: data).write(to: file)
    let bulk = try await cli(["import", root.path, "--replace", "--if-version", freshVersion, "--file", file.path])
    #expect(bulk.0 == 0, "\(bulk.2)")
    // WebKit caps plain input values; rendering that prefix must not edit the document.
    #expect(try await view.evaluateJavaScript("document.querySelector('#draft').value.startsWith('bulk bulk')") as? Bool == true)
    try await controller.session.finish()
    let reopened = try await cli(["get", root.path])
    let state = try #require(try JSONSerialization.jsonObject(with: Data(reopened.1.utf8)) as? [String: Any])
    let preserved = state["title"] as? String == large
    #expect(preserved)
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
    let op = String(decoding: try replace("must not apply"), as: UTF8.self)
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
      "op": try JSONSerialization.jsonObject(with: replace("must not apply")),
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
        "socket": server.path, "epoch": "peer", "pid": ProcessInfo.processInfo.processIdentifier,
        "documentPath": canonical.path,
      ]).write(to: canonical.appendingPathComponent("state/host.lock"))
      let result = try await cli(["apply", root.path, "--op", String(decoding: replace("refused"), as: UTF8.self)])
      #expect(result.0 != 0)
      #expect(result.2.contains("Peer refusal\n" + expected), "\(result.2)")
    }
    #expect(!FileManager.default.fileExists(atPath: canonical.appendingPathComponent("state/document.sqlite").path))
  }

  @Test @MainActor func oldRuntimeFailsBeforeCreatingState() async throws {
    let root = try fixture()
    defer { try? FileManager.default.removeItem(at: root) }
    let url = root.appendingPathComponent("manifest.json")
    var manifest = try JSONSerialization.jsonObject(with: Data(contentsOf: url)) as! [String: Any]
    manifest["runtime"] = "old-runtime"
    try JSONSerialization.data(withJSONObject: manifest).write(to: url)
    await #expect(throws: (any Error).self) {
      try await DocumentCommand.run(method: "get", url: root)
    }
    #expect(!FileManager.default.fileExists(atPath: root.appendingPathComponent("state").path))
  }

  @Test @MainActor func headlessDoesNotLoadAuthoredCode() async throws {
    let root = try fixture()
    defer { try? FileManager.default.removeItem(at: root) }
    try Data(
      "webkit.messageHandlers.storage.postMessage({method:'failed',error:'AUTHORED CODE RAN'});"
        .utf8
    ).write(to: root.appendingPathComponent("assets/app.js"))
    let data = try await DocumentCommand.run(
      method: "apply", url: root, operation: replace("Engine only"))
    #expect(String(decoding: data, as: UTF8.self).contains("Engine only"))
  }
}
