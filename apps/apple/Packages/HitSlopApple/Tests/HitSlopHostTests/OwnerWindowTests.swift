import AppKit
import Foundation
import HitSlopCore
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
      let controller = try await SlopDocumentWindowController.open(packageURL: root)
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
      try SlopDuplicator.duplicate(from: root, to: duplicate)
      #expect(
        !FileManager.default.fileExists(
          atPath: duplicate.appendingPathComponent("state/host.lock").path))
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
      let reopened = try await SlopDocumentWindowController.open(packageURL: root)
      try await reopened.session.waitUntilReady()
      #expect(try await command("get", url: root) == saved)
      try await reopened.finishClose()
    }
  }
}

extension OwnerClientTests {
  @Test @MainActor func discardReattachesTheReplacementPageIncludingAfterRendererDeath() async throws {
    _ = NSApplication.shared
    for dead in [false, true] {
      let root = try contractFixture()
      defer { try? FileManager.default.removeItem(at: root) }
      let controller = try await SlopDocumentWindowController.open(packageURL: root)
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
