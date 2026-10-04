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
  @Test @MainActor func nativeWindowExportsLiveEditsAndReleasesOwnership() async throws {
    _ = NSApplication.shared
    do {
      let root = try contractFixture()
      defer { try? FileManager.default.removeItem(at: root) }
      let controller = try await SlopDocumentWindowController.open(url: root)
      try await controller.session.waitUntilReady()
      weak var webView = controller.session.webView
      #expect(controller.window?.styleMask.contains(.titled) == false)
      #expect(!NSApp.windows.contains { $0 !== controller.window && controller.owns($0) })
      controller.showWindow(nil)
      await controller.waitForPresentation()
      // Hover follows the sampled pointer, so drive it with a point over the document.
      let frame = try #require(controller.window?.frame)
      controller.refreshToolbarHover(
        point: NSPoint(x: frame.midX, y: frame.midY), front: controller.window!.windowNumber)
      let panel = try #require(
        NSApp.windows.first { $0 !== controller.window && controller.owns($0) })
      #expect(panel.isVisible)
      panel.orderOut(nil)
      #expect(try await controller.session.webView.evaluateJavaScript("document.body.innerText.trim().length > 0") as? Bool == true)
      _ = try await command("apply", url: root, operation: setTitle("abcXYZ"))
      let live = try await command("get", url: root)
      #expect(String(decoding: live, as: UTF8.self).contains("abcXYZ"))
      _ = try await command("apply", url: root, operation: setTitle("Native socket edit"))
      let png = try await SlopRenderer.exportPNGData(session: controller.session)
      let pdf = try await SlopRenderer.exportPDFData(session: controller.session)
      #expect(NSImage(data: png) != nil)
      #expect(PDFDocument(data: pdf)?.string?.contains("Native socket edit") == true)
      #expect(controller.session.capturing == false)
      let duplicate = root.deletingLastPathComponent().appendingPathComponent(
        UUID().uuidString + ".slop")
      defer { try? FileManager.default.removeItem(at: duplicate) }
      try await controller.session.copy(to: duplicate)
      #expect(try liveDiscovery(path: duplicate.path) == nil)
      #expect(
        String(
          decoding: try await command("get", url: duplicate), as: UTF8.self
        ).contains("Native socket edit"))
      // Finishing at the controller boundary must also close native chrome and release WebKit.
      try await controller.finishClose()
      #expect(controller.window?.isVisible == false)
      #expect(webView == nil)
      _ = try await command("apply", url: root, operation: setTitle("Closed WASM edit"))
      #expect(
        String(decoding: try await command("get", url: root), as: UTF8.self)
          .contains("Closed WASM edit"))
      // An interactive reopen reads back exactly the saved state (the probe app writes nothing).
      let saved = try await command("get", url: root)
      let reopened = try await SlopDocumentWindowController.open(url: root)
      try await reopened.session.waitUntilReady()
      #expect(try await command("get", url: root) == saved)
      try await reopened.finishClose()
    }
  }
}

extension OwnerClientTests {
  // Failure: redoing the page's text after undoing an agent's edit produced PRSOAGENT.
  // Exercise the focused window's Edit actions, page publication, and saved reopen.
  @Test @MainActor func agentAndPageEditsRedoThroughTheWindow() async throws {
    _ = NSApplication.shared
    let root = try contractFixture()
    defer { try? FileManager.default.removeItem(at: root) }
    let controller = try await SlopDocumentWindowController.open(url: root)
    try await controller.session.waitUntilReady()
    controller.showWindow(nil)
    await controller.waitForPresentation()
    _ = try await command("apply", url: root, operation: setTitle("AGENT"))
    let window = try #require(controller.window)
    let webView = controller.session.webView
    window.makeFirstResponder(webView)
    // Type over the agent's text once the page shows it.
    _ = try await webView.callAsyncJavaScript(
      "const input = document.getElementById('draft'); for (let i = 0; i < 200 && input.value !== 'AGENT'; i++) await new Promise(r => setTimeout(r, 5)); input.focus(); input.select(); document.execCommand('insertText', false, 'PERSON'); await globalThis.__slop.flush(); return true",
      arguments: [:], in: nil, contentWorld: .page)
    func title() async throws -> String? {
      let reply = try JSONSerialization.jsonObject(with: try await command("get", url: root)) as! [String: Any]
      return (reply["state"] as? [String: Any] ?? reply)["title"] as? String
    }
    #expect(try await title() == "PERSON")
    for _ in 0..<3 {
      for (action, expected) in [("undo:", "AGENT"), ("undo:", "abc"), ("redo:", "AGENT"), ("redo:", "PERSON")] {
        #expect(window.firstResponder?.tryToPerform(Selector((action)), with: nil) == true)
        var current = try await title()
        for _ in 0..<200 where current != expected {
          try await Task.sleep(for: .milliseconds(5))
          current = try await title()
        }
        #expect(current == expected)
        #expect(try await webView.evaluateJavaScript("document.getElementById('draft').value") as? String == expected)
      }
    }
    try await controller.finishClose()
    #expect(try await title() == "PERSON", "redo was saved before close")
  }

  // Failure: Edit ▸ Undo reached only WebKit's own text undo, which knows nothing of the
  // document. Oracle: the undo action, sent from the focused page, reverts the agent's
  // edit, then the person's typing in the document and in the field.
  @Test @MainActor func editUndoRevertsThePersonsTypingInTheDocument() async throws {
    _ = NSApplication.shared
    let root = try contractFixture()
    defer { try? FileManager.default.removeItem(at: root) }
    let controller = try await SlopDocumentWindowController.open(url: root)
    try await controller.session.waitUntilReady()
    controller.showWindow(nil)
    await controller.waitForPresentation()
    let window = try #require(controller.window)
    let webView = controller.session.webView
    window.makeFirstResponder(webView)
    #expect(window.undoManager is DocumentUndoManager)
    #expect(window.undoManager?.canUndo == false)
    let field = "document.getElementById('draft')"
    _ = try await webView.callAsyncJavaScript(
      "const input = \(field); input.focus(); input.setSelectionRange(input.value.length, input.value.length); document.execCommand('insertText', false, 'XYZ'); await globalThis.__slop.flush(); return true",
      arguments: [:], in: nil, contentWorld: .page)
    _ = try await command("apply", url: root, operation: try JSONSerialization.data(withJSONObject: ["type": "increment", "path": ["hits"], "by": 3]))
    func saved() async throws -> [String: Any] {
      let reply = try JSONSerialization.jsonObject(with: try await command("get", url: root)) as! [String: Any]
      return reply["state"] as? [String: Any] ?? reply
    }
    #expect(try await saved()["title"] as? String == "abcXYZ")
    for _ in 0..<200 where window.undoManager?.canUndo != true { try await Task.sleep(for: .milliseconds(5)) }
    #expect(window.undoManager?.canUndo == true)
    #expect(window.firstResponder?.tryToPerform(Selector(("undo:")), with: nil) == true)
    var state = try await saved()
    for _ in 0..<200 where state["hits"] as? Int != 0 {
      try await Task.sleep(for: .milliseconds(5))
      state = try await saved()
    }
    #expect(state["hits"] as? Int == 0, "the agent's edit goes first")
    #expect(state["title"] as? String == "abcXYZ")
    #expect(window.firstResponder?.tryToPerform(Selector(("undo:")), with: nil) == true)
    for _ in 0..<200 where state["title"] as? String != "abc" {
      try await Task.sleep(for: .milliseconds(5))
      state = try await saved()
    }
    #expect(state["title"] as? String == "abc")
    #expect(try await webView.evaluateJavaScript("\(field).value") as? String == "abc")
    for _ in 0..<200 where window.undoManager?.canUndo != false { try await Task.sleep(for: .milliseconds(5)) }
    #expect(window.undoManager?.canUndo == false)
    #expect(window.undoManager?.canRedo == true)
    #expect(window.firstResponder?.tryToPerform(Selector(("redo:")), with: nil) == true)
    state = try await saved()
    for _ in 0..<200 where state["title"] as? String != "abcXYZ" {
      try await Task.sleep(for: .milliseconds(5))
      state = try await saved()
    }
    #expect(state["title"] as? String == "abcXYZ")
    try await controller.finishClose()
  }

  @Test @MainActor func discardReattachesTheReplacementPageIncludingAfterRendererDeath() async throws {
    _ = NSApplication.shared
    for dead in [false, true] {
      let root = try contractFixture()
      defer { try? FileManager.default.removeItem(at: root) }
      let controller = try await SlopDocumentWindowController.open(url: root)
      try await controller.session.waitUntilReady()
      let old = controller.session.webView
      if dead { controller.session.webViewWebContentProcessDidTerminate(old) }
      try await controller.session.discardPending()
      #expect(controller.session.webView !== old)
      #expect(controller.session.webView.superview != nil)
      try await controller.session.waitUntilReady()
      #expect(controller.session.isReady)
      // Recovery clears the failure overlay and re-enables export.
      #expect(controller.presentedPageError == nil)
      #expect(!controller.session.rendererDead)
      try await controller.finishClose()
    }
  }
}
