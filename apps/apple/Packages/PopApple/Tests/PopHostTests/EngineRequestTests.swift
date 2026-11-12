import AppKit
import Foundation
import PDFKit
import PopCore
import PopCoreBinding
import PopTestSupport
import Testing

@testable import PopDocument
@testable import PopHost

extension HostTests {
  /// One request as `pop` sends it (`pop-engine`), and its reply.
  func request(_ body: [String: Any], isolation: isolated (any Actor)? = #isolation) async throws -> [String: Any] {
    let result = try await cli(input: try JSONSerialization.data(withJSONObject: body))
    try #require(result.0 == 0, "\(body["method"] ?? ""): \(result.2)")
    return try #require(JSONSerialization.jsonObject(with: Data(result.1.utf8)) as? [String: Any])
  }

  @Test @MainActor func rejectedEditsPreserveSavedStateAndComposingDraft() async throws {
    let root = try contractFixture()
    defer { try? FileManager.default.removeItem(at: root) }
    let controller = try await PopDocumentWindowController.open(url: root)
    try await controller.session.waitUntilReady()
    let before = try await savedValue(root)
    let saved = try Data(contentsOf: root)
    let view = controller.session.webView
    _ = try await view.callAsyncJavaScript(
      """
      const input = document.querySelector('#draft');
      input.dispatchEvent(new CompositionEvent('compositionstart'));
      input.value = 'User is still typing';
      input.dispatchEvent(new InputEvent('input', {bubbles:true, isComposing:true}));
      return true;
      """, arguments: [:], in: nil, contentWorld: .page)
    let rejected = try await request([
      "method": "batch", "documentPath": root.path,
      "batch": [
        "intents": try JSONSerialization.jsonObject(
          with: Data((#"[{"type":"set","path":["missing"],"value":true}]"#).utf8))
      ],
    ])
    #expect(rejected["ok"] as? Bool == false)
    #expect(rejected["code"] as? String == "rejected" && rejected["reason"] as? String == "path_not_found")
    #expect(try Data(contentsOf: root) == saved)
    #expect(
      try await view.evaluateJavaScript("document.querySelector('#draft').value") as? String == "User is still typing")
    // Close commits a composition in progress instead of refusing to close.
    try await controller.session.close()
    let after = try await savedValue(root)
    #expect(after != before)
    #expect(after?["title"] as? String == "User is still typing")
  }

  @Test @MainActor func engineExportsSavedCaptureViewFromLiveAndClosedDocuments() async throws {
    _ = NSApplication.shared
    let root = try contractFixture { stage in
      let program = stage.appendingPathComponent("assets/export.js")
      let script = try String(contentsOf: program, encoding: .utf8).replacingOccurrences(
        of: "const request = JSON.parse(json);",
        with: """
          const request = JSON.parse(json);
          if (request.snapshot.title === 'Capture fails') throw new Error('intentional capture failure');
          """)
      try Data(script.utf8).write(to: program)
    }
    let folder = root.deletingLastPathComponent().appendingPathComponent(UUID().uuidString)
    try FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true)
    defer {
      try? FileManager.default.removeItem(at: root)
      try? FileManager.default.removeItem(at: folder)
    }
    let controller = try await PopDocumentWindowController.open(url: root)
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
    #expect(!text.contains("Selected view"))
    #expect(text.contains("CLI export pending title"))
    #expect(NSImage(data: try Data(contentsOf: png)) != nil)
    #expect(view.frame.size == originalSize)
    #expect(try NSBitmapImageRep(data: Data(contentsOf: png))?.pixelsWide == 960)
    #expect(try await view.evaluateJavaScript("globalThis.selectedView === 'Selected view'") as? Bool == true)
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
    // A closed document exports its saved capture view, taking no lock:
    // it renders even while another process holds the document.
    let saved = try Data(contentsOf: root)
    let holder = try DocumentOwner(url: root)
    let closed = folder.appendingPathComponent("closed.pdf")
    let reply = try await export("pdf", to: closed)
    #expect(reply["ok"] as? Bool == true, "\(reply)")
    let closedText = try #require(PDFDocument(data: Data(contentsOf: closed))?.string)
    #expect(closedText.contains("CLI export pending title"))
    #expect(!closedText.contains("Selected view"))
    #expect(try Data(contentsOf: root) == saved)
    try await holder.close()
  }

  // Failure: a closed document a newer pop wrote failed to export with an unknown
  // outcome, where every other command asks for an update. Oracle: the refusal's code and
  // reason, and no output.
  @Test func aClosedExportOfANewerDocumentAsksForAnUpdate() async throws {
    let root = try fixture()
    defer { try? FileManager.default.removeItem(at: root) }
    try Fixtures.sql(root, "UPDATE app SET runtime_abi = \(RuntimeABI.level + 1)")
    let output = root.deletingLastPathComponent().appendingPathComponent(UUID().uuidString + ".pdf")
    let reply = try await request([
      "method": "export", "documentPath": root.path, "format": "pdf", "output": output.path,
    ])
    #expect(reply["ok"] as? Bool == false)
    #expect(reply["code"] as? String == "rejected", "\(reply)")
    #expect(reply["reason"] as? String == "requires_update")
    #expect(!FileManager.default.fileExists(atPath: output.path))
  }

}
