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
      controller.toolbar.refresh(
        point: NSPoint(x: frame.midX, y: frame.midY), front: controller.window!.windowNumber)
      let panel = try #require(
        NSApp.windows.first { $0 !== controller.window && controller.owns($0) })
      #expect(panel.isVisible)
      panel.orderOut(nil)
      #expect(
        try await controller.session.webView.evaluateJavaScript("document.body.innerText.trim().length > 0") as? Bool
          == true)
      #expect(try await command("batch", url: root, setTitle("abcXYZ")).ok)
      #expect(try await savedValue(root)?["title"] as? String == "abcXYZ")
      #expect(try await command("batch", url: root, setTitle("Native socket edit")).ok)
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
      #expect(try await savedValue(duplicate)?["title"] as? String == "Native socket edit")
      // Finishing at the controller boundary must also close native chrome and release WebKit.
      try await controller.finishClose()
      #expect(controller.window?.isVisible == false)
      #expect(webView == nil)
      #expect(try await command("batch", url: root, setTitle("Closed edit")).ok)
      #expect(try await savedValue(root)?["title"] as? String == "Closed edit")
      // An interactive reopen reads back exactly the saved state (the probe app writes nothing).
      let saved = try await savedValue(root)
      let reopened = try await SlopDocumentWindowController.open(url: root)
      try await reopened.session.waitUntilReady()
      #expect(try await savedValue(root) == saved)
      try await reopened.finishClose()
    }
  }
}

extension HostTests {
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
    #expect(try await command("batch", url: root, setTitle("AGENT")).ok)
    let window = try #require(controller.window)
    let webView = controller.session.webView
    window.makeFirstResponder(webView)
    // Type over the agent's text once the page shows it.
    _ = try await webView.callAsyncJavaScript(
      "const input = document.getElementById('draft'); for (let i = 0; i < 200 && input.value !== 'AGENT'; i++) await new Promise(r => setTimeout(r, 5)); input.focus(); input.select(); document.execCommand('insertText', false, 'PERSON'); await globalThis.__slop.flush(); return true",
      arguments: [:], in: nil, contentWorld: .page)
    func title() async throws -> String? { try await savedValue(root)?["title"] as? String }
    #expect(try await title() == "PERSON")
    for _ in 0..<3 {
      for (action, expected) in [("undo:", "AGENT"), ("undo:", "abc"), ("redo:", "AGENT"), ("redo:", "PERSON")] {
        #expect(window.firstResponder?.tryToPerform(Selector((action)), with: nil) == true)
        var current: String?
        try await eventually(timeout: .seconds(1)) {
          current = try await title()
          return current == expected
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
    #expect(
      try await command(
        "batch", url: root,
        [
          "batch": [
            "intents": try JSONSerialization.jsonObject(
              with: Data((#"[{"type":"increment","path":["hits"],"by":3}]"#).utf8))
          ]
        ]
      ).ok)
    func saved() async throws -> [String: Any] { try await savedValue(root) as? [String: Any] ?? [:] }
    #expect(try await saved()["title"] as? String == "abcXYZ")
    await eventually(timeout: .seconds(1)) { window.undoManager?.canUndo == true }
    #expect(window.undoManager?.canUndo == true)
    #expect(window.firstResponder?.tryToPerform(Selector(("undo:")), with: nil) == true)
    var state: [String: Any] = [:]
    try await eventually(timeout: .seconds(1)) {
      state = try await saved()
      return state["hits"] as? Int == 0
    }
    #expect(state["hits"] as? Int == 0, "the agent's edit goes first")
    #expect(state["title"] as? String == "abcXYZ")
    #expect(window.firstResponder?.tryToPerform(Selector(("undo:")), with: nil) == true)
    try await eventually(timeout: .seconds(1)) {
      state = try await saved()
      return state["title"] as? String == "abc"
    }
    #expect(state["title"] as? String == "abc")
    #expect(try await webView.evaluateJavaScript("\(field).value") as? String == "abc")
    await eventually(timeout: .seconds(1)) { window.undoManager?.canUndo == false }
    #expect(window.undoManager?.canUndo == false)
    #expect(window.undoManager?.canRedo == true)
    #expect(window.firstResponder?.tryToPerform(Selector(("redo:")), with: nil) == true)
    try await eventually(timeout: .seconds(1)) {
      state = try await saved()
      return state["title"] as? String == "abcXYZ"
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
