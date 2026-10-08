import AppKit
import Foundation
import HitSlopCore
import HitSlopTestSupport
import Testing
import WebKit

@testable import HitSlopDocument
@testable import HitSlopHost

extension HostTests {
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
