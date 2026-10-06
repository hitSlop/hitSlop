import AppKit
import Foundation
import HitSlopCore
import HitSlopCoreBinding
import PDFKit
import Testing
import HitSlopTestSupport

@testable import HitSlopHost
@testable import HitSlopDocument

extension OwnerClientTests {
  @Test(arguments: [false, true]) @MainActor func executableReadModesAndSingleEdits(live: Bool) async throws {
    _ = NSApplication.shared
    let root = try fixture()
    defer { try? FileManager.default.removeItem(at: root) }
    let controller = live ? try await SlopDocumentWindowController.open(packageURL: root) : nil
    try await controller?.session.waitUntilReady()
    do {
      func run(_ args: [String]) async throws -> [String: Any] {
        let result = try await cli(args)
        try #require(result.0 == 0, "\(args): \(result.2)")
        return try #require(JSONSerialization.jsonObject(with: Data(result.1.utf8)) as? [String: Any])
      }
      let single = try await run(["apply", root.path, "--op", #"{"type":"set","path":["title"],"value":"Single edit"}"#])
      #expect((single["value"] as? [String: Any])?["title"] as? String == "Single edit")
      #expect(single["ids"] as? [String] == [])
      #expect(single["sequence"] as? Int != nil)
      let batch = try await run(["batch", root.path, "--ops", #"[{"type":"set","path":["title"],"value":"Batch edit"}]"#])
      #expect((batch["value"] as? [String: Any])?["title"] as? String == "Batch edit")
      let value = try await run(["get", root.path])
      let snapshot = try await run(["get", root.path, "--snapshot"])
      let state = try #require(snapshot["state"] as? [String: Any])
      #expect(NSDictionary(dictionary: try #require(state["value"] as? [String: Any])) == NSDictionary(dictionary: value))
      #expect(state["version"] is String)
      #expect(state["sequence"] is Int)
      #expect(state["issues"] is [Any])
      let schema = try JSONSerialization.jsonObject(with: Data(contentsOf: root.appendingPathComponent("state.schema.json"))) as! [String: Any]
      #expect(NSDictionary(dictionary: try #require(snapshot["schema"] as? [String: Any])) == NSDictionary(dictionary: schema))
      let injected = try await cli(["apply", root.path, "--op", #"{"type":"set","path":["title"],"value":"Injected"},{"type":"set","path":["title"],"value":"Second"}"#])
      #expect(injected.0 != 0)
      #expect(NSDictionary(dictionary: try await run(["get", root.path])) == NSDictionary(dictionary: value))
      try await controller?.session.close()
    } catch { try? await controller?.session.close(); throw error }
  }

  @Test @MainActor func rejectedEditsPreserveSavedStateAndComposingDraft() async throws {
    let root = try captureFixture()
    defer { try? FileManager.default.removeItem(at: root) }
    let controller = try await SlopDocumentWindowController.open(packageURL: root)
    try await controller.session.waitUntilReady()
    let before = try await command("get", url: root)
    let saved = try Data(contentsOf: root.appendingPathComponent("state/document.sqlite"))
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
    try await controller.session.close()
    let after = try await command("get", url: root)
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
      "globalThis.stopFailure=globalThis.__slop.capture.onPrepare(()=>{throw new Error('intentional capture failure')});return true",
      arguments: [:], in: nil, contentWorld: .page)
    let failed = try await cli(["export", root.path, "--format", "pdf", "--output", pdf.path])
    #expect(failed.0 != 0)
    #expect(try Data(contentsOf: pdf) == previous)
    #expect(controller.session.capturing == false)
    _ = try await view.evaluateJavaScript("globalThis.stopFailure()")
    let rejected = try await cli([
      "export", root.path, "--format", "pdf", "--output",
      root.appendingPathComponent("bad.pdf").path,
    ])
    #expect(rejected.0 != 0)
    try await controller.session.close()
    let saved = try Data(contentsOf: root.appendingPathComponent("state/document.sqlite"))
    let closed = folder.appendingPathComponent("closed.pdf")
    let result = try await cli(["export", root.path, "--format", "pdf", "--output", closed.path])
    #expect(result.0 == 0, "\(result.2)")
    #expect(
      PDFDocument(data: try Data(contentsOf: closed))?.string?.contains("Selected view") == false)
    #expect(try Data(contentsOf: root.appendingPathComponent("state/document.sqlite")) == saved)
  }

  @Test @MainActor func staleEpochsAndUnavailableOwnersFailSafely() async throws {
    _ = NSApplication.shared
    let root = try fixture()
    defer { try? FileManager.default.removeItem(at: root) }
    let controller = try await SlopDocumentWindowController.open(packageURL: root)
    try await controller.session.waitUntilReady()
    let documentRoot = controller.session.package.rootURL
    let before = try await command("get", url: root)
    let path = try DocumentCommand.liveSocket(for: documentRoot)
    let stale = try JSONSerialization.data(withJSONObject: [
      "method": "batch", "documentPath": documentRoot.path, "epoch": "old",
      "ops": "[" + String(decoding: setTitle("must not apply"), as: UTF8.self) + "]",
    ])
    let response = try await Task.detached { try SocketClient.call(path: path, request: stale) }.value
    let refusal = try #require(try JSONSerialization.jsonObject(with: response) as? [String: Any])
    #expect(refusal["ok"] as? Bool == false)
    #expect(refusal["code"] as? String == "session_changed")
    #expect(try await command("get", url: root) == before)
    try await controller.session.close()
    let lock = try WriterLock.acquire(root)
    defer { lock.release() }
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
    let lock = try WriterLock.acquire(canonical)
    defer { lock.release() }
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
      let refusal = try JSONSerialization.data(withJSONObject: envelope)
      let server = try SocketServer { request, _ in
        if case .hello = request { return SocketReply(ok: true, epoch: "peer", coreBuildId: DocumentOwner.coreBuildID).encoded() }
        return refusal
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
    let data = try await command("apply", url: root, operation: setTitle("Engine only"))
    #expect(String(decoding: data, as: UTF8.self).contains("Engine only"))
  }

  // Failure: scalar edits from agents were refused, clear left a value behind, or an
  // out-of-range value was stored. Oracle: the CLI's printed value and exit status.
  @Test func scalarEditsSetClearAndRefuseFromTheCLI() async throws {
    let repository = #filePath.components(separatedBy: "/apps/apple/")[0]
    let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString + ".slop")
    defer { try? FileManager.default.removeItem(at: root) }
    try FileManager.default.copyItem(atPath: repository + "/tests/fixtures/scalars/document", toPath: root.path)
    let value = { (output: String) throws -> [String: Any] in
      let reply = try JSONSerialization.jsonObject(with: Data(output.utf8)) as! [String: Any]
      return reply["value"] as! [String: Any]
    }
    let set = try await cli(["batch", root.path, "--ops",
      #"[{"type":"set","path":["memo"],"value":"hi"},{"type":"set","path":["ratio"],"value":1},{"type":"set","path":["currency"],"value":"EUR"}]"#])
    #expect(set.0 == 0, "\(set.2)")
    let written = try value(set.1)
    #expect(written["memo"] as? String == "hi" && written["ratio"] as? Int == 1 && written["currency"] as? String == "EUR")
    let cleared = try await cli(["apply", root.path, "--op", #"{"type":"clear","path":["memo"]}"#])
    #expect(cleared.0 == 0, "\(cleared.2)")
    #expect(try value(cleared.1)["memo"] == nil)
    let refused = try await cli(["apply", root.path, "--op", #"{"type":"set","path":["rating"],"value":9}"#])
    #expect(refused.0 != 0)
    #expect(refused.2.contains("out_of_range") && refused.2.contains("Not applied."), "\(refused.2)")
  }

  // Failure: agents could not address record entries by key or scalar-list elements by
  // index. Oracle: the CLI's printed value.
  @Test func recordAndListEditsFromTheCLI() async throws {
    let repository = #filePath.components(separatedBy: "/apps/apple/")[0]
    let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString + ".slop")
    defer { try? FileManager.default.removeItem(at: root) }
    try FileManager.default.copyItem(atPath: repository + "/tests/fixtures/collections/document", toPath: root.path)
    let edited = try await cli(["batch", root.path, "--ops",
      ##"[{"type":"set","path":["done","3:5"],"value":true},{"type":"set","path":["cells","A1"],"value":{"input":"hi"}},{"type":"insert","path":["presets"],"value":75,"index":1},{"type":"set","path":["pixels",{"index":0}],"value":"#000"}]"##])
    #expect(edited.0 == 0, "\(edited.2)")
    let value = try #require((try JSONSerialization.jsonObject(with: Data(edited.1.utf8)) as? [String: Any])?["value"] as? [String: Any])
    #expect((value["done"] as? [String: Any])?["3:5"] as? Bool == true)
    #expect(value["presets"] as? [Int] == [60, 75, 90])
    #expect((value["pixels"] as? [String])?.first == "#000")
    let cleared = try await cli(["apply", root.path, "--op", #"{"type":"clear","path":["done","3:5"]}"#])
    #expect(cleared.0 == 0, "\(cleared.2)")
    let after = try #require((try JSONSerialization.jsonObject(with: Data(cleared.1.utf8)) as? [String: Any])?["value"] as? [String: Any])
    #expect((after["done"] as? [String: Any])?.isEmpty == true)
  }
}
