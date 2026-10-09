import AppKit
import Foundation
import HitSlopCore
import HitSlopCoreBinding
import HitSlopTestSupport
import PDFKit
import Testing

@testable import HitSlopDocument
@testable import HitSlopHost

extension HostTests {
  @Test func screenshotsRefuseDocumentDestinations() async throws {
    let root = try contractFixture()
    let before = try Data(contentsOf: root)
    let folder = try Fixtures.folder()
    defer {
      try? FileManager.default.removeItem(at: root)
      try? FileManager.default.removeItem(at: folder)
    }
    let other = folder.appendingPathComponent("other.SLOP")
    let renamed = folder.appendingPathComponent("renamed.png")
    let symbolic = folder.appendingPathComponent("symbolic.png")
    let hard = folder.appendingPathComponent("hard.png")
    try before.write(to: other)
    try before.write(to: renamed)
    try FileManager.default.createSymbolicLink(at: symbolic, withDestinationURL: root)
    try FileManager.default.linkItem(at: root, to: hard)
    for output in [root, other, renamed, symbolic, hard] {
      let body = try JSONSerialization.data(withJSONObject: [
        "method": "screenshot", "documentPath": root.path, "output": output.path,
        "target": "preview", "ifPresent": false,
      ])
      let (_, json, _) = try await cli(input: body, tool: "hitslop-native")
      let reply = try #require(JSONSerialization.jsonObject(with: Data(json.utf8)) as? [String: Any])
      #expect(reply["ok"] as? Bool == false)
      #expect(reply["reason"] as? String == "invalid_request")
      #expect(try Data(contentsOf: output) == before)
    }
  }

  @Test func nativeWireCrossesFFIAsTypes() throws {
    let request = try decodeNativeRequest(
      input: Data(
        #"{"method":"screenshot","documentPath":"x.slop","output":"x.png","target":"icon","ifPresent":true}"#.utf8))
    guard case .screenshot(let path, let output, let target, let ifPresent) = request else {
      Issue.record("Expected a typed screenshot request")
      return
    }
    #expect(path == "x.slop" && output == "x.png" && target == .icon && ifPresent)
    let reply = encodeNativeReply(reply: .screenshot(output: nil))
    let fields = try #require(JSONSerialization.jsonObject(with: Data(reply.utf8)) as? [String: Any])
    #expect(fields["ok"] as? Bool == true && fields["method"] as? String == "screenshot")
    #expect(fields["output"] is NSNull)
    do {
      _ = try decodeNativeRequest(input: Data(#"{"method":"open","documentPath":""}"#.utf8))
      Issue.record("An empty path must be refused before host work")
    } catch NativeRefusal.Refused(let reply) {
      guard case .failure(_, let code, let reason, _) = reply else {
        Issue.record("Expected a classified failure")
        return
      }
      #expect(code == .rejected && reason == .invalidRequest)
    }
  }

  /// One request as `slop` sends it (`slop-engine`), and its reply.
  func request(_ body: [String: Any], isolation: isolated (any Actor)? = #isolation) async throws -> [String: Any] {
    let result = try await cli(input: try JSONSerialization.data(withJSONObject: body))
    try #require(result.0 == 0, "\(body["method"] ?? ""): \(result.2)")
    return try #require(JSONSerialization.jsonObject(with: Data(result.1.utf8)) as? [String: Any])
  }

  // The helper renders; documents are read and edited through slop-engine alone, so the
  // helper is not a second way in. Oracle: the refusal's code and reason.
  @Test func theHelperRefusesDocumentEdits() async throws {
    let root = try fixture()
    defer { try? FileManager.default.removeItem(at: root) }
    let body = try JSONSerialization.data(withJSONObject: ["method": "get", "documentPath": root.path])
    let (status, output, errors) = try await cli(input: body, tool: "hitslop-native")
    try #require(status == 0, "\(errors)")
    let reply = try #require(try JSONSerialization.jsonObject(with: Data(output.utf8)) as? [String: Any])
    #expect(reply["ok"] as? Bool == false)
    #expect(reply["code"] as? String == "rejected")
    #expect(reply["reason"] as? String == "invalid_request")
  }

  @Test(arguments: [false, true]) @MainActor func engineRequestsReadAndEditLiveAndClosedDocuments(live: Bool)
    async throws
  {
    _ = NSApplication.shared
    let root = try fixture()
    defer { try? FileManager.default.removeItem(at: root) }
    let controller = live ? try await SlopDocumentWindowController.open(url: root) : nil
    try await controller?.session.waitUntilReady()
    do {
      let edit = try await request([
        "method": "batch", "documentPath": root.path,
        "batch": [
          "intents": try JSONSerialization.jsonObject(
            with: Data((#"[{"type":"set","path":["title"],"value":"Batch edit"}]"#).utf8))
        ],
      ])
      #expect(edit["ok"] as? Bool == true)
      #expect(edit["ids"] as? [String] == [])
      #expect(Set(edit.keys) == ["ok", "method", "ids"], "an edit replies with its inserted IDs only")
      let get = try await request(["method": "get", "documentPath": root.path])
      let snapshot = try #require(get["state"] as? [String: Any])
      #expect((snapshot["value"] as? [String: Any])?["title"] as? String == "Batch edit")
      #expect(snapshot["version"] is String)
      let schema = try JSONSerialization.jsonObject(with: Data(SlopFile(url: root).descriptor.utf8)) as! [String: Any]
      #expect(
        NSDictionary(dictionary: try #require(snapshot["schema"] as? [String: Any])) == NSDictionary(dictionary: schema)
      )
      // A malformed batch is refused before anything is sent: not applied.
      let malformed = try await request(["method": "batch", "documentPath": root.path, "ops": "{}"])
      #expect(malformed["ok"] as? Bool == false && malformed["code"] as? String == "rejected")
      try await controller?.session.close()
    } catch {
      try? await controller?.session.close()
      throw error
    }
  }

  @Test @MainActor func browserSnapshotDrainsLiveDraftAndReopensIndependently() async throws {
    let root = try contractFixture()
    let folder = try Fixtures.folder()
    defer {
      try? FileManager.default.removeItem(at: root)
      try? FileManager.default.removeItem(at: folder)
    }
    let controller = try await SlopDocumentWindowController.open(url: root)
    do {
      try await controller.session.waitUntilReady()
      _ = try await controller.session.webView.callAsyncJavaScript(
        """
        const input = document.querySelector('#draft');
        input.focus();
        input.dispatchEvent(new CompositionEvent('compositionstart'));
        input.value = 'Browser snapshot draft';
        input.dispatchEvent(new InputEvent('input', {bubbles:true, isComposing:true}));
        return true;
        """, arguments: [:], in: nil, contentWorld: .page)
      let output = folder.appendingPathComponent("browser.slop")
      let reply = try await request(["method": "copy", "documentPath": root.path, "output": output.path])
      #expect(reply["ok"] as? Bool == true, "\(reply)")
      #expect(try await savedValue(output)?["title"] as? String == "Browser snapshot draft")
      #expect(try await command("batch", url: output, setTitle("Independent copy")).ok)
      #expect(try await savedValue(root)?["title"] as? String == "Browser snapshot draft")
      try await controller.session.close()
    } catch {
      try? await controller.session.close()
      throw error
    }
  }

  @Test @MainActor func rejectedEditsPreserveSavedStateAndComposingDraft() async throws {
    let root = try contractFixture()
    defer { try? FileManager.default.removeItem(at: root) }
    let controller = try await SlopDocumentWindowController.open(url: root)
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

  @Test @MainActor func engineExportsSavedDefaultViewFromLiveAndClosedDocuments() async throws {
    _ = NSApplication.shared
    let root = try contractFixture { stage in
      let app = stage.appendingPathComponent("assets/ui.js")
      let script = try String(contentsOf: app, encoding: .utf8).replacingOccurrences(
        of: "const doc = ctx.document;",
        with:
          "const doc = ctx.document; ctx.capture.onPrepare(() => { if (doc.current.title === 'Capture fails') throw new Error('intentional capture failure'); });"
      )
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

  // Failure: a closed document a newer hitSlop wrote failed to export with an unknown
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
      "batch": [
        "intents": try JSONSerialization.jsonObject(
          with: Data((#"[{"type":"set","path":["title"],"value":"must not apply"}]"#).utf8))
      ],
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
