import AppKit
import Foundation
import HitSlopCore
import HitSlopCoreBinding
import HitSlopTestSupport
import PDFKit
import Testing

@testable import HitSlopDocument
@testable import HitSlopHost

extension HostTests {
  @Test @MainActor func themeOverridesSurviveReloadDuplicateAndClosedEditing() async throws {
    _ = NSApplication.shared
    let root = try contractFixture()
    defer { try? FileManager.default.removeItem(at: root) }
    let controller = try await SlopDocumentWindowController.open(url: root)
    try await controller.session.waitUntilReady()
    #expect(try await command("batch", url: root, setTitle("Preserved through interface reload")).ok)
    _ = try await controller.session.webView.callAsyncJavaScript(
      "dispatchEvent(new ErrorEvent('error', {error:new Error('Test application failure')})); return true",
      arguments: [:], in: nil, contentWorld: .page)
    // An issue that leaves the slop running shows as the badge, not a blocking sheet,
    // and a CLI edit leaves it until the person dismisses it.
    await eventually(timeout: .seconds(1)) { controller.issueBadge != nil }
    let badge = try #require(controller.issueBadge)
    #expect(controller.window?.attachedSheet == nil)
    #expect(try await command("batch", url: root, setTitle("Preserved through interface reload")).ok)
    #expect(controller.issueBadge === badge)
    controller.dismissIssue()
    #expect(controller.issueBadge == nil)
    let baseline = try await effectiveTheme(url: root)
    #expect(try await setTheme(["accent": "#654321"], url: root).ok)
    let theme = try await effectiveTheme(url: root)
    #expect(theme["accent"] == "#654321")
    func accent() async throws -> String? {
      try await controller.session.webView.callAsyncJavaScript(
        "return getComputedStyle(document.documentElement).getPropertyValue('--slop-accent').trim()",
        arguments: [:], in: nil, contentWorld: .page) as? String
    }
    #expect(try await accent() == "#654321")
    #expect(try await setTheme(["unknown": "red"], url: root).code == .rejected)
    // A shared theme file round-trips through the live owner and restyles the page.
    let exported =
      try JSONSerialization.jsonObject(with: await commandState("theme.export", url: root)) as? [String: String]
    let shared = try #require(exported?["file"])
    #expect(try await setTheme([:], url: root, replace: true).ok)
    #expect(try await themeCommand(["type": "importTheme", "file": shared], url: root).ok)
    try await controller.session.flush()
    #expect(try await accent() == "#654321")
    var foreign = try #require(try JSONSerialization.jsonObject(with: Data(shared.utf8)) as? [String: Any])
    foreign["template"] = "other-" + (foreign["template"] as? String ?? "")
    let foreignFile = String(decoding: try JSONSerialization.data(withJSONObject: foreign), as: UTF8.self)
    #expect(try await themeCommand(["type": "importTheme", "file": foreignFile], url: root).code == .rejected)
    // A coded rejection is known not to have applied.
    let refused = try await command(
      "batch", url: root,
      [
        "batch": [
          "intents": try JSONSerialization.jsonObject(
            with: Data((#"[{"type":"set","path":["missing"],"value":1}]"#).utf8))
        ]
      ])
    #expect(refused.code == .rejected)
    try await controller.session.reloadInterface()
    #expect(try await savedValue(root)?["title"] as? String == "Preserved through interface reload")
    let duplicate = root.deletingLastPathComponent().appendingPathComponent(
      UUID().uuidString + ".slop")
    defer { try? FileManager.default.removeItem(at: duplicate) }
    _ = try await controller.duplicateDocument(to: duplicate)
    #expect(try await effectiveTheme(url: duplicate) == effectiveTheme(url: root))
    // A duplicate carries the same saved state, and a preview rendered from it.
    #expect(try await savedValue(duplicate) == savedValue(root))
    #expect(SlopArtwork.png(duplicate, .preview) != nil)
    // Pointer sampling continues while asynchronous close releases storage. A ready
    // session must never expose an already-destroyed renderer to the native toolbar.
    var finished = false
    let close = Task { @MainActor in
      defer { finished = true }
      try await controller.session.close()
    }
    while !finished {
      controller.toolbar.refresh()
      await Task.yield()
    }
    try await close.value
    #expect(try await effectiveTheme(url: root) == theme)
    #expect(try await setTheme(["accent": nil], url: root).ok)
    #expect(try await effectiveTheme(url: root) == baseline)
    // A closed document imports through the owner the command opens.
    #expect(try await themeCommand(["type": "importTheme", "file": shared], url: root).ok)
    #expect(try await effectiveTheme(url: root) == theme)
  }

  // Failure: every CLI command ran a page close barrier that made the page inert, blurring
  // the field the user was typing in and cancelling IME composition.
  @Test @MainActor func cliEditLeavesTheFocusedFieldAlone() async throws {
    _ = NSApplication.shared
    let root = try contractFixture()
    defer { try? FileManager.default.removeItem(at: root) }
    let controller = try await SlopDocumentWindowController.open(url: root)
    try await controller.session.waitUntilReady()
    let view = controller.session.webView
    let focused = try await view.callAsyncJavaScript(
      "const input = document.createElement('input'); document.body.append(input); input.focus(); globalThis.__probe = input; return document.activeElement === input",
      arguments: [:], in: nil, contentWorld: .page)
    #expect(focused as? Bool == true)
    #expect(try await command("batch", url: root, setTitle("From the CLI")).ok)
    let still = try await view.callAsyncJavaScript(
      "return document.activeElement === globalThis.__probe && !document.body.inert",
      arguments: [:], in: nil, contentWorld: .page)
    #expect(still as? Bool == true)
    try await controller.session.close()
  }

  // A renderer that dies after an edit is durable takes neither the edit nor ownership
  // with it; Retry reattaches a new page to the same owner.
  @Test @MainActor func rendererDeathKeepsCommittedEditsAndOwnership() async throws {
    _ = NSApplication.shared
    let root = try contractFixture()
    defer { try? FileManager.default.removeItem(at: root) }
    let controller = try await SlopDocumentWindowController.open(url: root)
    try await controller.session.waitUntilReady()
    let engine = controller.session
    weak var oldWebView = engine.webView
    #expect(try await command("batch", url: root, setTitle("Committed before renderer death")).ok)
    let pid = try #require(engine.webView.value(forKey: "_webProcessIdentifier") as? Int32)
    #expect(Darwin.kill(pid, SIGKILL) == 0)
    await eventually(timeout: .seconds(5)) { engine.rendererDead }
    #expect(engine.rendererDead)
    #expect(try liveDiscovery(path: root.path) != nil)
    #expect(Fixtures.isLocked(root))
    _ = try await controller.perform(.retry)
    #expect(oldWebView == nil)
    #expect(try await savedValue(root)?["title"] as? String == "Committed before renderer death")
    try await controller.session.close()
    try await controller.session.close()
    #expect(try liveDiscovery(path: root.path) == nil)
    #expect(!Fixtures.isLocked(root))
  }

  // Gap: failed-save retry tests do not prove that explicit discard reloads bytes,
  // awaits the bridge, retains ownership, and remounts the visible app.
  @Test @MainActor func discardRestoresSavedStateWithoutReleasingOwnership() async throws {
    _ = NSApplication.shared
    let root = try contractFixture()
    defer { try? FileManager.default.removeItem(at: root) }
    let controller = try await SlopDocumentWindowController.open(url: root)
    try await controller.session.waitUntilReady()
    let engine = controller.session
    // Keep the window's handling, without presenting save or failure sheets.
    let events = SessionEvents(next: controller)
    events.status = { _ in }
    events.failure = { _ in }
    engine.delegate = events
    #expect(try await command("batch", url: root, setTitle("Durable title")).ok)
    let baseline = try await savedValue(root)
    // Writes fail while the document stays readable, so discard can reload it.
    let hold = try Fixtures.DatabaseHold(root, readable: true)
    #expect(try await command("batch", url: root, setTitle("Unsaved title")).code == .saveFailed)
    await #expect(throws: (any Error).self) { try await engine.prepareClose() }
    try await engine.discardPending()
    // Writes still fail: this succeeds only if discard really removed pending writes.
    #expect(try await savedValue(root) == baseline)
    hold.release()
    #expect(Fixtures.isLocked(root))
    #expect(try await command("batch", url: root, setTitle("After discard")).ok)
    try await controller.session.close()
    #expect(try await savedValue(root)?["title"] as? String == "After discard")
  }

  // A save that fails while another process holds the database, end to end through the
  // window controller. One hold covers what each failure once proved separately:
  // - a close stopped by the failed save fails as a save failure and never hands the window
  //   off (whether its alert shows, and once, is `SaveAttentionTests`; that the coordinator
  //   adds no second alert is `AppModelTests`);
  // - propagated close and quit failures do not report the storage incident twice;
  // - a capture never acquires a source, and ownership stays;
  // - close used to unmount the app first, leaving an open window with no app: the page
  //   stays mounted and editable;
  // - once the hold ends the edit saves, and a renderer death still closes and releases.
  @Test @MainActor func aFailedSaveKeepsTheDocumentOwnedEditableAndRecoverable() async throws {
    _ = NSApplication.shared
    let root = try contractFixture()
    defer { try? FileManager.default.removeItem(at: root) }
    var handedOff = false
    var operations: [SlopTelemetryEvent.Failure] = []
    let controller = try await SlopDocumentWindowController.open(
      url: root, routing: SlopDocumentRouting(command: { _ in }, closeHidden: { handedOff = true }),
      telemetry: SlopTelemetry { if case .failed(let operation, _) = $0 { operations.append(operation) } })
    controller.window?.moveOffScreen()
    try await controller.session.waitUntilReady()
    let session = controller.session
    // The window's save telemetry, without presenting its alert: a sheet would bring the
    // window on screen.
    let events = SessionEvents(next: controller)
    events.status = { [weak controller] status in controller?.recordSaveStatus(status) }
    events.failure = { _ in }
    session.delegate = events
    let hold = try Fixtures.DatabaseHold(root)
    #expect(try await command("batch", url: root, setTitle("Unsaved while held")).code == .saveFailed)

    var failure: SlopDocumentFailure?
    do { _ = try await controller.perform(.close) } catch { failure = SlopDocumentFailure(command: error) }
    #expect(failure == .save)
    #expect(!handedOff)
    #expect(!controller.isHiddenForClose)
    await #expect(throws: (any Error).self) { try await controller.prepareToClose(operation: .quit) }
    #expect(operations == [.save])

    var rendered = false
    await #expect(throws: (any Error).self) { try await session.withCaptureSnapshot { _ in rendered = true } }
    #expect(!rendered && !session.capturing)
    #expect(Fixtures.isLocked(root))
    hold.release()

    let edited =
      try await session.webView.callAsyncJavaScript(
        """
        const edit = document.getElementById('edit');
        if (!edit) return false;
        edit.click();
        return true
        """, arguments: [:], in: nil, contentWorld: .page) as? Bool
    #expect(edited == true)
    try await session.flush()
    #expect((try await savedValue(root)?["title"] as? String)?.hasPrefix("Edited") == true)

    let pid = try #require(session.webView.value(forKey: "_webProcessIdentifier") as? Int32)
    #expect(pid > 0)
    if pid > 0 { #expect(Darwin.kill(pid, SIGKILL) == 0) }
    await eventually(timeout: .seconds(3)) { session.rendererDead }
    #expect(session.rendererDead)
    try await controller.prepareToClose()
    try await session.close()
    controller.window?.orderOut(nil)
    #expect(!Fixtures.isLocked(root))
  }
}

extension HostTests {
  @Test(.nightly) @MainActor func saveTelemetryReportsOneFailureUntilRecovery() async throws {
    let root = try contractFixture()
    defer { try? FileManager.default.removeItem(at: root) }
    var events: [SlopTelemetryEvent] = []
    let controller = try await SlopDocumentWindowController.open(
      url: root, telemetry: SlopTelemetry { if case .failed = $0 { events.append($0) } })
    try await controller.session.waitUntilReady()
    // Status reporting is separate from the existing real failed-save/ownership test.
    let failed = DocumentSaveStatus.failed(.busy)
    controller.recordSaveStatus(failed)
    controller.recordSaveStatus(failed)
    #expect(events == [.failed(.save, .init(reason: .storage))])
    controller.recordSaveStatus(.saved)
    controller.recordSaveStatus(failed)
    #expect(events == Array(repeating: .failed(.save, .init(reason: .storage)), count: 2))
    try await controller.session.close()
  }
}

extension HostTests {
  // Gap: guest diagnostics are displayed but never reported. Expect a fixed category,
  // with neither the guest's message nor its arbitrary code in the uploaded fields.
  // Failure: every reported issue opened a blocking sheet, so a refused edit (a typed
  // number past its bound) interrupted the person. Oracle: only a save failure, which
  // puts unsaved work at risk, attaches a sheet.
  // The alert adapter, on a real window: `SaveAttention` decides, this presents it. An
  // authored issue shows the badge and never the sheet. Nightly: a sheet brings its window
  // on screen.
  @Test(.nightly) @MainActor func saveFailureSheetPresentsOnTheWindow() async throws {
    let root = try contractFixture()
    defer { try? FileManager.default.removeItem(at: root) }
    let controller = try await SlopDocumentWindowController.open(url: root)
    await controller.waitForPresentation()
    controller.pageSession(
      controller.session, didReport: SlopPageIssue(message: "DocumentError: out_of_range", isOperation: true))
    #expect(controller.issueBadge != nil)
    #expect(controller.window?.attachedSheet == nil)
    controller.pageSession(controller.session, saveStatus: .failed(.busy))
    controller.pageSession(controller.session, saveStatus: .failed(.busy))
    let window = try #require(controller.window)
    #expect(window.sheets.count == 1)
    let alert = try #require(window.attachedSheet)
    #expect(Set(alert.contentView.map { buttons(in: $0) } ?? []) == ["Retry Save", "Keep Open"])
    controller.pageSession(controller.session, saveStatus: .saved)
    await eventually { window.attachedSheet == nil }
    #expect(window.attachedSheet == nil)
    try await controller.session.close()
  }

  @Test(.nightly) @MainActor func authoredTelemetryUsesOnlyFixedCategories() async throws {
    let root = try contractFixture()
    defer { try? FileManager.default.removeItem(at: root) }
    var failures: [SlopFailureContext] = []
    let controller = try await SlopDocumentWindowController.open(
      url: root, telemetry: SlopTelemetry { if case .failed(_, let context) = $0 { failures.append(context) } })
    await controller.waitForPresentation()
    controller.pageSession(
      controller.session,
      didReport: SlopPageIssue(
        message: "secret document /private/example/document.slop", isOperation: false))
    #expect(failures.count == 1)
    #expect(failures.first?.classification == .authored)
    #expect(failures.first?.reason == .authoredException)
    #expect(
      failures.first?.fields(for: .renderer).values.contains(where: { $0.contains("private") || $0.contains("secret") })
        == false)
    try await controller.session.close()
  }

  // Gap: readiness and WebKit callbacks can report the same renderer failure twice.
  // Expect one incident until a new renderer is ready, then permit another incident.
  @Test(.nightly) @MainActor func rendererTelemetryDeduplicatesUntilReady() async throws {
    let root = try contractFixture()
    defer { try? FileManager.default.removeItem(at: root) }
    var failures = 0
    let controller = try await SlopDocumentWindowController.open(
      url: root, telemetry: SlopTelemetry { if case .failed = $0 { failures += 1 } })
    await controller.waitForPresentation()
    let error = CocoaError(.fileReadUnknown)
    controller.pageSession(controller.session, didFail: error)
    controller.pageSession(controller.session, didFail: error)
    #expect(failures == 1)
    controller.pageSessionDidBecomeReady(controller.session)
    controller.pageSession(controller.session, didFail: error)
    #expect(failures == 2)
    try await controller.session.close()
  }
}

extension HostTests {
  // Gap: duplication has success analytics but no error reporting. A rejected destination
  // must report once, cancellation must not fail, and the source must remain usable.
  @Test(.nightly) @MainActor func duplicateTelemetryReportsRejectedDestinationWithoutLosingSource() async throws {
    let root = try contractFixture()
    defer { try? FileManager.default.removeItem(at: root) }
    var failures: [SlopFailureContext] = []
    let controller = try await SlopDocumentWindowController.open(
      url: root, telemetry: SlopTelemetry { if case .failed(.duplicate, let context) = $0 { failures.append(context) } }
    )
    await controller.waitForPresentation()
    #expect(try await controller.duplicateDocument(to: nil) == nil)
    #expect(failures.isEmpty)
    await #expect(throws: (any Error).self) { _ = try await controller.duplicateDocument(to: root) }
    #expect(failures.count == 1)
    #expect(failures.first?.reason == .destinationExists)
    #expect(failures.first?.classification == .rejection)
    #expect(Fixtures.isLocked(root))
    try await controller.session.flush()
    try await controller.session.close()
  }
}

/// The titles of the buttons inside an alert's view, in order.
@MainActor private func buttons(in view: NSView) -> [String] {
  view.subviews.flatMap { subview -> [String] in
    if let button = subview as? NSButton, button.bezelStyle != .disclosure, !button.title.isEmpty { return [button.title] }
    return buttons(in: subview)
  }
}
