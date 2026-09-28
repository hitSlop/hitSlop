import AppKit
import Foundation
import HitSlopCore
import HitSlopRuntime
import PDFKit
import SQLite3
import Testing

@testable import HitSlopHost
@testable import HitSlopDocument

extension LoroClientTests {
  @Test @MainActor func themeOverridesSurviveReloadDuplicateAndClosedEditing() async throws {
    _ = NSApplication.shared
    let root = try contractFixture()
    defer { try? FileManager.default.removeItem(at: root) }
    let controller = try await SlopDocumentWindowController.open(packageURL: root)
    try await controller.session.waitUntilReady()
    let epoch = controller.session.engine.epoch
    _ = try await DocumentCommand.run(
      method: "apply", url: root, operation: prefixTitle("Preserved through interface reload"))
    _ = try await controller.session.webView.callAsyncJavaScript(
      "dispatchEvent(new ErrorEvent('error', {error:new Error('Test application failure')})); return true",
      arguments: [:], in: nil, contentWorld: .page)
    for _ in 0..<40 where controller.window?.attachedSheet == nil {
      try await Task.sleep(for: .milliseconds(25))
    }
    let issueSheet = try #require(controller.window?.attachedSheet)
    _ = try await DocumentCommand.run(
      method: "apply", url: root, operation: prefixTitle("Preserved through interface reload"))
    #expect(controller.window?.attachedSheet === issueSheet)
    controller.window?.endSheet(issueSheet, returnCode: .alertSecondButtonReturn)
    issueSheet.orderOut(nil)
    let baseline = try await DocumentCommand.run(method: "theme.get", url: root)
    let values = try JSONSerialization.data(withJSONObject: ["accent": "#654321"])
    let theme = try await DocumentCommand.run(method: "theme.set", url: root, themeValues: values)
    #expect(String(decoding: theme, as: UTF8.self).contains("#654321"))
    let css = try await controller.session.webView.callAsyncJavaScript(
      "return getComputedStyle(document.documentElement).getPropertyValue('--slop-accent').trim()",
      arguments: [:], in: nil, contentWorld: .page)
    #expect(css as? String == "#654321")
    await #expect(throws: (any Error).self) {
      _ = try await DocumentCommand.run(
        method: "theme.set", url: root, themeValues: Data("{\"unknown\":\"red\"}".utf8))
    }
    do {
      _ = try await DocumentCommand.run(
        method: "apply", url: root,
        operation: Data(#"{"type":"set","path":["missing"],"value":1}"#.utf8))
      Issue.record("Invalid operation was accepted")
    } catch {
      // A coded rejection is known not to have applied; only unknown outcomes ask for slop get.
      #expect(error.localizedDescription.hasSuffix("Not applied."))
    }
    try await controller.session.engine.reloadInterface()
    #expect(controller.session.engine.epoch == epoch)
    let state = try await DocumentCommand.run(method: "get", url: root)
    #expect(String(decoding: state, as: UTF8.self).contains("Preserved through interface reload"))
    let duplicate = root.deletingLastPathComponent().appendingPathComponent(
      UUID().uuidString + ".slop")
    defer { try? FileManager.default.removeItem(at: duplicate) }
    try SlopDuplicator.duplicate(from: root, to: duplicate)
    #expect(
      try Data(contentsOf: root.appendingPathComponent("state/theme.json"))
        == Data(contentsOf: duplicate.appendingPathComponent("state/theme.json")))
    // A duplicate is a new logical document carrying the same saved history.
    let original = try savedRow(root), copy = try savedRow(duplicate)
    #expect(original.docID != copy.docID)
    #expect(original.checkpoint == copy.checkpoint)
    // Pointer sampling continues while asynchronous close releases storage. A ready
    // session must never expose an already-destroyed renderer to the native toolbar.
    var finished = false
    let close = Task { @MainActor in
      defer { finished = true }
      try await controller.session.finish()
    }
    while !finished {
      controller.refreshToolbarHover()
      await Task.yield()
    }
    try await close.value
    let reopened = try await DocumentCommand.run(method: "theme.get", url: root)
    #expect(reopened == theme)
    _ = try await DocumentCommand.run(method: "theme.reset", url: root, themeToken: "accent")
    let reset = try await DocumentCommand.run(method: "theme.get", url: root)
    #expect(try JSONSerialization.jsonObject(with: reset) as? NSDictionary == JSONSerialization.jsonObject(with: baseline) as? NSDictionary)
  }

  #if DEBUG
  // These cases require fault-injection hooks that are absent from production builds.
  @Test @MainActor func committedWriteSurvivesRendererDeathBeforeAcknowledgement() async throws {
    _ = NSApplication.shared
    let root = try contractFixture()
    defer { try? FileManager.default.removeItem(at: root) }
    let controller = try await SlopDocumentWindowController.open(packageURL: root)
    try await controller.session.waitUntilReady()
    let engine = controller.session.engine
    let epoch = engine.epoch
    weak var oldWebView = engine.webView
    let pid = try #require(engine.webView.value(forKey: "_webProcessIdentifier") as? Int32)
    engine.owner.storage.testingPhase = { phase in
      guard phase == "append:committed" else { return }
      Darwin.kill(pid, SIGKILL)
      throw NSError(domain: "StorageFault", code: 1, userInfo: [NSLocalizedDescriptionKey: "Lost storage acknowledgement"])
    }
    // Renderer death closes the socket; the CLI must report unknown outcome.
    await #expect(throws: (any Error).self) {
      _ = try await DocumentCommand.run(
        method: "apply", url: root, operation: prefixTitle("Committed before renderer death"))
    }
    engine.owner.storage.testingPhase = nil
    for _ in 0..<200 where !engine.rendererDead { try await Task.sleep(for: .milliseconds(25)) }
    #expect(engine.rendererDead)
    #expect(
      !FileManager.default.fileExists(atPath: root.appendingPathComponent("state/host.lock").path))
    #expect(throws: (any Error).self) { _ = try DocumentWriterLock(root: root) }
    _ = try await controller.perform(.retry)
    #expect(engine.epoch == epoch)
    #expect(oldWebView == nil)
    let bytes = try await DocumentCommand.run(method: "get", url: root)
    #expect(String(decoding: bytes, as: UTF8.self).contains("Committed before renderer death"))
    try await controller.session.finish()
    try await controller.session.finish()
    #expect(
      !FileManager.default.fileExists(atPath: root.appendingPathComponent("state/host.lock").path))
    let ownership = try DocumentWriterLock(root: root)
    ownership.close()
  }

  @Test @MainActor func lostAppendReplyIsRecoveredByGetWithoutReplayingIntent() async throws {
    _ = NSApplication.shared
    let root = try contractFixture()
    defer { try? FileManager.default.removeItem(at: root) }
    let controller = try await SlopDocumentWindowController.open(packageURL: root)
    try await controller.session.waitUntilReady()
    let engine = controller.session.engine
    engine.owner.storage.testingPhase = { phase in
      guard phase == "append:committed" else { return }
      throw NSError(domain: "StorageFault", code: 1, userInfo: [NSLocalizedDescriptionKey: "Lost storage acknowledgement"])
    }
    _ = try await DocumentCommand.run(
      method: "apply", url: root, operation: prefixTitle("Lost reply"))
    engine.owner.storage.testingPhase = nil
    let bytes = try await DocumentCommand.run(method: "get", url: root)
    #expect(String(decoding: bytes, as: UTF8.self).contains("Lost reply"))
    try await controller.session.finish()
    let reopened = try await DocumentCommand.run(method: "get", url: root)
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
    let engine = controller.session.engine
    engine.onStatus = nil
    engine.onError = nil
    _ = try await DocumentCommand.run(method: "apply", url: root, operation: prefixTitle("Durable title"))
    let baseline = try await DocumentCommand.run(method: "get", url: root)
    let identity = try savedRow(root).docID
    engine.owner.storage.testingPhase = { phase in
      guard phase == "append:uncommitted" || phase == "checkpoint:uncommitted" else { return }
      throw NSError(domain: "StorageFault", code: 2, userInfo: [NSLocalizedDescriptionKey: "Injected save failure"])
    }
    await #expect(throws: (any Error).self) {
      _ = try await DocumentCommand.run(method: "apply", url: root, operation: prefixTitle("Unsaved title"))
    }
    await #expect(throws: (any Error).self) { try await engine.prepareClose() }
    #expect(throws: (any Error).self) { _ = try DocumentWriterLock(root: root) }
    try await engine.discardPending()
    // Keep writes failing: this succeeds only if discard really removed pending writes.
    let restored = try await DocumentCommand.run(method: "get", url: root)
    #expect(restored == baseline)
    #expect(try savedRow(root).docID == identity)
    #expect(throws: (any Error).self) { _ = try DocumentWriterLock(root: root) }
    engine.owner.storage.testingPhase = nil
    _ = try await DocumentCommand.run(method: "apply", url: root, operation: prefixTitle("After discard"))
    try await controller.session.finish()
    let reopened = try await DocumentCommand.run(method: "get", url: root)
    #expect(String(decoding: reopened, as: UTF8.self).contains("After discard"))
  }

  @Test @MainActor func failedSaveRetainsOwnershipAndRendererDeathReleasesOnClose() async throws {
    let root = try contractFixture()
    defer { try? FileManager.default.removeItem(at: root) }
    let controller = try await SlopDocumentWindowController.open(packageURL: root)
    try await controller.session.waitUntilReady()
    var operations: [SlopTelemetryEvent.Failure] = []
    controller.telemetry = SlopTelemetry { if case .failed(let operation, _) = $0 { operations.append(operation) } }
    controller.session.engine.onStatus = { [weak controller] status in
      guard let controller else { return }
      // Preserve production status handling, without displaying a sheet in the test harness.
      controller.runtimeSession(controller.session, saveStatus: DocumentSaveStatus(status: status.status, error: nil))
    }
    controller.session.engine.onError = nil
    controller.session.engine.owner.storage.testingPhase = { phase in
      guard phase == "append:uncommitted" || phase == "checkpoint:uncommitted" else { return }
      throw NSError(domain: "StorageFault", code: 2, userInfo: [NSLocalizedDescriptionKey: "Injected save failure"])
    }
    do {
      _ = try await DocumentCommand.run(
        method: "apply", url: root, operation: prefixTitle("Recovered edit"))
      Issue.record("Injected write succeeded")
    } catch { #expect(error.localizedDescription.contains("Run slop get")) }
    do {
      try await controller.prepareToClose(operation: .quit)
      try await controller.finishClose(operation: .quit)
      Issue.record("Failed save allowed close")
    } catch {}
    // Gap: propagated close/quit errors must not duplicate the storage incident.
    #expect(operations == [.save])
    #expect(throws: (any Error).self) { _ = try DocumentWriterLock(root: root) }
    controller.session.engine.owner.storage.testingPhase = nil
    try await controller.session.flush()
    let pid = try #require(
      controller.session.webView.value(forKey: "_webProcessIdentifier") as? Int32)
    #expect(pid > 0)
    if pid > 0 { #expect(Darwin.kill(pid, SIGKILL) == 0) }
    for _ in 0..<100 where !controller.session.engine.rendererDead {
      try await Task.sleep(for: .milliseconds(30))
    }
    #expect(controller.session.engine.rendererDead)
    try await controller.prepareToClose()
    try await controller.session.finish()
    controller.window?.orderOut(nil)
    #expect(
      String(decoding: try await DocumentCommand.run(method: "get", url: root), as: UTF8.self)
        .contains("Recovered edit"))
  }
  #endif
}

extension LoroClientTests {
  @Test @MainActor func saveTelemetryReportsOneFailureUntilRecovery() async throws {
    let root = try contractFixture()
    defer { try? FileManager.default.removeItem(at: root) }
    let controller = try await SlopDocumentWindowController.open(packageURL: root)
    try await controller.session.waitUntilReady()
    var events: [SlopTelemetryEvent] = []
    controller.telemetry = SlopTelemetry { if case .failed = $0 { events.append($0) } }
    // Status reporting is separate from the existing real failed-save/ownership test.
    let failed = DocumentSaveStatus(status: "save-failed", error: nil)
    controller.runtimeSession(controller.session, saveStatus: failed)
    controller.runtimeSession(controller.session, saveStatus: failed)
    #expect(events == [.failed(.save, .init(reason: .storage))])
    controller.runtimeSession(controller.session, saveStatus: DocumentSaveStatus(status: "saved", error: nil))
    controller.runtimeSession(controller.session, saveStatus: failed)
    #expect(events == Array(repeating: .failed(.save, .init(reason: .storage)), count: 2))
    try await controller.session.finish()
  }
}

extension LoroClientTests {
  // Gap: guest diagnostics are displayed but never reported. Expect a fixed category,
  // with neither the guest's message nor its arbitrary code in the uploaded fields.
  @Test @MainActor func authoredTelemetryUsesOnlyFixedCategories() async throws {
    let root = try contractFixture()
    defer { try? FileManager.default.removeItem(at: root) }
    let controller = try await SlopDocumentWindowController.open(packageURL: root)
    await controller.waitForPresentation()
    var failures: [SlopFailureContext] = []
    controller.telemetry = SlopTelemetry {
      if case .failed(_, let context) = $0 { failures.append(context) }
    }
    controller.runtimeSession(controller.session, didReport: SlopRuntimeIssue(
      source: .unhandled, code: "private-code", message: "secret document /private/example/document.slop"))
    #expect(failures.count == 1)
    #expect(failures.first?.classification == .authored)
    #expect(failures.first?.reason == .authoredException)
    #expect(failures.first?.fields(for: .renderer).values.contains(where: { $0.contains("private") || $0.contains("secret") }) == false)
    try await controller.session.finish()
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
    controller.runtimeSession(controller.session, didFail: error)
    controller.runtimeSession(controller.session, didFail: error)
    #expect(failures == 1)
    controller.runtimeSessionDidBecomeReady(controller.session)
    controller.runtimeSession(controller.session, didFail: error)
    #expect(failures == 2)
    try await controller.session.finish()
  }
}

extension LoroClientTests {
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
    #expect(throws: (any Error).self) { _ = try DocumentWriterLock(root: root) }
    try await controller.session.flush()
    try await controller.session.finish()
  }
}

/// Reads the host-owned identity and checkpoint directly from a closed or live package.
private func savedRow(_ root: URL) throws -> (docID: String, checkpoint: Data?) {
  var db: OpaquePointer?
  defer { sqlite3_close(db) }
  let path = root.appendingPathComponent("state/document.sqlite").path
  var statement: OpaquePointer?
  guard sqlite3_open_v2(path, &db, SQLITE_OPEN_READONLY, nil) == SQLITE_OK,
    sqlite3_prepare_v2(db, "SELECT doc_id, checkpoint FROM document WHERE id=1", -1, &statement, nil) == SQLITE_OK,
    sqlite3_step(statement) == SQLITE_ROW, let text = sqlite3_column_text(statement, 0)
  else { sqlite3_finalize(statement); throw CocoaError(.fileReadCorruptFile) }
  defer { sqlite3_finalize(statement) }
  let size = Int(sqlite3_column_bytes(statement, 1))
  let checkpoint = sqlite3_column_blob(statement, 1).map { Data(bytes: $0, count: size) }
  return (String(cString: text), checkpoint)
}
