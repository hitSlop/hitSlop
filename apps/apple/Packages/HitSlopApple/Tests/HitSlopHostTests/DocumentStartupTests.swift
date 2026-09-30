import AppKit
import Foundation
import HitSlopDocument
import HitSlopCore
import Testing
import HitSlopTestSupport
@testable import HitSlopHost

extension OwnerClientTests {
  @Test(.enabled(if: ProcessInfo.processInfo.environment["HITSLOP_STARTUP_BENCH"] == "1"))
  @MainActor func documentStartupTimings() async throws {
    _ = NSApplication.shared
    for name in ["quick-checklist"] {
      for sample in 0..<3 {
        let root = try fixture(name)
        defer { try? FileManager.default.removeItem(at: root) }
        let start = ContinuousClock.now
        let controller = try await SlopDocumentWindowController.open(packageURL: root)
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
        root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString + ".slop")
        try FileManager.default.copyItem(atPath: skinSource, toPath: root.path)
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
        let controller = try await SlopDocumentWindowController.open(packageURL: root, presentsWindow: true)
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
    let root = try contractFixture()
    defer { try? FileManager.default.removeItem(at: root) }
    let previewURL = root.appendingPathComponent("QuickLook/Preview.png")
    try FileManager.default.createDirectory(at: previewURL.deletingLastPathComponent(), withIntermediateDirectories: true)
    let before = Data("optional preview is untouched during open".utf8)
    try before.write(to: previewURL)
    let controller = try await SlopDocumentWindowController.open(packageURL: root)
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
    #expect(try Data(contentsOf: previewURL) == before)
    try await controller.session.close()
    controller.close()
  }

  @Test @MainActor func previewIsNotNeededAndCloseCancelsHiddenOpening() async throws {
    _ = NSApplication.shared
    for invalid in [false, true] {
      let root = try contractFixture()
      defer { try? FileManager.default.removeItem(at: root) }
      let preview = root.appendingPathComponent("QuickLook/Preview.png")
      if invalid {
        try FileManager.default.createDirectory(at: preview.deletingLastPathComponent(), withIntermediateDirectories: true)
        try Data("not an image".utf8).write(to: preview)
      }
      let controller = try await SlopDocumentWindowController.open(packageURL: root)
      controller.showWindow(nil)
      #expect(controller.window?.isVisible == false)
      weak var webView = controller.session.webView
      _ = try await controller.perform(.close)
      await controller.waitForPresentation()
      #expect(controller.openingProgress == nil)
      #expect(controller.window?.isVisible == false)
      #expect(!controller.session.isReady)
      // WebKit can finish asynchronous cancellation on the next run-loop turn.
      for _ in 0..<100 where webView != nil { try await Task.sleep(for: .milliseconds(10)) }
      #expect(webView == nil)
    }
  }

  @Test @MainActor func startupFailureRevealsNativeError() async throws {
    _ = NSApplication.shared
    let root = try contractFixture()
    defer { try? FileManager.default.removeItem(at: root) }
    let script = root.appendingPathComponent("assets/app.js")
    let original = try Data(contentsOf: script)
    try Data("await webkit.messageHandlers.hitslop.postMessage({method:'failed',error:'startup fixture failure'});".utf8)
      .write(to: root.appendingPathComponent("assets/app.js"))
    let controller = try await SlopDocumentWindowController.open(packageURL: root)
    var failures: [String] = []
    controller.showWindow(nil)
    controller.onPageFailure = {
      failures.append($0)
      controller.updatePresentation(pinned: false, commandsEnabled: true, pageError: $0)
    }
    await controller.waitForPresentation()
    #expect(controller.openingProgress == nil)
    #expect(controller.window?.isVisible == true)
    #expect(failures.count == 1)
    #expect(failures.first?.contains("startup fixture failure") == true)
    controller.onPageFailure = nil
    try original.write(to: script)
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
    let controller = try await SlopDocumentWindowController.open(packageURL: root)
    await controller.waitForPresentation()
    #expect(controller.isContentReady)
    #expect(controller.window?.isVisible == false)
    #expect(controller.openingProgress == nil)
    try await controller.session.close()
    controller.close()
  }

  @Test @MainActor func progressCancelClosesPendingDocumentWithoutRevealingIt() async throws {
    _ = NSApplication.shared
    let root = try contractFixture()
    defer { try? FileManager.default.removeItem(at: root) }
    let script = root.appendingPathComponent("assets/app.js")
    let original = try String(contentsOf: script, encoding: .utf8)
    try Data(("Object.defineProperty(document,'fonts',{value:{size:1,status:'loading',forEach(){},ready:new Promise(()=>{})}});\n" + original).utf8).write(to: script)
    // Fonts never load, so the page never reports ready and the window stays loading.
    let controller = try await SlopDocumentWindowController.open(packageURL: root, presentsWindow: true)
    let progress = try #require(controller.openingProgress)
    progress.cancelOpening()
    for _ in 0..<200 where controller.isLoading { try await Task.sleep(for: .milliseconds(10)) }
    #expect(!controller.isLoading)
    #expect(!controller.isContentReady)
    #expect(controller.window?.isVisible == false)
    #expect(controller.openingProgress == nil)
    #expect(FileManager.default.fileExists(atPath: root.path))
    // A cancelled document must release ownership, not merely hide its window.
    let reopened = try await SlopDocumentWindowController.open(packageURL: root)
    _ = try await reopened.perform(.close)
  }

  @Test @MainActor func closeDuringFontLoadingCancelsPresentationAndExport() async throws {
    _ = NSApplication.shared
    let root = try contractFixture()
    let output = root.deletingLastPathComponent().appendingPathComponent(UUID().uuidString + ".png")
    defer {
      try? FileManager.default.removeItem(at: root)
      try? FileManager.default.removeItem(at: output)
    }
    let script = root.appendingPathComponent("assets/app.js")
    let original = try String(contentsOf: script, encoding: .utf8)
    try Data(("Object.defineProperty(document,'fonts',{value:{size:1,status:'loading',forEach(){},ready:new Promise(()=>{})}});\n" + original).utf8).write(to: script)
    let controller = try await SlopDocumentWindowController.open(packageURL: root)
    controller.showWindow(nil)
    for _ in 0..<200 where controller.openingProgress?.panel == nil {
      try await Task.sleep(for: .milliseconds(10))
    }
    #expect(controller.openingProgress?.panel?.isVisible == true)
    #expect(controller.window?.isVisible == false)
    weak var view = controller.session.webView
    let export = Task { try await controller.exportDocument(format: .png, to: output) }
    await Task.yield()
    _ = try await controller.perform(.close)
    await #expect(throws: (any Error).self) { try await export.value }
    #expect(!FileManager.default.fileExists(atPath: output.path))
    #expect(controller.openingProgress == nil)
    for _ in 0..<100 where view != nil { try await Task.sleep(for: .milliseconds(10)) }
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

extension OwnerClientTests {
  // Failure: close awaited the app's unmount without a deadline, so a teardown that never
  // settled blocked closing and render-session exports after the document was released.
  @Test @MainActor func aHungUnmountDoesNotBlockCloseOrExport() async throws {
    let root = try contractFixture()
    defer { try? FileManager.default.removeItem(at: root) }
    try Data(#"""
      export default { mount() { return { unmount: () => new Promise(r => setTimeout(r, 3_600_000)) }; } };
      """#.utf8).write(to: root.appendingPathComponent("assets/app.js"))
    // The timer keeps the promise reachable: WebKit rejects calls on unreachable ones.
    // Polls instead of awaiting, so a regression fails at the deadline rather than hanging.
    func finishes(within limit: Duration, _ work: @escaping @MainActor () async throws -> Void) async throws -> Bool {
      let finished = Locked(false)
      let task = Task { @MainActor in try await work(); finished.modify { $0 = true } }
      let end = ContinuousClock.now.advanced(by: limit)
      while !finished.value, ContinuousClock.now < end { try await Task.sleep(for: .milliseconds(50)) }
      if finished.value { try await task.value }
      return finished.value
    }
    let controller = try await SlopDocumentWindowController.open(packageURL: root)
    try await controller.session.waitUntilReady()
    weak var page = controller.session.webView
    #expect(try await finishes(within: .seconds(6)) { try await controller.session.close() })
    controller.close()
    // An abandoned page is released with its view, ending its scripts.
    #expect(try await finishes(within: .seconds(3)) { while page != nil { try await Task.sleep(for: .milliseconds(50)) } })
    _ = try await command("get", url: root)
    #expect(try await finishes(within: .seconds(10)) { _ = try await SlopRenderer.exportPNGData(packageURL: root) })
  }

  // A subscriber failure must reach native reporting without interrupting the accepted edit.
  @Test @MainActor func observerFailureIsReportedWithoutPreventingDurability() async throws {
    let root = try contractFixture()
    defer { try? FileManager.default.removeItem(at: root) }
    try Data(#"""
      export default { mount(ctx) {
        ctx.document.subscribe(() => { throw new Error('observer failure'); });
        globalThis.observerProbe = async () => {
          ctx.document.change(tx => tx.fields.title.set('Saved despite observer failure'));
          await ctx.document.flush();
          return ctx.document.status;
        };
        return {};
      } };
      """#.utf8).write(to: root.appendingPathComponent("assets/app.js"))
    let (incidents, continuation) = AsyncStream<SlopFailureContext>.makeStream()
    defer { continuation.finish() }
    let controller = try await SlopDocumentWindowController.open(packageURL: root,
      telemetry: SlopTelemetry { if case .failed(_, let context) = $0 { continuation.yield(context) } })
    try await controller.session.waitUntilReady()
    let status = try await controller.session.webView.callAsyncJavaScript(
      "return await globalThis.observerProbe()", arguments: [:], in: nil, contentWorld: .page)
    #expect(status as? String == "saved")
    var iterator = incidents.makeAsyncIterator()
    let incident = await iterator.next()
    #expect(incident?.classification == .authored)
    // WebKit's stack lists frames only; the person must still see what went wrong.
    #expect(controller.guestIssue?.message.hasPrefix("Error: observer failure\n") == true)
    try await controller.session.close()
    let saved = try await command("get", url: root)
    #expect(String(decoding: saved, as: UTF8.self).contains("Saved despite observer failure"))
  }

  // Gap: installing telemetry only after open returns loses early guest startup failures.
  // A disposable fixture throws before mounting; expect one sanitized authored incident.
  @Test @MainActor func startupTelemetryIsInstalledBeforeAuthoredCodeRuns() async throws {
    let root = try contractFixture()
    defer { try? FileManager.default.removeItem(at: root) }
    try Data("throw new Error('private startup contents');".utf8)
      .write(to: root.appendingPathComponent("assets/app.js"))
    var failures: [SlopFailureContext] = []
    let controller = try await SlopDocumentWindowController.open(packageURL: root,
      telemetry: SlopTelemetry { if case .failed(_, let context) = $0 { failures.append(context) } })
    await controller.waitForPresentation()
    #expect(!controller.isContentReady)
    #expect(failures.count == 1)
    #expect(failures.first?.classification == .authored)
    #expect(failures.first?.reason == .authoredException)
    try await controller.session.close()
  }
}
