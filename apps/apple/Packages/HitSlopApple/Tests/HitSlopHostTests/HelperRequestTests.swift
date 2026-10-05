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

  @Test @MainActor func helperExportsSavedDefaultViewFromLiveAndClosedDocuments() async throws {
    _ = NSApplication.shared
    let root = try contractFixture { stage in
      let app = stage.appendingPathComponent("assets/app.js")
      let script = try String(contentsOf: app, encoding: .utf8).replacingOccurrences(
        of: "const doc = ctx.document;",
        with: "const doc = ctx.document; ctx.capture.onPrepare(() => { if (doc.current.title === 'Capture fails') throw new Error('intentional capture failure'); });")
      try Data(script.utf8).write(to: app)
    }
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
    #expect(text.contains("Default view"))
    #expect(!text.contains("Selected view"))
    #expect(text.contains("CLI export pending title"))
    #expect(NSImage(data: try Data(contentsOf: png)) != nil)
    #expect(view.frame.size == originalSize)
    #expect(try NSBitmapImageRep(data: Data(contentsOf: png))?.pixelsWide == 960)
    #expect(try await view.evaluateJavaScript("globalThis.selectedView === 'Selected view'") as? Bool == true)
    #expect(
      try await view.evaluateJavaScript(
        "[...document.querySelectorAll('[data-slop-capture-target]')].every(e=>e.hidden && e.childElementCount===0)"
      ) as? Bool == true)
    let previous = try Data(contentsOf: pdf)
    #expect(try await command("batch", url: root, setTitle("Capture fails")).ok)
    let failed = folder.appendingPathComponent("failed.pdf")
    #expect(try await export("pdf", to: failed)["ok"] as? Bool == false)
    #expect(!FileManager.default.fileExists(atPath: failed.path))
    #expect(try await export("pdf", to: pdf)["ok"] as? Bool == false)
    #expect(try Data(contentsOf: pdf) == previous)
    #expect(controller.session.capturing == false)
    #expect(try await command("batch", url: root, setTitle("CLI export pending title")).ok)
    // An export never replaces the document it renders.
    #expect(try await export("pdf", to: root)["ok"] as? Bool == false)
    try await controller.session.close()
    // A closed document exports its saved state with the initial view, taking no lock:
    // it renders even while another process holds the document.
    let saved = try Data(contentsOf: root)
    let holder = try DocumentOwner(url: root)
    let closed = folder.appendingPathComponent("closed.pdf")
    let reply = try await export("pdf", to: closed)
    #expect(reply["ok"] as? Bool == true, "\(reply)")
    #expect(PDFDocument(data: try Data(contentsOf: closed))?.string?.contains("Selected view") == false)
    #expect(try Data(contentsOf: root) == saved)
    try await holder.close()
  }

  // Failure: an engine of another build sent the live app a request it could not read,
  // and the refusal did not say which side to update. Oracle: the code and saved value.
  @Test @MainActor func aRequestInAnUnservedProtocolIsRefusedByTheLiveOwner() async throws {
    _ = NSApplication.shared
    let root = try fixture()
    defer { try? FileManager.default.removeItem(at: root) }
    let controller = try await SlopDocumentWindowController.open(url: root)
    try await controller.session.waitUntilReady()
    let documentRoot = controller.session.file.url
    let before = try await savedValue(root)
    let discovery = try #require(try liveDiscovery(path: documentRoot.path))
    let advertised = try Fixtures.object(discovery)
    let path = try #require(advertised["socket"] as? String)
    let documentPath = try #require(advertised["documentPath"] as? String)
    let newer = try JSONSerialization.data(withJSONObject: [
      "protocol": HelperProtocol.version + 1, "method": "batch", "documentPath": documentPath,
      "ops": #"[{"type":"set","path":["title"],"value":"must not apply"}]"#,
    ])
    let response = try await Task.detached { try SocketClient.call(path: path, request: newer) }.value
    let refusal = try #require(try JSONSerialization.jsonObject(with: response) as? [String: Any])
    #expect(refusal["ok"] as? Bool == false)
    #expect(refusal["reason"] as? String == "requires_update")
    #expect((refusal["error"] as? String)?.contains("update hitSlop") == true)
    #expect(try await savedValue(root) == before)
    try await controller.session.close()
  }

}
