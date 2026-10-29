import AppKit
import Foundation
import Testing
import TinysCore
import TinysTestSupport

@testable import TinysHost

@Test @MainActor func documentWindowUsesTheTinyIconForMiniwindowAndDockMenu() async throws {
  let root = try documentWindowFixture(resizable: true)
  defer { try? FileManager.default.removeItem(at: root.deletingLastPathComponent()) }
  let controller = try await TinyDocumentWindowController.open(url: root)
  defer { controller.close() }
  let window = try #require(controller.window)

  #expect(window.styleMask.contains(.borderless))
  #expect(window.styleMask.contains(.miniaturizable))
  #expect(window.styleMask.contains(.resizable))
  #expect(
    window.validateMenuItem(
      NSMenuItem(title: "Close", action: #selector(NSWindow.performClose(_:)), keyEquivalent: "w")))
  #expect(window.representedURL?.standardizedFileURL == root.standardizedFileURL)
  #expect(window.title == "fixture.tiny")
  #expect(window.miniwindowTitle == "fixture.tiny")
  // The owner reads the icon off the main thread.
  await eventually(timeout: .seconds(2)) { window.miniwindowImage != nil }
  #expect(window.miniwindowImage != nil)
  #expect(controller.documentTitle == "fixture.tiny")
  #expect(controller.dockMenuImage.size == NSSize(width: 16, height: 16))
}

// The hover toolbar remains reachable when a document straddles display edges.
@Test func toolbarRemainsOnTheVisibleScreen() {
  let screen = NSRect(x: 100, y: 50, width: 1000, height: 800)
  let atEdge = NSRect(x: 990, y: 600, width: 240, height: 250)
  #expect(screen.contains(tinyToolbarFrame(document: atEdge, visible: screen)))
  // A borderless document can straddle displays; its toolbar must stay reachable
  // even when the document's top extends beyond this screen.
  let aboveScreen = NSRect(x: 990, y: 840, width: 240, height: 300)
  #expect(screen.contains(tinyToolbarFrame(document: aboveScreen, visible: screen)))
  let narrow = NSRect(x: -300, y: 0, width: 320, height: 600)
  let document = NSRect(x: -300, y: 200, width: 240, height: 200)
  #expect(narrow.contains(tinyToolbarFrame(document: document, visible: narrow)))
}

@Test @MainActor func nonResizableDocumentWindowStillMiniaturizes() async throws {
  let root = try documentWindowFixture(resizable: false)
  defer { try? FileManager.default.removeItem(at: root.deletingLastPathComponent()) }
  let controller = try await TinyDocumentWindowController.open(url: root)
  defer { controller.close() }
  let window = try #require(controller.window)

  #expect(window.styleMask.contains(.miniaturizable))
  #expect(!window.styleMask.contains(.resizable))
}

private func documentWindowFixture(resizable: Bool) throws -> URL {
  let stage = try Fixtures.minimalStage(fields: [
    "window": ["width": 320, "height": 240, "resizable": resizable]
  ])
  let icon = try Fixtures.png(width: 512, height: 512) { _ in
    NSColor.systemOrange.setFill()
    NSBezierPath(ovalIn: NSRect(x: 64, y: 64, width: 384, height: 384)).fill()
  }
  try FileManager.default.createDirectory(
    at: stage.appendingPathComponent("artwork"), withIntermediateDirectories: true)
  try Fixtures.addArtwork(stage, name: "icon", bytes: icon)
  return try Fixtures.document(stage: stage, at: Fixtures.folder().appendingPathComponent("fixture.tiny"))
}
