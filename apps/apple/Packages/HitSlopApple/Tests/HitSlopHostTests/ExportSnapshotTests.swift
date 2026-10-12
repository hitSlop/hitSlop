import AppKit
import Foundation
import HitSlopCore
import HitSlopCoreBinding
import HitSlopTestSupport
import HitSlopWidgets
import Testing

@testable import HitSlopDocument
@testable import HitSlopHost

extension HostTests {
  @Test @MainActor func widgetCapturesSavedStateAtEveryFamilySize() async throws {
    let root = try contractFixture { stage in
      try Fixtures.updateApp(stage) { $0["views"] = ["export": false, "icon": false, "widget": [:]] as [String: Any] }
      try Fixtures.addWidget(stage)
    }
    defer { try? FileManager.default.removeItem(at: root) }
    let session = try await DocumentSession.open(url: root)
    session.load()
    try await session.waitUntilReady()
    do {
      #expect(try await command("batch", url: root, setTitle("Widget saved")).ok)
      let images = try await SlopRenderer.widgetImages(session: session)
      #expect(images.count == 3)
      for viewport in widgetViewports() {
        let image = try #require(images.first { $0.family == WidgetImageFamily(viewport.family) })
        let bitmap = try #require(NSBitmapImageRep(data: image.png))
        #expect(bitmap.pixelsWide == Int(viewport.width) * 2)
        #expect(bitmap.pixelsHigh == Int(viewport.height) * 2)
        let color = try #require(
          bitmap.colorAt(x: bitmap.pixelsWide / 2, y: bitmap.pixelsHigh / 2)?.usingColorSpace(.deviceRGB))
        #expect(color.redComponent > 0.95 && color.blueComponent < 0.05)
      }
    } catch {
      try await session.close()
      throw error
    }
    try await session.close()
    let unsupported = try contractFixture()
    defer { try? FileManager.default.removeItem(at: unsupported) }
    await #expect(throws: (any Error).self) { try await SlopRenderer.widgetImages(url: unsupported) }
  }

  @Test @MainActor func widgetCacheKeepsCompleteGenerationsAndTracksFilesAcrossRename() throws {
    let folder = try Fixtures.folder()
    defer { try? FileManager.default.removeItem(at: folder) }
    let cache = WidgetCache(root: folder.appendingPathComponent("cache"))
    let source = folder.appendingPathComponent("First.slop")
    try Data("document".utf8).write(to: source)
    let png = try Fixtures.png(width: 32, height: 32) {
      NSColor.red.setFill()
      $0.fill()
    }
    let images = widgetViewports().map { WidgetImage(family: WidgetImageFamily($0.family), png: png) }
    let time = Date()
    let first = try cache.publish(url: source, title: "First", images: images, capturedAt: time, refreshMinutes: nil)
    #expect(cache.records().count == 1)
    #expect(WidgetCache.selectionID(from: WidgetCache.openURL(id: first.id)) == first.id)
    #expect(WidgetCache.selectionID(from: URL(string: "hitslop://widget/../../other")!) == nil)
    #expect(throws: (any Error).self) {
      try cache.publish(
        url: source, title: "Incomplete", images: Array(images.prefix(2)), capturedAt: time.addingTimeInterval(1),
        refreshMinutes: nil)
    }
    #expect(cache.record(id: first.id)?.generation == first.generation)
    let older = try cache.publish(
      url: source, title: "Older", images: images, capturedAt: time.addingTimeInterval(-1), refreshMinutes: nil)
    #expect(older.generation == first.generation)
    let next = try cache.publish(
      url: source, title: "Next", images: images, capturedAt: time.addingTimeInterval(1), refreshMinutes: nil)
    let newest = try cache.publish(
      url: source, title: "Newest", images: images, capturedAt: time.addingTimeInterval(2), refreshMinutes: nil)
    #expect(!FileManager.default.fileExists(atPath: cache.imageURL(first, .small).path))
    #expect(FileManager.default.fileExists(atPath: cache.imageURL(next, .small).path))
    #expect(cache.imageData(first, .small) == png, "a reader with an expired pointer retries the current generation")
    #expect(cache.record(id: first.id)?.generation == newest.generation)
    let copy = folder.appendingPathComponent("Copy.slop")
    try FileManager.default.copyItem(at: source, to: copy)
    let second = try cache.publish(url: copy, title: "Copy", images: images, capturedAt: time, refreshMinutes: nil)
    #expect(second.id != first.id)
    let renamed = folder.appendingPathComponent("Renamed.slop")
    try FileManager.default.moveItem(at: source, to: renamed)
    #expect(try cache.resolve(first).resolvingSymlinksInPath() == renamed.resolvingSymlinksInPath())
    #expect(cache.record(for: renamed)?.id == first.id)
    try FileManager.default.removeItem(at: renamed)
    try Data("replacement".utf8).write(to: renamed)
    #expect(throws: (any Error).self) { try cache.resolve(first) }
    #expect(cache.pruneUnavailable())
    #expect(cache.record(id: first.id) == nil)
    #expect(cache.record(id: second.id) != nil)
    try FileManager.default.removeItem(at: copy)
    #expect(cache.pruneUnavailable())
    #expect(cache.records().isEmpty)
  }

  @Test(arguments: [false, true]) @MainActor func copyKeepsTheStateUsedForItsArtwork(closeDuringRender: Bool)
    async throws
  {
    let root = try contractFixture()
    let destination = root.deletingLastPathComponent().appendingPathComponent(UUID().uuidString + ".slop")
    defer {
      try? FileManager.default.removeItem(at: root)
      try? FileManager.default.removeItem(at: destination)
    }
    let session = try await DocumentSession.open(url: root)
    session.load()
    try await session.waitUntilReady()
    #expect(try await command("batch", url: root, setTitle("Before rendering")).ok)
    let png = try Fixtures.png(width: 32, height: 32) {
      NSColor.red.setFill()
      $0.fill()
    }
    var capturedTitle: String?
    var renderError: Error?
    try await session.copy(to: destination) { source in
      do {
        capturedTitle = try await savedValue(source)?["title"] as? String
        #expect(try await command("batch", url: root, setTitle("After rendering began")).ok)
        if closeDuringRender { try await session.close() }
      } catch { renderError = error }
      return SlopRenderedArtwork(preview: png, icon: nil)
    }
    #expect(renderError == nil)
    #expect(capturedTitle == "Before rendering")
    #expect(try await savedValue(destination)?["title"] as? String == capturedTitle)
    #expect(Fixtures.pixels(SlopArtwork.png(destination, .preview)) == Fixtures.pixels(png))
    #expect(try await savedValue(root)?["title"] as? String == "After rendering began")
    try await session.close()
  }

  // An acquired export snapshot is independent of its window: it renders after the
  // window's owner discards and closes.
  @Test @MainActor func acquiredExportSnapshotMayFinishAfterDiscardAndClose() async throws {
    let root = try contractFixture()
    let output = root.deletingLastPathComponent().appendingPathComponent(UUID().uuidString + ".pdf")
    defer {
      try? FileManager.default.removeItem(at: root)
      try? FileManager.default.removeItem(at: output)
    }
    let session = try await DocumentSession.open(url: root)
    session.load()
    try await session.waitUntilReady()
    try await session.withCaptureSnapshot { source in
      try await session.discardPending()
      try await session.close()
      try await SlopRenderer.exportClosed(source, format: .pdf, output: output)
    }
    #expect(FileManager.default.fileExists(atPath: output.path))
  }
}
