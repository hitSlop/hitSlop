import AppKit
import Foundation
import HitSlopCore
import HitSlopTestSupport
import HitSlopWidgets
import Testing

@testable import HitSlopDocument
@testable import HitSlopHost

extension HostTests {
  @Test @MainActor func widgetOnlyCloseDoesNotRewriteDocumentArtwork() async throws {
    _ = NSApplication.shared
    let root = try widgetCloseFixture()
    defer { try? FileManager.default.removeItem(at: root) }
    let folder = try Fixtures.folder()
    defer { try? FileManager.default.removeItem(at: folder) }
    let cache = WidgetCache(root: folder)
    SlopWidgetPublisher.shared.start(cache: cache) { _ in nil }
    defer { SlopWidgetPublisher.shared.stop() }
    let before = try Data(contentsOf: root)
    let modified = try root.resourceValues(forKeys: [.contentModificationDateKey]).contentModificationDate
    for edited in [false, false, true] {
      let previous = cache.record(for: root)?.generation
      let controller = try await SlopDocumentWindowController.open(url: root)
      controller.window?.moveOffScreen()
      controller.showWindow(nil)
      await controller.waitForPresentation()
      if edited { #expect(try await command("batch", url: root, setTitle("Changed widget")).ok) }
      try await controller.finishClose()
      let record = try #require(cache.record(for: root))
      if edited {
        #expect(record.generation != previous)
        break
      }
      if let previous { #expect(record.generation == previous, "unchanged close must not capture again") }
      #expect(try Data(contentsOf: root) == before)
      #expect(try root.resourceValues(forKeys: [.contentModificationDateKey]).contentModificationDate == modified)
    }
  }

  @Test(arguments: [false, true]) @MainActor
  func closeArtworkAndWidgetsKeepIndependentResults(brokenWidget: Bool) async throws {
    _ = NSApplication.shared
    let root = try widgetCloseFixture(brokenRenderer: brokenWidget)
    let folder = try Fixtures.folder()
    defer {
      try? FileManager.default.removeItem(at: root)
      try? FileManager.default.removeItem(at: folder)
    }
    let cache = WidgetCache(root: folder)
    let publisher = SlopWidgetPublisher.shared
    publisher.start(cache: cache) { _ in nil }
    defer { publisher.stop() }
    let session = try await DocumentSession.open(url: root)
    session.load()
    try await session.waitUntilReady()
    let artwork = await SlopRenderer.artwork(
      session: session, telemetry: SlopTelemetry { _ in }, widgetURL: root)
    #expect(artwork.preview != nil)
    #expect(artwork.icon != nil)
    #expect((cache.record(for: root) == nil) == brokenWidget)
    try await session.close(artwork: artwork)
    #expect(!Fixtures.isLocked(root))
    #expect(Fixtures.pixels(SlopArtwork.png(root, .preview)) == Fixtures.pixels(artwork.preview))
    #expect(Fixtures.pixels(SlopArtwork.png(root, .icon)) == Fixtures.pixels(artwork.icon))
  }

  private func widgetCloseFixture(brokenRenderer: Bool = false) throws -> URL {
    try contractFixture { stage in
      try Fixtures.addArtwork(stage, name: "preview", bytes: Fixtures.png())
      try Fixtures.updateApp(stage) {
        $0["views"] = ["export": true, "icon": true, "preview": true, "widget": [:]] as [String: Any]
      }
      try Fixtures.addWidget(stage)
      if brokenRenderer {
        try Data("globalThis.__slopWidget = () => { throw new Error('capture unavailable'); };".utf8)
          .write(to: stage.appendingPathComponent("assets/widget.js"))
      }
    }
  }

  @Test @MainActor func staleWidgetRendererTokensRefreshWithoutEditingAndFailuresKeepTheirImages() async throws {
    _ = NSApplication.shared
    for scenario in ["old renderer", "missing token", "failed refresh"] {
      let broken = scenario == "failed refresh"
      let root = try widgetCloseFixture(brokenRenderer: broken)
      let folder = try Fixtures.folder()
      defer {
        try? FileManager.default.removeItem(at: root)
        try? FileManager.default.removeItem(at: folder)
      }
      let cache = WidgetCache(root: folder)
      let png = try Fixtures.png()
      let images = WidgetImageFamily.allCases.map { WidgetImage(family: $0, png: png) }
      let first = try cache.publish(
        url: root, title: root.deletingPathExtension().lastPathComponent, images: images,
        capturedAt: Date(timeIntervalSince1970: 1),
        refreshMinutes: nil, sourceVersion: scenario == "missing token" ? nil : "prior-renderer")
      let saved = try Data(contentsOf: root)
      var reloads = 0
      let publisher = SlopWidgetPublisher.shared
      publisher.start(cache: cache, reload: { reloads += 1 }, openSession: { _ in nil })
      defer { publisher.stop() }
      await publisher.refresh(ids: [first.id], scheduledIDs: [])
      let current = try #require(cache.record(for: root))
      if broken {
        #expect(current.generation == first.generation)
        #expect(cache.imageData(current, .small) == images[0].png)
        #expect(reloads == 0, "a failed refresh must not consume a reload request")
      } else {
        #expect(current.generation != first.generation, "\(scenario) must refresh unchanged documents")
        #expect(current.sourceVersion == (try await SlopRenderer.widgetSourceVersion(url: root)))
        #expect(reloads == 1)
        await publisher.refresh(ids: [first.id], scheduledIDs: [])
        #expect(cache.record(for: root)?.generation == current.generation)
        #expect(reloads == 1, "unchanged renderer and document must skip duplicate capture")
      }
      #expect(try Data(contentsOf: root) == saved, "widget refresh must never save or migrate the document")
    }
  }

  // A rename or move changes the widget choice's label and location even when its
  // saved-state token is unchanged. Metadata maintenance must not need a working renderer.
  @Test(arguments: ["move", "rename", "failed capture"]) @MainActor
  func widgetMovesRefreshMetadataWithoutRecapturing(scenario: String) async throws {
    _ = NSApplication.shared
    let root = try widgetCloseFixture(brokenRenderer: true)
    let folder = try Fixtures.folder()
    defer {
      try? FileManager.default.removeItem(at: root)
      try? FileManager.default.removeItem(at: folder)
    }
    let sourceFolder = folder.appendingPathComponent("source", isDirectory: true)
    let destinationFolder = folder.appendingPathComponent("destination", isDirectory: true)
    try FileManager.default.createDirectory(at: sourceFolder, withIntermediateDirectories: true)
    try FileManager.default.createDirectory(at: destinationFolder, withIntermediateDirectories: true)
    let source = sourceFolder.appendingPathComponent("Before.slop")
    try FileManager.default.moveItem(at: root, to: source)
    let cache = WidgetCache(root: folder.appendingPathComponent("cache", isDirectory: true))
    let png = try Fixtures.png()
    let images = WidgetImageFamily.allCases.map { WidgetImage(family: $0, png: png) }
    let version =
      scenario == "failed capture" ? "prior-renderer" : try await SlopRenderer.widgetSourceVersion(url: source)
    let first = try cache.publish(
      url: source, title: "Before", images: images, capturedAt: Date(timeIntervalSince1970: 1),
      refreshMinutes: 15, sourceVersion: version)
    let saved = try Data(contentsOf: source)
    let title = scenario == "move" ? "Before" : "After"
    let destination = destinationFolder.appendingPathComponent(title + ".slop")
    try FileManager.default.moveItem(at: source, to: destination)
    try FileManager.default.removeItem(at: sourceFolder)
    #expect(try cache.resolve(first).standardizedFileURL == destination.standardizedFileURL)
    var reloads = 0
    let publisher = SlopWidgetPublisher.shared
    publisher.start(cache: cache, reload: { reloads += 1 }, openSession: { _ in nil })
    defer { publisher.stop() }
    await publisher.refresh(ids: [first.id], scheduledIDs: [])
    let current = try #require(cache.record(for: destination))
    #expect(current.title == title)
    #expect(current.id == first.id)
    #expect(current.generation == first.generation)
    #expect(current.updatedAt == first.updatedAt)
    #expect(current.sourceVersion == first.sourceVersion)
    #expect(current.refreshMinutes == first.refreshMinutes)
    for family in WidgetImageFamily.allCases { #expect(cache.imageData(current, family) == png) }
    #expect(reloads == 1, "location changes must reload the widget without capturing new pixels")
    await publisher.refresh(ids: [first.id], scheduledIDs: [])
    #expect(reloads == 1)
    #expect(try Data(contentsOf: destination) == saved)
    // Pruning must use the new parent, even after the original parent disappears.
    try FileManager.default.removeItem(at: destination)
    #expect(cache.pruneUnavailable())
    #expect(cache.record(id: first.id) == nil)
  }

  @Test @MainActor func aWidgetLocationFailureDoesNotSkipChangedSavedPixels() async throws {
    _ = NSApplication.shared
    let root = try widgetCloseFixture()
    let folder = try Fixtures.folder()
    defer {
      try? FileManager.default.removeItem(at: root)
      try? FileManager.default.removeItem(at: folder)
    }
    let cache = WidgetCache(root: folder)
    let capture = try await SlopRenderer.widgetCapture(url: root)
    let first = try cache.publish(
      url: root, title: "Old location", images: capture.images, capturedAt: Date(),
      refreshMinutes: nil, sourceVersion: capture.sourceVersion)
    let before = try #require(cache.imageData(first, .small))
    #expect(try await command("batch", url: root, setTitle("Widget saved")).ok)
    let publisher = SlopWidgetPublisher.shared
    let original = publisher.refreshLocation
    var failedLocation = false
    publisher.refreshLocation = { cache, id, url in
      let bookmark = folder.appendingPathComponent(id).appendingPathComponent("document.bookmark")
      let saved = try Data(contentsOf: bookmark)
      try FileManager.default.removeItem(at: bookmark)
      try FileManager.default.createDirectory(at: bookmark, withIntermediateDirectories: false)
      defer {
        try? FileManager.default.removeItem(at: bookmark)
        try? saved.write(to: bookmark)
      }
      do { return try cache.refreshLocation(id: id, url: url) } catch {
        failedLocation = true
        throw error
      }
    }
    publisher.start(cache: cache, openSession: { _ in nil })
    defer {
      publisher.stop()
      publisher.refreshLocation = original
    }
    await publisher.refresh(ids: [first.id], scheduledIDs: [])
    #expect(failedLocation)
    let current = try #require(cache.record(for: root))
    #expect(current.generation != first.generation)
    #expect(current.sourceVersion == (try await SlopRenderer.widgetSourceVersion(url: root)))
    #expect(Fixtures.pixels(cache.imageData(current, .small)) != Fixtures.pixels(before))
  }

  @Test @MainActor func savedEditsPublishNewWidgetPixelsAndRequestReloadWithoutClosing() async throws {
    _ = NSApplication.shared
    let root = try widgetCloseFixture()
    let folder = try Fixtures.folder()
    defer {
      try? FileManager.default.removeItem(at: root)
      try? FileManager.default.removeItem(at: folder)
    }
    let cache = WidgetCache(root: folder)
    var reloads = 0
    let publisher = SlopWidgetPublisher.shared
    publisher.start(cache: cache, reload: { reloads += 1 }, openSession: { _ in nil })
    defer { publisher.stop() }
    let controller = try await SlopDocumentWindowController.open(url: root)
    controller.window?.moveOffScreen()
    controller.showWindow(nil)
    await controller.waitForPresentation()
    #expect(await eventually(timeout: .seconds(10)) { cache.record(for: root) != nil })
    let first = try #require(cache.record(for: root))
    let oldImage = try #require(cache.imageData(first, .small))
    let previousReloads = reloads
    #expect(try await command("batch", url: root, setTitle("Widget saved")).ok)
    #expect(await eventually(timeout: .seconds(10)) { cache.record(for: root)?.generation != first.generation })
    let newer = try #require(cache.record(for: root))
    #expect(Fixtures.pixels(cache.imageData(newer, .small)) != Fixtures.pixels(oldImage))
    #expect(reloads > previousReloads)
    #expect(controller.session.isReady, "refresh must not close the editor")
    try await controller.finishClose()
  }

  @Test @MainActor func closedWidgetEditsRefreshAndRestartCatchesUpWithoutAnInterval() async throws {
    _ = NSApplication.shared
    let root = try widgetCloseFixture()
    let folder = try Fixtures.folder()
    defer {
      try? FileManager.default.removeItem(at: root)
      try? FileManager.default.removeItem(at: folder)
    }
    let cache = WidgetCache(root: folder)
    let publisher = SlopWidgetPublisher.shared
    var reloads = 0
    func start() { publisher.start(cache: cache, reload: { reloads += 1 }, openSession: { _ in nil }) }
    start()
    defer { publisher.stop() }
    let controller = try await SlopDocumentWindowController.open(url: root)
    controller.window?.moveOffScreen()
    controller.showWindow(nil)
    await controller.waitForPresentation()
    try await controller.finishClose()
    let first = try #require(cache.record(for: root))
    let blue = try #require(Fixtures.pixels(cache.imageData(first, .small)))
    let initialReloads = reloads
    #expect(!Fixtures.isLocked(root))
    #expect(try await command("batch", url: root, setTitle("Widget saved")).ok)
    #expect(
      await eventually(timeout: .seconds(10)) {
        guard let record = cache.record(for: root) else { return false }
        return Fixtures.pixels(cache.imageData(record, .small)) != blue
      }, "a closed-document CLI save must refresh without a declared interval or open window")
    #expect(reloads > initialReloads)
    let red = try #require(cache.record(for: root))
    publisher.stop()
    #expect(try await command("batch", url: root, setTitle("Changed while quit")).ok)
    let saved = try Data(contentsOf: root)
    #expect(cache.record(for: root)?.generation == red.generation, "quit retains the previous image")
    start()
    #expect(
      await eventually(timeout: .seconds(10)) {
        guard let record = cache.record(for: root), record.generation != red.generation else { return false }
        return Fixtures.pixels(cache.imageData(record, .small)) == blue
      }, "startup must catch up even when the widget has no refresh interval")
    #expect(try Data(contentsOf: root) == saved, "background refresh must never write the source")
    let caughtUp = try #require(cache.record(for: root))
    await publisher.refresh(ids: [caughtUp.id])
    #expect(cache.record(for: root)?.generation == caughtUp.generation, "unchanged state must not render again")
  }

  // Hold capture work until the window hands off. Ownership must remain with the
  // closing document, and Dock reveal must not resurrect the editor during that wait.
  @Test @MainActor func closeHandsOffBeforeArtworkAndKeepsOwnership() async throws {
    _ = NSApplication.shared
    for scenario in ["unchanged", "edited", "missing preview"] {
      let root = try contractFixture { stage in
        if scenario != "missing preview" {
          try Fixtures.addArtwork(stage, name: "preview", bytes: Fixtures.png())
        }
      }
      defer { try? FileManager.default.removeItem(at: root) }
      var handedOff: ContinuousClock.Instant?
      weak var observed: SlopDocumentWindowController?
      let controller = try await SlopDocumentWindowController.open(
        url: root,
        routing: SlopDocumentRouting(
          command: { _ in },
          closeHidden: {
            handedOff = .now
            #expect(observed?.window?.isVisible == false)
            #expect(Fixtures.isLocked(root), "handoff must retain ownership")
          }))
      controller.window?.moveOffScreen()
      observed = controller
      controller.showWindow(nil)
      await controller.waitForPresentation()
      weak var editor = controller.session.webView
      if scenario == "edited" {
        #expect(try await command("batch", url: root, setTitle("Saved before handoff")).ok)
      }
      let releaseCapture = DispatchSemaphore(value: 0)
      defer { releaseCapture.signal() }
      if scenario != "unchanged" {
        let held = Locked(false)
        SlopRenderer.captureQueue.async(flags: .barrier) {
          held.modify { $0 = true }
          releaseCapture.wait()
        }
        #expect(await eventually { held.value })
      }
      let started = ContinuousClock.now
      let finished = Locked(false)
      let closing = Task { @MainActor in
        _ = try await controller.perform(.close)
        finished.modify { $0 = true }
      }
      if scenario != "unchanged" {
        #expect(await eventually { handedOff != nil }, "window handoff must precede artwork completion")
        #expect(!finished.value)
        #expect(Fixtures.isLocked(root))
        controller.revealFromDock()
        // A delayed AppKit Dock restore orders the window directly, bypassing its controller.
        controller.window?.makeKeyAndOrderFront(nil)
        #expect(controller.window?.isVisible == false)
        if scenario == "edited" {
          #expect(try await savedValue(root)?["title"] as? String == "Saved before handoff")
        }
        releaseCapture.signal()
      }
      try await closing.value
      #expect(handedOff != nil)
      let completed = ContinuousClock.now
      print(
        "Close timing [\(scenario)]: handoff=\(started.duration(to: handedOff ?? completed)), total=\(started.duration(to: completed))"
      )
      #expect(!Fixtures.isLocked(root))
      #expect(await eventually(timeout: .seconds(5)) { editor == nil })
      #expect(SlopArtwork.png(root, .preview) != nil)
    }
  }
}
