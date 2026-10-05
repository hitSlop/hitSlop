import AppKit
import Foundation
import HitSlopDocument
import HitSlopCore
import HitSlopCoreBinding
import Testing
import HitSlopTestSupport
@testable import HitSlopHost

extension HostTests {
  @Test(.enabled(if: ProcessInfo.processInfo.environment["HITSLOP_STARTUP_BENCH"] == "1"))
  @MainActor func documentStartupTimings() async throws {
    _ = NSApplication.shared
    for name in ["quick-checklist"] {
      for sample in 0..<3 {
        let root = try fixture(name)
        defer { try? FileManager.default.removeItem(at: root) }
        let start = ContinuousClock.now
        let controller = try await SlopDocumentWindowController.open(url: root)
        let prepared = start.duration(to: .now)
        controller.showWindow(nil)
        try await controller.session.waitUntilReady()
        let ready = start.duration(to: .now)
        await controller.waitForPresentation()
        let visible = start.duration(to: .now)
        print("[startup benchmark] \(name) sample=\(sample) prepared=\(prepared) ready=\(ready) visible=\(visible)")
        // Keep this benchmark focused on opening, without background preview refreshes.
        try await controller.session.close()
        controller.close()
      }
    }
  }

  @Test(.enabled(if: ProcessInfo.processInfo.environment["HITSLOP_STARTUP_BENCH"] == "1"))
  @MainActor func savedDocumentStartupTimings() async throws {
    _ = NSApplication.shared
    let environment = ProcessInfo.processInfo.environment
    if environment["HITSLOP_STARTUP_FOREGROUND"] == "1" {
      NSApp.activate(ignoringOtherApps: true)
    }
    let samples = max(1, Int(environment["HITSLOP_STARTUP_SAMPLES"] ?? "10") ?? 10)
    var names = ["quick-checklist", "large-checklist"]
    var skinSource: String?
    if let fixtures = environment["HITSLOP_PRESENTATION_FIXTURES"] {
      skinSource = try JSONDecoder().decode([String: String].self, from: Data(fixtures.utf8))["washer"]
      if skinSource != nil { names.append("washer") }
    }
    if let name = environment["HITSLOP_STARTUP_CASE"] { names = names.filter { $0 == name } }
    #expect(!names.isEmpty)
    for name in names {
      let root: URL
      if name == "washer", let skinSource {
        root = try Fixtures.document(from: URL(fileURLWithPath: skinSource))
      } else { root = try fixture(name == "large-checklist" ? "quick-checklist" : name) }
      defer { try? FileManager.default.removeItem(at: root) }
      var operations: [[String: Any]] = name == "washer"
        ? [["type": "increment", "path": ["count"], "by": 7]]
        : [["type": "set", "path": ["title"], "value": "Saved opening benchmark"]]
      if name == "large-checklist" {
        operations += (0..<1000).map { index in
          ["type": "insert", "path": ["tasks"],
           "value": ["text": "Saved task \(index)", "done": false, "archived": false]]
        }
      }
      // Seed in a separate helper process so preparing saved bytes cannot warm this WebKit.
      let json = String(decoding: try JSONSerialization.data(withJSONObject: operations), as: UTF8.self)
      let seeded = try await cli(["batch", root.path, "--ops", json])
      try #require(seeded.0 == 0, "\(seeded.2)")
      for sample in 0..<samples {
        let start = ContinuousClock.now
        let controller = try await SlopDocumentWindowController.open(url: root, presentsWindow: true)
        let progress = controller.openingProgress
        let prepared = start.duration(to: .now)
        try await controller.session.waitUntilReady()
        let ready = start.duration(to: .now)
        await controller.waitForPresentation()
        let visible = start.duration(to: .now)
        #expect(controller.isContentReady)
        print("[saved startup benchmark] \(name) sample=\(sample) prepared=\(prepared) ready=\(ready) visible=\(visible) progress=\(progress?.wasShown ?? false)")
        try await controller.session.close()
        _ = try await controller.perform(.close)
      }
    }
  }

  @Test @MainActor func openingStaysHiddenUntilReadyAndHandsOffFocus() async throws {
    _ = NSApplication.shared
    let before = try Fixtures.png()
    let root = try contractFixture { stage in
      try FileManager.default.createDirectory(at: stage.appendingPathComponent("artwork"), withIntermediateDirectories: true)
      try before.write(to: stage.appendingPathComponent("artwork/preview.png"))
    }
    defer { try? FileManager.default.removeItem(at: root) }
    let controller = try await SlopDocumentWindowController.open(url: root)
    controller.showWindow(nil)
    #expect(controller.isLoading)
    #expect(controller.window?.isVisible == false)
    controller.revealFromDock()
    #expect(controller.window?.isVisible == false)
    await controller.waitForPresentation()
    #expect(controller.session.isReady)
    #expect(controller.openingProgress == nil)
    #expect(controller.window?.isVisible == true)
    #expect(controller.isContentReady)
    #expect(SlopArtwork.png(root, .preview) == before, "opening never rewrites artwork")
    try await controller.session.close()
    controller.close()
  }

  @Test @MainActor func previewIsNotNeededAndCloseCancelsHiddenOpening() async throws {
    _ = NSApplication.shared
    do {
      let root = try contractFixture()
      defer { try? FileManager.default.removeItem(at: root) }
      #expect(SlopArtwork.png(root, .preview) == nil)
      let controller = try await SlopDocumentWindowController.open(url: root)
      controller.showWindow(nil)
      #expect(controller.window?.isVisible == false)
      weak var webView = controller.session.webView
      _ = try await controller.perform(.close)
      await controller.waitForPresentation()
      #expect(controller.openingProgress == nil)
      #expect(controller.window?.isVisible == false)
      #expect(!controller.session.isReady)
      // WebKit can finish asynchronous cancellation on the next run-loop turn.
      await eventually(timeout: .seconds(1)) { webView == nil }
      #expect(webView == nil)
    }
  }

  // An app's own startup failure is shown natively; a retry once its cause is gone (here,
  // the document's title) opens it.
  @Test @MainActor func startupFailureRevealsNativeError() async throws {
    _ = NSApplication.shared
    let root = try contractFixture { stage in
      let assets = stage.appendingPathComponent("assets")
      try FileManager.default.moveItem(at: assets.appendingPathComponent("app.js"), to: assets.appendingPathComponent("probe.js"))
      try Data("""
        import probe from "./probe.js";
        export default { async mount(ctx, target) {
          if (ctx.document.current.title !== "Recovered") {
            await webkit.messageHandlers.hitslop.postMessage({method: "failed", error: "startup fixture failure"});
            await new Promise(() => {});
          }
          return probe.mount(ctx, target);
        } };
        """.utf8).write(to: assets.appendingPathComponent("app.js"))
    }
    defer { try? FileManager.default.removeItem(at: root) }
    let controller = try await SlopDocumentWindowController.open(url: root)
    controller.showWindow(nil)
    await controller.waitForPresentation()
    #expect(controller.openingProgress == nil)
    #expect(controller.window?.isVisible == true)
    #expect(controller.presentedPageError?.contains("startup fixture failure") == true)
    #expect(try await command("batch", url: root, setTitle("Recovered")).ok)
    _ = try await controller.perform(.retry)
    await controller.waitForPresentation()
    #expect(controller.isContentReady)
    #expect(controller.presentedPageError == nil)
    #expect(controller.window?.isVisible == true)
    try await controller.session.close()
    controller.close()
  }

  @Test @MainActor func hiddenOpenDoesNotPresentProgressOrDocument() async throws {
    _ = NSApplication.shared
    let root = try contractFixture()
    defer { try? FileManager.default.removeItem(at: root) }
    let controller = try await SlopDocumentWindowController.open(url: root)
    await controller.waitForPresentation()
    #expect(controller.isContentReady)
    #expect(controller.window?.isVisible == false)
    #expect(controller.openingProgress == nil)
    try await controller.session.close()
    controller.close()
  }

  @Test @MainActor func progressCancelClosesPendingDocumentWithoutRevealingIt() async throws {
    _ = NSApplication.shared
    // Fonts never finish loading, so the page never reports ready.
    let root = try contractFixture(edit: neverLoadFonts)
    defer { try? FileManager.default.removeItem(at: root) }
    // Fonts never load, so the page never reports ready and the window stays loading.
    let controller = try await SlopDocumentWindowController.open(url: root, presentsWindow: true)
    let progress = try #require(controller.openingProgress)
    progress.cancelOpening()
    await eventually(timeout: .seconds(2)) { !controller.isLoading }
    #expect(!controller.isLoading)
    #expect(!controller.isContentReady)
    #expect(controller.window?.isVisible == false)
    #expect(controller.openingProgress == nil)
    #expect(FileManager.default.fileExists(atPath: root.path))
    // A cancelled document must release ownership, not merely hide its window.
    let reopened = try await SlopDocumentWindowController.open(url: root)
    _ = try await reopened.perform(.close)
  }

  @Test @MainActor func closeDuringFontLoadingCancelsPresentationAndExport() async throws {
    _ = NSApplication.shared
    let root = try contractFixture(edit: neverLoadFonts)
    let output = root.deletingLastPathComponent().appendingPathComponent(UUID().uuidString + ".png")
    defer {
      try? FileManager.default.removeItem(at: root)
      try? FileManager.default.removeItem(at: output)
    }
    let controller = try await SlopDocumentWindowController.open(url: root)
    controller.showWindow(nil)
    await eventually(timeout: .seconds(2)) { controller.openingProgress?.panel != nil }
    #expect(controller.openingProgress?.panel?.isVisible == true)
    #expect(controller.window?.isVisible == false)
    weak var view = controller.session.webView
    let export = Task { try await controller.exportDocument(format: .png, to: output) }
    await Task.yield()
    _ = try await controller.perform(.close)
    await #expect(throws: (any Error).self) { try await export.value }
    #expect(!FileManager.default.fileExists(atPath: output.path))
    #expect(controller.openingProgress == nil)
    await eventually(timeout: .seconds(1)) { view == nil }
    #expect(view == nil)
  }
}

// Completing the delay releases feedback; completing the open cancels it even if
// an already-delivered timer callback arrives. No minimum wall-clock duration.
@Test @MainActor func openingProgressIsDelayedAndFastCompletionNeverShowsIt() async throws {
  _ = NSApplication.shared
  let fastDelay = OpeningDelay()
  let fast = SlopOpeningProgress(wait: { _ in await fastDelay.wait() })
  await fastDelay.started()
  #expect(fast.panel == nil)
  fast.finish()
  await fastDelay.release()
  let slowDelay = OpeningDelay()
  let slow = SlopOpeningProgress(wait: { _ in await slowDelay.wait() })
  await slowDelay.started()
  #expect(slow.panel == nil)
  await slowDelay.release()
  await slow.waitForFeedback()
  #expect(fast.panel == nil)
  #expect(slow.panel?.isVisible == true)
  var cancelled = false
  slow.onCancel = { cancelled = true }
  slow.cancelOpening()
  #expect(cancelled)
  #expect(slow.panel == nil)
}

private actor OpeningDelay {
  private var continuation: CheckedContinuation<Void, Never>?
  private var entered: CheckedContinuation<Void, Never>?
  func wait() async {
    await withCheckedContinuation { continuation = $0; entered?.resume(); entered = nil }
  }
  func started() async {
    if continuation != nil { return }
    await withCheckedContinuation { entered = $0 }
  }
  func release() { continuation?.resume(); continuation = nil }
}

extension HostTests {
  // Failure: close awaited the app's unmount without a deadline, so a teardown that never
  // settled blocked closing and render-session exports after the document was released.
  @Test @MainActor func aHungUnmountDoesNotBlockCloseOrExport() async throws {
    let root = try contractFixture { stage in
      try Data(#"""
        export default { mount() { return { unmount: () => new Promise(r => setTimeout(r, 3_600_000)) }; } };
        """#.utf8).write(to: stage.appendingPathComponent("assets/app.js"))
    }
    defer { try? FileManager.default.removeItem(at: root) }
    // The timer keeps the promise reachable: WebKit rejects calls on unreachable ones.
    // Polls instead of awaiting, so a regression fails at the deadline rather than hanging.
    func finishes(within limit: Duration, _ work: @escaping @MainActor () async throws -> Void) async throws -> Bool {
      let finished = Locked(false)
      let task = Task { @MainActor in try await work(); finished.modify { $0 = true } }
      guard await eventually(timeout: limit, { finished.value }) else { return false }
      try await task.value
      return true
    }
    let controller = try await SlopDocumentWindowController.open(url: root)
    try await controller.session.waitUntilReady()
    weak var page = controller.session.webView
    #expect(try await finishes(within: .seconds(6)) { try await controller.session.close() })
    controller.close()
    // An abandoned page is released with its view, ending its scripts.
    #expect(await eventually(timeout: .seconds(3)) { page == nil })
    #expect(try await command("get", url: root).ok)
    #expect(try await finishes(within: .seconds(10)) {
      _ = try await SlopRenderer.withRenderSession(url: root) { try await SlopRenderer.exportPNGData(session: $0) }
    })
  }

  // A subscriber failure must reach native reporting without interrupting the accepted edit.
  @Test @MainActor func observerFailureIsReportedWithoutPreventingDurability() async throws {
    let root = try contractFixture { stage in
      try Data(#"""
        export default { mount(ctx) {
          ctx.document.subscribe(() => { throw new Error('observer failure'); });
          globalThis.observerProbe = async () => {
            ctx.document.change(tx => tx.fields.title.set('Saved despite observer failure'));
            await ctx.document.flush();
            return true;
          };
          return {};
        } };
        """#.utf8).write(to: stage.appendingPathComponent("assets/app.js"))
    }
    defer { try? FileManager.default.removeItem(at: root) }
    let (incidents, continuation) = AsyncStream<SlopFailureContext>.makeStream()
    defer { continuation.finish() }
    let controller = try await SlopDocumentWindowController.open(url: root,
      telemetry: SlopTelemetry { if case .failed(_, let context) = $0 { continuation.yield(context) } })
    try await controller.session.waitUntilReady()
    let flushed = try await controller.session.webView.callAsyncJavaScript(
      "return await globalThis.observerProbe()", arguments: [:], in: nil, contentWorld: .page)
    #expect(flushed as? Bool == true)
    var iterator = incidents.makeAsyncIterator()
    let incident = await iterator.next()
    #expect(incident?.classification == .authored)
    // WebKit's stack lists frames only; the person must still see what went wrong.
    #expect(controller.guestIssue?.message.hasPrefix("Error: observer failure\n") == true)
    try await controller.session.close()
    #expect(try await savedValue(root)?["title"] as? String == "Saved despite observer failure")
  }

  // Gap: installing telemetry only after open returns loses early guest startup failures.
  // A disposable fixture throws before mounting; expect one sanitized authored incident.
  @Test @MainActor func startupTelemetryIsInstalledBeforeAuthoredCodeRuns() async throws {
    let root = try contractFixture { stage in
      try Data("throw new Error('private startup contents');".utf8).write(to: stage.appendingPathComponent("assets/app.js"))
    }
    defer { try? FileManager.default.removeItem(at: root) }
    var failures: [SlopFailureContext] = []
    let controller = try await SlopDocumentWindowController.open(url: root,
      telemetry: SlopTelemetry { if case .failed(_, let context) = $0 { failures.append(context) } })
    await controller.waitForPresentation()
    #expect(!controller.isContentReady)
    #expect(failures.count == 1)
    #expect(failures.first?.classification == .authored)
    #expect(failures.first?.reason == .authoredException)
    try await controller.session.close()
  }
}

/// Makes a stage's app wait forever for its fonts, so its page never reports ready.
private func neverLoadFonts(_ stage: URL) throws {
  let script = stage.appendingPathComponent("assets/app.js")
  let original = try String(contentsOf: script, encoding: .utf8)
  try Data(("Object.defineProperty(document,'fonts',{value:{size:1,status:'loading',forEach(){},ready:new Promise(()=>{})}});\n" + original).utf8).write(to: script)
}
