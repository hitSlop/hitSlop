import AppKit
import Foundation
import HitSlopCore
import HitSlopCoreBinding
import PDFKit
import Testing
import HitSlopTestSupport

@testable import HitSlopHost
@testable import HitSlopDocument

extension OwnerClientTests {
  @Test @MainActor func themeOverridesSurviveReloadDuplicateAndClosedEditing() async throws {
    _ = NSApplication.shared
    let root = try contractFixture()
    defer { try? FileManager.default.removeItem(at: root) }
    let controller = try await SlopDocumentWindowController.open(packageURL: root)
    try await controller.session.waitUntilReady()
    let epoch = controller.session.epoch
    _ = try await command("apply", url: root, operation: setTitle("Preserved through interface reload"))
    _ = try await controller.session.webView.callAsyncJavaScript(
      "dispatchEvent(new ErrorEvent('error', {error:new Error('Test application failure')})); return true",
      arguments: [:], in: nil, contentWorld: .page)
    // An issue that leaves the slop running shows as the badge, not a blocking sheet,
    // and a CLI edit leaves it until the person dismisses it.
    for _ in 0..<40 where controller.issueBadge == nil {
      try await Task.sleep(for: .milliseconds(25))
    }
    let badge = try #require(controller.issueBadge)
    #expect(controller.window?.attachedSheet == nil)
    _ = try await command("apply", url: root, operation: setTitle("Preserved through interface reload"))
    #expect(controller.issueBadge === badge)
    controller.dismissIssue()
    #expect(controller.issueBadge == nil)
    let baseline = try await command("theme.get", url: root)
    let values = try JSONSerialization.data(withJSONObject: ["accent": "#654321"])
    let theme = try await command("theme.set", url: root, themeValues: values)
    #expect(String(decoding: theme, as: UTF8.self).contains("#654321"))
    let css = try await controller.session.webView.callAsyncJavaScript(
      "return getComputedStyle(document.documentElement).getPropertyValue('--slop-accent').trim()",
      arguments: [:], in: nil, contentWorld: .page)
    #expect(css as? String == "#654321")
    await #expect(throws: (any Error).self) {
      _ = try await command("theme.set", url: root, themeValues: Data("{\"unknown\":\"red\"}".utf8))
    }
    do {
      _ = try await command("apply", url: root,
        operation: Data(#"{"type":"set","path":["missing"],"value":1}"#.utf8))
      Issue.record("Invalid operation was accepted")
    } catch {
      // A coded rejection is known not to have applied; only unknown outcomes ask for slop get.
      #expect(error.localizedDescription.hasSuffix("Not applied."))
    }
    try await controller.session.reloadInterface()
    #expect(controller.session.epoch == epoch)
    let state = try await command("get", url: root)
    #expect(String(decoding: state, as: UTF8.self).contains("Preserved through interface reload"))
    let duplicate = root.deletingLastPathComponent().appendingPathComponent(
      UUID().uuidString + ".slop")
    defer { try? FileManager.default.removeItem(at: duplicate) }
    try SlopDuplicator.duplicate(from: root, to: duplicate)
    #expect(try await command("theme.get", url: duplicate) == command("theme.get", url: root))
    // A duplicate is a new logical document carrying the same saved state.
    #expect(try savedDocumentID(root) != savedDocumentID(duplicate))
    #expect(try await command("get", url: duplicate) == command("get", url: root))
    // Pointer sampling continues while asynchronous close releases storage. A ready
    // session must never expose an already-destroyed renderer to the native toolbar.
    var finished = false
    let close = Task { @MainActor in
      defer { finished = true }
      try await controller.session.close()
    }
    while !finished {
      controller.refreshToolbarHover()
      await Task.yield()
    }
    try await close.value
    let reopened = try await command("theme.get", url: root)
    #expect(reopened == theme)
    _ = try await command("theme.reset", url: root, themeToken: "accent")
    let reset = try await command("theme.get", url: root)
    #expect(try JSONSerialization.jsonObject(with: reset) as? NSDictionary == JSONSerialization.jsonObject(with: baseline) as? NSDictionary)
  }

  // Failure: every CLI command ran a page close barrier that made the page inert, blurring
  // the field the user was typing in and cancelling IME composition.
  @Test @MainActor func cliEditLeavesTheFocusedFieldAlone() async throws {
    _ = NSApplication.shared
    let root = try contractFixture()
    defer { try? FileManager.default.removeItem(at: root) }
    let controller = try await SlopDocumentWindowController.open(packageURL: root)
    try await controller.session.waitUntilReady()
    let view = controller.session.webView
    let focused = try await view.callAsyncJavaScript(
      "const input = document.createElement('input'); document.body.append(input); input.focus(); globalThis.__probe = input; return document.activeElement === input",
      arguments: [:], in: nil, contentWorld: .page)
    #expect(focused as? Bool == true)
    _ = try await command("apply", url: root, operation: setTitle("From the CLI"))
    let still = try await view.callAsyncJavaScript(
      "return document.activeElement === globalThis.__probe && !document.body.inert",
      arguments: [:], in: nil, contentWorld: .page)
    #expect(still as? Bool == true)
    try await controller.session.close()
  }

  #if DEBUG
  // These cases require fault-injection hooks that are absent from production builds.
  @Test @MainActor func committedWriteSurvivesRendererDeathAndLostAcknowledgement() async throws {
    _ = NSApplication.shared
    let root = try contractFixture()
    defer { try? FileManager.default.removeItem(at: root) }
    let controller = try await SlopDocumentWindowController.open(packageURL: root)
    try await controller.session.waitUntilReady()
    let engine = controller.session
    let epoch = engine.epoch
    weak var oldWebView = engine.webView
    let pid = try #require(engine.webView.value(forKey: "_webProcessIdentifier") as? Int32)
    engine.owner.testingPhase = { phase in
      guard phase.hasSuffix(":committed") else { return }
      Darwin.kill(pid, SIGKILL)
      throw NSError(domain: "StorageFault", code: 1, userInfo: [NSLocalizedDescriptionKey: "Lost storage acknowledgement"])
    }
    // The socket follows the owner. A lost acknowledgement reports failure until
    // a later retry confirms the save; the accepted edit and ownership remain.
    await #expect(throws: (any Error).self) {
      _ = try await command("apply", url: root, operation: setTitle("Committed before renderer death"))
    }
    engine.owner.testingPhase = nil
    for _ in 0..<200 where !engine.rendererDead { try await Task.sleep(for: .milliseconds(25)) }
    #expect(engine.rendererDead)
    #expect(
      FileManager.default.fileExists(atPath: root.appendingPathComponent("state/host.lock").path))
    #expect(throws: (any Error).self) { _ = try WriterLock.acquire(root) }
    _ = try await controller.perform(.retry)
    #expect(engine.epoch == epoch)
    #expect(oldWebView == nil)
    let bytes = try await command("get", url: root)
    #expect(String(decoding: bytes, as: UTF8.self).contains("Committed before renderer death"))
    try await controller.session.close()
    try await controller.session.close()
    #expect(
      !FileManager.default.fileExists(atPath: root.appendingPathComponent("state/host.lock").path))
    let ownership = try WriterLock.acquire(root)
    ownership.release()
  }

  // Failure: close unmounted the app before the native close, so a failed close left an
  // open, editable window with no app in it.
  @Test @MainActor func failedCloseKeepsTheAppMountedAndEditable() async throws {
    _ = NSApplication.shared
    let root = try contractFixture()
    defer { try? FileManager.default.removeItem(at: root) }
    let controller = try await SlopDocumentWindowController.open(packageURL: root)
    try await controller.session.waitUntilReady()
    let session = controller.session
    session.owner.testingPhase = { phase in
      guard phase == "close" else { return }
      throw NSError(domain: "StorageFault", code: 3, userInfo: [NSLocalizedDescriptionKey: "Injected close failure"])
    }
    await #expect(throws: (any Error).self) { try await session.close() }
    session.owner.testingPhase = nil
    let edited = try await session.webView.callAsyncJavaScript("""
      const edit = document.getElementById('edit');
      if (!edit) return false;
      edit.click();
      return true
      """, arguments: [:], in: nil, contentWorld: .page) as? Bool
    #expect(edited == true)
    try await session.flush()
    #expect(String(decoding: try await command("get", url: root), as: UTF8.self).contains("Edited"))
    try await session.close()
    try WriterLock.acquire(root).release()
  }

  @Test @MainActor func lostAcknowledgementIsRetriedWithoutReplayingIncrement() async throws {
    _ = NSApplication.shared
    let root = try contractFixture()
    defer { try? FileManager.default.removeItem(at: root) }
    let controller = try await SlopDocumentWindowController.open(packageURL: root)
    try await controller.session.waitUntilReady()
    let engine = controller.session
    engine.owner.testingPhase = { phase in
      guard phase.hasSuffix(":committed") else { return }
      throw NSError(domain: "StorageFault", code: 1, userInfo: [NSLocalizedDescriptionKey: "Lost storage acknowledgement"])
    }
    await #expect(throws: (any Error).self) {
      _ = try await command("apply", url: root,
        operation: Data(#"{"type":"increment","path":["hits"],"by":1}"#.utf8))
    }
    engine.owner.testingPhase = nil
    let bytes = try await command("get", url: root)
    #expect((try JSONSerialization.jsonObject(with: bytes) as? [String: Any])?["hits"] as? Int == 1)
    try await controller.session.close()
    let reopened = try await command("get", url: root)
    #expect(reopened == bytes)
  }

  // Gap: failed-save retry tests do not prove that explicit discard reloads bytes,
  // awaits the bridge, retains ownership, and remounts the visible app.
  @Test @MainActor func discardRestoresSavedStateWithoutReleasingOwnership() async throws {
    _ = NSApplication.shared
    let root = try contractFixture()
    defer { try? FileManager.default.removeItem(at: root) }
    let controller = try await SlopDocumentWindowController.open(packageURL: root)
    try await controller.session.waitUntilReady()
    let engine = controller.session
    // Keep the window's handling, without presenting save or failure sheets.
    let events = SessionEvents(next: controller)
    events.status = { _ in }
    events.failure = { _ in }
    engine.delegate = events
    _ = try await command("apply", url: root, operation: setTitle("Durable title"))
    let baseline = try await command("get", url: root)
    let identity = try savedDocumentID(root)
    engine.owner.testingPhase = { phase in
      guard phase == "append:uncommitted" || phase == "checkpoint:uncommitted" else { return }
      throw NSError(domain: "StorageFault", code: 2, userInfo: [NSLocalizedDescriptionKey: "Injected save failure"])
    }
    await #expect(throws: (any Error).self) {
      _ = try await command("apply", url: root, operation: setTitle("Unsaved title"))
    }
    await #expect(throws: (any Error).self) { try await engine.prepareClose() }
    #expect(throws: (any Error).self) { _ = try WriterLock.acquire(root) }
    try await engine.discardPending()
    // Keep writes failing: this succeeds only if discard really removed pending writes.
    let restored = try await command("get", url: root)
    #expect(restored == baseline)
    #expect(try savedDocumentID(root) == identity)
    #expect(throws: (any Error).self) { _ = try WriterLock.acquire(root) }
    engine.owner.testingPhase = nil
    _ = try await command("apply", url: root, operation: setTitle("After discard"))
    try await controller.session.close()
    let reopened = try await command("get", url: root)
    #expect(String(decoding: reopened, as: UTF8.self).contains("After discard"))
  }

  @Test @MainActor func failedSaveRetainsOwnershipAndRendererDeathReleasesOnClose() async throws {
    let root = try contractFixture()
    defer { try? FileManager.default.removeItem(at: root) }
    let controller = try await SlopDocumentWindowController.open(packageURL: root)
    try await controller.session.waitUntilReady()
    var operations: [SlopTelemetryEvent.Failure] = []
    controller.telemetry = SlopTelemetry { if case .failed(let operation, _) = $0 { operations.append(operation) } }
    // Preserve production status handling, without displaying a sheet in the test harness.
    let events = SessionEvents(next: controller)
    events.status = { [weak controller] status in controller?.recordSaveStatus(status) }
    events.failure = { _ in }
    controller.session.delegate = events
    controller.session.owner.testingPhase = { phase in
      guard phase == "append:uncommitted" || phase == "checkpoint:uncommitted" else { return }
      throw NSError(domain: "StorageFault", code: 2, userInfo: [NSLocalizedDescriptionKey: "Injected save failure"])
    }
    do {
      _ = try await command("apply", url: root, operation: setTitle("Recovered edit"))
      Issue.record("Injected write succeeded")
    } catch { #expect(error.localizedDescription.contains("Run slop get")) }
    do {
      try await controller.prepareToClose(operation: .quit)
      try await controller.finishClose(operation: .quit)
      Issue.record("Failed save allowed close")
    } catch {}
    // Gap: propagated close/quit errors must not duplicate the storage incident.
    #expect(operations == [.save])
    #expect(throws: (any Error).self) { _ = try WriterLock.acquire(root) }
    controller.session.owner.testingPhase = nil
    try await controller.session.flush()
    let pid = try #require(
      controller.session.webView.value(forKey: "_webProcessIdentifier") as? Int32)
    #expect(pid > 0)
    if pid > 0 { #expect(Darwin.kill(pid, SIGKILL) == 0) }
    for _ in 0..<100 where !controller.session.rendererDead {
      try await Task.sleep(for: .milliseconds(30))
    }
    #expect(controller.session.rendererDead)
    try await controller.prepareToClose()
    try await controller.session.close()
    controller.window?.orderOut(nil)
    #expect(
      String(decoding: try await command("get", url: root), as: UTF8.self)
        .contains("Recovered edit"))
  }
  #endif
}

extension OwnerClientTests {
  @Test @MainActor func saveTelemetryReportsOneFailureUntilRecovery() async throws {
    let root = try contractFixture()
    defer { try? FileManager.default.removeItem(at: root) }
    let controller = try await SlopDocumentWindowController.open(packageURL: root)
    try await controller.session.waitUntilReady()
    var events: [SlopTelemetryEvent] = []
    controller.telemetry = SlopTelemetry { if case .failed = $0 { events.append($0) } }
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

extension OwnerClientTests {
  // Gap: guest diagnostics are displayed but never reported. Expect a fixed category,
  // with neither the guest's message nor its arbitrary code in the uploaded fields.
  // Failure: every reported issue opened a blocking sheet, so a refused edit (a typed
  // number past its bound) interrupted the person. Oracle: only a save failure, which
  // puts unsaved work at risk, attaches a sheet.
  @Test @MainActor func onlySaveFailuresBlockTheWindow() async throws {
    let root = try contractFixture()
    defer { try? FileManager.default.removeItem(at: root) }
    let controller = try await SlopDocumentWindowController.open(packageURL: root)
    await controller.waitForPresentation()
    controller.pageSession(controller.session, didReport: SlopPageIssue(
      message: "OperationRejectedError: out_of_range", isOperation: true))
    #expect(controller.issueBadge != nil)
    #expect(controller.window?.attachedSheet == nil)
    controller.pageSession(controller.session, saveStatus: .failed(.busy))
    let sheet = try #require(controller.window?.attachedSheet)
    #expect(controller.issueBadge != nil)
    controller.window?.endSheet(sheet, returnCode: .alertSecondButtonReturn)
    sheet.orderOut(nil)
    try await controller.session.close()
  }

  @Test @MainActor func authoredTelemetryUsesOnlyFixedCategories() async throws {
    let root = try contractFixture()
    defer { try? FileManager.default.removeItem(at: root) }
    let controller = try await SlopDocumentWindowController.open(packageURL: root)
    await controller.waitForPresentation()
    var failures: [SlopFailureContext] = []
    controller.telemetry = SlopTelemetry {
      if case .failed(_, let context) = $0 { failures.append(context) }
    }
    controller.pageSession(controller.session, didReport: SlopPageIssue(
      message: "secret document /private/example/document.slop", isOperation: false))
    #expect(failures.count == 1)
    #expect(failures.first?.classification == .authored)
    #expect(failures.first?.reason == .authoredException)
    #expect(failures.first?.fields(for: .renderer).values.contains(where: { $0.contains("private") || $0.contains("secret") }) == false)
    try await controller.session.close()
  }

  // Gap: readiness and WebKit callbacks can report the same renderer failure twice.
  // Expect one incident until a new renderer is ready, then permit another incident.
  @Test @MainActor func rendererTelemetryDeduplicatesUntilReady() async throws {
    let root = try contractFixture()
    defer { try? FileManager.default.removeItem(at: root) }
    let controller = try await SlopDocumentWindowController.open(packageURL: root)
    await controller.waitForPresentation()
    var failures = 0
    controller.telemetry = SlopTelemetry { if case .failed = $0 { failures += 1 } }
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

extension OwnerClientTests {
  // Gap: duplication has success analytics but no error reporting. A rejected destination
  // must report once, cancellation must not fail, and the source must remain usable.
  @Test @MainActor func duplicateTelemetryReportsRejectedDestinationWithoutLosingSource() async throws {
    let root = try contractFixture()
    defer { try? FileManager.default.removeItem(at: root) }
    let controller = try await SlopDocumentWindowController.open(packageURL: root)
    await controller.waitForPresentation()
    var failures: [SlopFailureContext] = []
    controller.telemetry = SlopTelemetry {
      if case .failed(.duplicate, let context) = $0 { failures.append(context) }
    }
    #expect(try await controller.duplicateDocument(to: nil) == nil)
    #expect(failures.isEmpty)
    await #expect(throws: (any Error).self) { _ = try await controller.duplicateDocument(to: root) }
    #expect(failures.count == 1)
    #expect(failures.first?.reason == .destinationExists)
    #expect(failures.first?.classification == .rejection)
    #expect(throws: (any Error).self) { _ = try WriterLock.acquire(root) }
    try await controller.session.flush()
    try await controller.session.close()
  }
}

/// The saved document identity of a closed or live package (a snapshot takes no lock).
private func savedDocumentID(_ root: URL) throws -> String {
  try NativeStore.open(root: root.path, mode: .snapshot).docId()
}
