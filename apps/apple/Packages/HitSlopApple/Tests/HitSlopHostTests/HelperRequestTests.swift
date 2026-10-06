import AppKit
import Foundation
import HitSlopCore
import HitSlopCoreBinding
import PDFKit
import Testing
import HitSlopTestSupport

@testable import HitSlopHost
@testable import HitSlopDocument

extension HostTests {
  /// One request through the helper binary (`hitslop-native request`), and its reply.
  func request(_ body: [String: Any]) async throws -> [String: Any] {
    let result = try await cli(["request"], input: try JSONSerialization.data(withJSONObject: body))
    try #require(result.0 == 0, "\(body["method"] ?? ""): \(result.2)")
    return try #require(JSONSerialization.jsonObject(with: Data(result.1.utf8)) as? [String: Any])
  }

  @Test(arguments: [false, true]) @MainActor func helperRequestsReadAndEditLiveAndClosedDocuments(live: Bool) async throws {
    _ = NSApplication.shared
    let root = try fixture()
    defer { try? FileManager.default.removeItem(at: root) }
    let controller = live ? try await SlopDocumentWindowController.open(url: root) : nil
    try await controller?.session.waitUntilReady()
    do {
      let edit = try await request(["method": "batch", "documentPath": root.path,
        "ops": #"[{"type":"set","path":["title"],"value":"Batch edit"}]"#])
      #expect(edit["ok"] as? Bool == true)
      #expect(edit["ids"] as? [String] == [])
      #expect(edit["sequence"] as? Int != nil)
      #expect(edit["state"] == nil, "an edit replies with its IDs and sequence only")
      let get = try await request(["method": "get", "documentPath": root.path])
      let snapshot = try #require(get["state"] as? [String: Any])
      let state = try #require(snapshot["state"] as? [String: Any])
      #expect((state["value"] as? [String: Any])?["title"] as? String == "Batch edit")
      #expect(state["version"] is String)
      #expect(state["issues"] is [Any])
      let schema = try JSONSerialization.jsonObject(with: Data(SlopFile(url: root).descriptor.utf8)) as! [String: Any]
      #expect(NSDictionary(dictionary: try #require(snapshot["schema"] as? [String: Any])) == NSDictionary(dictionary: schema))
      // A malformed batch is refused before anything is sent: not applied.
      let malformed = try await request(["method": "batch", "documentPath": root.path, "ops": "{}"])
      #expect(malformed["ok"] as? Bool == false && malformed["code"] as? String == "rejected")
      try await controller?.session.close()
    } catch { try? await controller?.session.close(); throw error }
  }

  @Test @MainActor func rejectedEditsPreserveSavedStateAndComposingDraft() async throws {
    let root = try contractFixture()
    defer { try? FileManager.default.removeItem(at: root) }
    let controller = try await SlopDocumentWindowController.open(url: root)
    try await controller.session.waitUntilReady()
    let before = try await savedValue(root)
    let saved = try Data(contentsOf: root)
    let view = controller.session.webView
    _ = try await view.callAsyncJavaScript("""
      const input = document.querySelector('#draft');
      input.dispatchEvent(new CompositionEvent('compositionstart'));
      input.value = 'User is still typing';
      input.dispatchEvent(new InputEvent('input', {bubbles:true, isComposing:true}));
      return true;
      """, arguments: [:], in: nil, contentWorld: .page)
    let rejected = try await request(["method": "batch", "documentPath": root.path,
      "ops": #"[{"type":"set","path":["missing"],"value":true}]"#])
    #expect(rejected["ok"] as? Bool == false)
    #expect(rejected["code"] as? String == "rejected" && rejected["reason"] as? String == "path_not_found")
    #expect(try Data(contentsOf: root) == saved)
    #expect(try await view.evaluateJavaScript("document.querySelector('#draft').value") as? String == "User is still typing")
    // Close commits a composition in progress instead of refusing to close.
    try await controller.session.close()
    let after = try await savedValue(root)
    #expect(after != before)
    #expect(after?["title"] as? String == "User is still typing")
  }

  @Test @MainActor func helperExportsLiveSelectionAndClosedDefaultView() async throws {
    _ = NSApplication.shared
    let root = try contractFixture()
    let folder = root.deletingLastPathComponent().appendingPathComponent(UUID().uuidString)
    try FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true)
    defer {
      try? FileManager.default.removeItem(at: root)
      try? FileManager.default.removeItem(at: folder)
    }
    let controller = try await SlopDocumentWindowController.open(url: root)
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
    func export(_ format: String, to output: URL) async throws -> [String: Any] {
      try await request(["method": "export", "documentPath": root.path, "format": format, "output": output.path])
    }
    let pdf = folder.appendingPathComponent("live.pdf")
    let png = folder.appendingPathComponent("live.png")
    for (format, output) in [("pdf", pdf), ("png", png)] {
      let reply = try await export(format, to: output)
      #expect(reply["ok"] as? Bool == true, "\(reply)")
      #expect(reply["output"] as? String == output.path)
    }
    let text = try #require(PDFDocument(data: Data(contentsOf: pdf))?.string)
    #expect(text.contains("Selected view"))
    #expect(text.contains("CLI export pending title"))
    #expect(NSImage(data: try Data(contentsOf: png)) != nil)
    #expect(view.frame.size == originalSize)
    #expect(try NSBitmapImageRep(data: Data(contentsOf: png))?.pixelsWide == Int(originalSize.width * 2))
    #expect(try await view.evaluateJavaScript("globalThis.selectedView === 'Selected view'") as? Bool == true)
    #expect(
      try await view.evaluateJavaScript(
        "[...document.querySelectorAll('[data-slop-capture-target]')].every(e=>e.hidden && e.childElementCount===0)"
      ) as? Bool == true)
    let previous = try Data(contentsOf: pdf)
    _ = try await view.callAsyncJavaScript(
      "globalThis.stopFailure=globalThis.__slop.capture.onPrepare(()=>{throw new Error('intentional capture failure')});return true",
      arguments: [:], in: nil, contentWorld: .page)
    #expect(try await export("pdf", to: pdf)["ok"] as? Bool == false)
    #expect(try Data(contentsOf: pdf) == previous)
    #expect(controller.session.capturing == false)
    _ = try await view.evaluateJavaScript("globalThis.stopFailure()")
    // An export never replaces the document it renders.
    #expect(try await export("pdf", to: root)["ok"] as? Bool == false)
    try await controller.session.close()
    // A closed document exports its saved state with the initial view, taking no lock:
    // it renders even while another process holds the document.
    let saved = try Data(contentsOf: root)
    let holder = try NativeStore.open(path: root.path, mode: .document)
    defer { try? holder.close() }
    let closed = folder.appendingPathComponent("closed.pdf")
    let reply = try await export("pdf", to: closed)
    #expect(reply["ok"] as? Bool == true, "\(reply)")
    #expect(PDFDocument(data: try Data(contentsOf: closed))?.string?.contains("Selected view") == false)
    #expect(try Data(contentsOf: root) == saved)
  }

  @Test @MainActor func staleEpochsAreRefusedAsReplaced() async throws {
    _ = NSApplication.shared
    let root = try fixture()
    defer { try? FileManager.default.removeItem(at: root) }
    let controller = try await SlopDocumentWindowController.open(url: root)
    try await controller.session.waitUntilReady()
    let documentRoot = controller.session.file.url
    let before = try await savedValue(root)
    let path = try DocumentCommand.liveSocket(for: documentRoot)
    let stale = try JSONSerialization.data(withJSONObject: [
      "method": "batch", "documentPath": documentRoot.path, "epoch": "old",
      "ops": #"[{"type":"set","path":["title"],"value":"must not apply"}]"#,
    ])
    let response = try await Task.detached { try SocketClient.call(path: path, request: stale) }.value
    let refusal = try #require(try JSONSerialization.jsonObject(with: response) as? [String: Any])
    #expect(refusal["ok"] as? Bool == false)
    #expect(refusal["code"] as? String == "owner_replaced")
    #expect(try await savedValue(root) == before)
    try await controller.session.close()
  }

  // The helper passes a live owner's outcome through unchanged; the CLI says what it means.
  @Test @MainActor func peerOutcomesReachTheCLIUnchanged() async throws {
    let root = try fixture()
    defer { try? FileManager.default.removeItem(at: root) }
    let canonical = try SlopFile(url: root).url
    // An owner that is not this build's: it holds the document and names the peer below.
    let store = try NativeStore.open(path: canonical.path, mode: .document)
    defer { try? store.close() }
    let saved = try Data(contentsOf: canonical)
    for code in ["rejected", "owner_replaced", "closing", "save_failed", "unknown_outcome"] {
      let refusal = try JSONSerialization.data(withJSONObject: ["ok": false, "error": "Peer refusal", "code": code])
      let server = try SocketServer { request, _ in
        if case .hello = request { return SocketReply(ok: true, epoch: "peer", coreBuildId: DocumentOwner.coreBuildID).encoded() }
        return refusal
      }
      defer { server.stop() }
      try store.publishDiscovery(json: String(decoding: JSONSerialization.data(withJSONObject: [
        "socket": server.path, "documentPath": canonical.path,
      ]), as: UTF8.self))
      let reply = try await request(["method": "batch", "documentPath": root.path, "ops": "[]"])
      #expect(reply["code"] as? String == (code == "closing" ? "closing" : code), "\(reply)")
      #expect(reply["error"] as? String == "Peer refusal")
    }
    #expect(try Data(contentsOf: canonical) == saved)
  }
}
