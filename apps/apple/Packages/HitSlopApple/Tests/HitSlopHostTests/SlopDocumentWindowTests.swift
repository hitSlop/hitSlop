import AppKit
import Foundation
import HitSlopCore
import Testing
@testable import HitSlopHost


@Test @MainActor func documentWindowUsesTheSlopIconForMiniwindowAndDockMenu() throws {
    let root = try documentWindowFixture(resizable: true)
    defer { try? FileManager.default.removeItem(at: root.deletingLastPathComponent()) }
    let controller = try SlopDocumentWindowController(packageURL: root)
    defer { controller.close() }
    let window = try #require(controller.window)

    #expect(window.styleMask.contains(.borderless))
    #expect(window.styleMask.contains(.miniaturizable))
    #expect(window.styleMask.contains(.resizable))
    #expect(window.validateMenuItem(NSMenuItem(title: "Close", action: #selector(NSWindow.performClose(_:)), keyEquivalent: "w")))
    #expect(window.representedURL?.standardizedFileURL == root.standardizedFileURL)
    #expect(window.title == "fixture.slop")
    #expect(window.miniwindowTitle == "fixture.slop")
    #expect(window.miniwindowImage != nil)
    #expect(controller.documentTitle == "fixture.slop")
    #expect(controller.dockMenuImage.size == NSSize(width: 16, height: 16))
}

// The hover toolbar remains reachable when a document straddles display edges.
@Test func toolbarRemainsOnTheVisibleScreen() {
    let screen = NSRect(x: 100, y: 50, width: 1000, height: 800)
    let atEdge = NSRect(x: 990, y: 600, width: 240, height: 250)
    #expect(screen.contains(slopToolbarFrame(document: atEdge, visible: screen)))
    // A borderless document can straddle displays; its toolbar must stay reachable
    // even when the document's top extends beyond this screen.
    let aboveScreen = NSRect(x: 990, y: 840, width: 240, height: 300)
    #expect(screen.contains(slopToolbarFrame(document: aboveScreen, visible: screen)))
    let narrow = NSRect(x: -300, y: 0, width: 320, height: 600)
    let document = NSRect(x: -300, y: 200, width: 240, height: 200)
    #expect(narrow.contains(slopToolbarFrame(document: document, visible: narrow)))
}

@Test @MainActor func nonResizableDocumentWindowStillMiniaturizes() throws {
    let root = try documentWindowFixture(resizable: false)
    defer { try? FileManager.default.removeItem(at: root.deletingLastPathComponent()) }
    let controller = try SlopDocumentWindowController(packageURL: root)
    defer { controller.close() }
    let window = try #require(controller.window)

    #expect(window.styleMask.contains(.miniaturizable))
    #expect(!window.styleMask.contains(.resizable))
}



private func documentWindowFixture(resizable: Bool) throws -> URL {
    let parent = FileManager.default.temporaryDirectory.appendingPathComponent("hitslop-document-window-\(UUID().uuidString)", isDirectory: true)
    let root = parent.appendingPathComponent("fixture.slop", isDirectory: true)
    try FileManager.default.createDirectory(at: root.appendingPathComponent("QuickLook"), withIntermediateDirectories: true)
    try FileManager.default.createDirectory(at: root.appendingPathComponent("assets"), withIntermediateDirectories: true)
    try Data("export default { mount() { return {}; } };".utf8).write(to: root.appendingPathComponent("assets/app.js"))
    try Data(#"{"format":1,"root":{"kind":"object","properties":{}}}"#.utf8).write(to: root.appendingPathComponent("state.schema.json"))
    try Data("{}".utf8).write(to: root.appendingPathComponent("initial.json"))
    try FileManager.default.createDirectory(at: root.appendingPathComponent("assets"), withIntermediateDirectories: true)
    try Data(#"{"runtimeContract":2,"minRuntimeRevision":1,"sdkVersion":"2.0.0","loroVersion":"1.16.1","protocolVersion":1}"#.utf8).write(to: root.appendingPathComponent("assets/runtime.json"))
    let resizableJSON = resizable ? "true" : "false"
    let manifest = #"{"runtime":"hitslop-v1","$schema":"https://api.hitslop.com/schemas/v1/manifest.schema.json","author":{"name":"Fixture Author","url":"https://example.com"},"slug":"miniwindow-fixture","title":"Miniwindow Fixture","description":"Tests document miniaturize chrome.","categories":["utilities"],"presentation":{"width":320,"height":240,"resizable":\#(resizableJSON)}}"#
    try Data(manifest.utf8).write(to: root.appendingPathComponent("manifest.json"))
    let skill = root.appendingPathComponent(".agents/skills/hitslop-document/SKILL.md")
    try FileManager.default.createDirectory(at: skill.deletingLastPathComponent(), withIntermediateDirectories: true)
    try Data(contentsOf: URL(fileURLWithPath: #filePath).deletingLastPathComponent().appendingPathComponent("../../../../../../packages/cli/skills/hitslop-document/SKILL.md").standardizedFileURL).write(to: skill)
    let bitmap = try #require(NSBitmapImageRep(bitmapDataPlanes: nil, pixelsWide: 512, pixelsHigh: 512, bitsPerSample: 8, samplesPerPixel: 4, hasAlpha: true, isPlanar: false, colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 0))
    NSGraphicsContext.saveGraphicsState()
    NSGraphicsContext.current = NSGraphicsContext(bitmapImageRep: bitmap)
    NSColor.systemOrange.setFill()
    NSBezierPath(ovalIn: NSRect(x: 64, y: 64, width: 384, height: 384)).fill()
    NSGraphicsContext.restoreGraphicsState()
    try #require(bitmap.representation(using: .png, properties: [:])).write(to: root.appendingPathComponent("QuickLook/Icon.png"))
    return root
}
