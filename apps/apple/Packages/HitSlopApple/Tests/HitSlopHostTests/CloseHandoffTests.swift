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

  private func widgetCloseFixture(hold: Bool = false) throws -> URL {
    return try contractFixture { stage in
      try Fixtures.addArtwork(stage, name: "preview", bytes: Fixtures.png())
      try Fixtures.updateApp(stage) { $0["views"] = ["export": false, "icon": false, "widget": [:]] as [String: Any] }
      let app = stage.appendingPathComponent("assets/ui.js")
      let script = try String(contentsOf: app, encoding: .utf8).replacingOccurrences(
        of: "const doc = ctx.document;",
        with: """
          const doc = ctx.document;
          let held = false;
          ctx.capture.onPrepare(mode => {
            if (!\(hold ? "true" : "false") || mode !== 'widget' || held) return;
            held = true;
            return new Promise(resolve => { globalThis.releaseScheduledWidget = resolve; });
          });
          const widget = document.createElement('div');
          widget.style.cssText = 'width:100vw;height:100vh;background:red';
          widget.hidden = true; document.body.append(widget);
          ctx.capture.registerTarget('widget', { element: widget,
            prepare() { widget.hidden = false; }, restore() { widget.hidden = true; } });
          """)
      try Data(script.utf8).write(to: app)
    }
  }

  @Test @MainActor func widgetCloseDoesNotWaitForScheduledCapture() async throws {
    _ = NSApplication.shared
    let scheduledURL = try widgetCloseFixture(hold: true)
    let closingURL = try widgetCloseFixture()
    let folder = try Fixtures.folder()
    defer {
      try? FileManager.default.removeItem(at: scheduledURL)
      try? FileManager.default.removeItem(at: closingURL)
      try? FileManager.default.removeItem(at: folder)
    }
    let cache = WidgetCache(root: folder)
    let png = try Fixtures.png()
    let record = try cache.publish(
      url: scheduledURL, title: "Scheduled",
      images: WidgetImageFamily.allCases.map { WidgetImage(family: $0, png: png) },
      capturedAt: Date().addingTimeInterval(-3600), refreshMinutes: 15)
    let publisher = SlopWidgetPublisher.shared
    publisher.start(cache: cache) { _ in nil }
    defer { publisher.stop() }
    let scheduled = Task { @MainActor in await publisher.refresh(ids: [record.id]) }
    var capturePage: WKWebView?
    let waiting = await eventually(timeout: .seconds(5)) {
      for window in NSApp.windows {
        guard let view = window.contentView as? WKWebView else { continue }
        if (try? await view.evaluateJavaScript("typeof globalThis.releaseScheduledWidget === 'function'")) as? Bool
          == true
        {
          capturePage = view
          return true
        }
      }
      return false
    }
    #expect(waiting)
    let controller = try await SlopDocumentWindowController.open(url: closingURL)
    controller.window?.moveOffScreen()
    controller.showWindow(nil)
    await controller.waitForPresentation()
    let finished = Locked(false)
    let closing = Task { @MainActor in
      try await controller.finishClose()
      finished.modify { $0 = true }
    }
    let independent = await eventually(timeout: .seconds(3)) { finished.value }
    #expect(independent, "close must finish while an unrelated scheduled renderer is still held")
    _ = try await capturePage?.evaluateJavaScript("globalThis.releaseScheduledWidget(); true")
    await scheduled.value
    try await closing.value
    #expect(cache.record(for: closingURL) != nil)
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
