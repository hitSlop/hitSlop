import AppKit
import Foundation
import HitSlopCore
import HitSlopTestSupport
import HitSlopWidgets
import Testing
import WebKit

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

  private func widgetCloseFixture(brokenRenderer: Bool = false) throws -> URL {
    try contractFixture { stage in
      try Fixtures.addArtwork(stage, name: "preview", bytes: Fixtures.png())
      try Fixtures.updateApp(stage) { $0["views"] = ["export": false, "icon": false, "widget": [:]] as [String: Any] }
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
        url: root, title: "Previous renderer", images: images, capturedAt: Date(timeIntervalSince1970: 1),
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

  // The capture page waits for this test to release it. The window must hand off before
  // that wait ends, while retaining its owner; Dock reveal must not resurrect the editor.
  @Test @MainActor func closeHandsOffBeforeArtworkAndKeepsOwnership() async throws {
    _ = NSApplication.shared
    for scenario in ["unchanged", "edited", "missing preview"] {
      let root = try contractFixture { stage in
        if scenario != "missing preview" {
          try Fixtures.addArtwork(stage, name: "preview", bytes: Fixtures.png())
        }
        let app = stage.appendingPathComponent("assets/ui.js")
        let script = try String(contentsOf: app, encoding: .utf8).replacingOccurrences(
          of: "const doc = ctx.document;",
          with: """
            const doc = ctx.document;
            let held = false;
            globalThis.closeArtworkScenario = '\(scenario)';
            ctx.capture.onPrepare(() => {
              if (!ctx.capture.isRenderer() || held) return;
              held = true;
              return new Promise(resolve => { globalThis.releaseCloseArtwork = resolve; });
            });
            """)
        try Data(script.utf8).write(to: app)
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
      let started = ContinuousClock.now
      let finished = Locked(false)
      let closing = Task { @MainActor in
        _ = try await controller.perform(.close)
        finished.modify { $0 = true }
      }
      if scenario != "unchanged" {
        var capturePage: WKWebView?
        let waiting = await eventually(timeout: .seconds(5)) {
          for window in NSApp.windows {
            guard let view = window.contentView as? WKWebView else { continue }
            if (try? await view.evaluateJavaScript(
              "globalThis.closeArtworkScenario === '\(scenario)' && typeof globalThis.releaseCloseArtwork === 'function'"
            )) as? Bool
              == true
            {
              capturePage = view
              return true
            }
          }
          return false
        }
        #expect(waiting, "artwork must be held by the capture page")
        #expect(handedOff != nil, "the browser handoff must precede artwork completion")
        #expect(!finished.value)
        #expect(Fixtures.isLocked(root))
        controller.revealFromDock()
        // A delayed AppKit Dock restore orders the window directly, bypassing its controller.
        controller.window?.makeKeyAndOrderFront(nil)
        #expect(controller.window?.isVisible == false)
        if scenario == "edited" {
          #expect(try await savedValue(root)?["title"] as? String == "Saved before handoff")
        }
        _ = try await capturePage?.evaluateJavaScript("globalThis.releaseCloseArtwork(); true")
        capturePage = nil
      }
      try await closing.value
      #expect(handedOff != nil)
      let completed = ContinuousClock.now
      print(
        "Close timing [\(scenario)]: handoff=\(started.duration(to: handedOff ?? completed)), total=\(started.duration(to: completed))"
      )
      #expect(!Fixtures.isLocked(root))
      #expect(editor == nil)
      #expect(SlopArtwork.png(root, .preview) != nil)
    }
  }
}
