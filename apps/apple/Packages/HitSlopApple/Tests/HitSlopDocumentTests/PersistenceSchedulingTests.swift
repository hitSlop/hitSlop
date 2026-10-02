import Foundation
import HitSlopCore
import HitSlopCoreBinding
import Testing
import HitSlopTestSupport
@testable import HitSlopDocument

/// Pauses the persistence queue at one storage phase until released.
final class StorageGate: @unchecked Sendable {
  private let reachedSignal = DispatchSemaphore(value: 0)
  private let releaseSignal = DispatchSemaphore(value: 0)
  private let lock = NSLock()
  private var armed = true
  let phase: String
  init(_ phase: String) { self.phase = phase }
  func hook(_ at: String) {
    guard at == phase, lock.withLock({ let was = armed; armed = false; return was }) else { return }
    reachedSignal.signal()
    releaseSignal.wait()
  }
  func reached() async {
    await withCheckedContinuation { continuation in
      DispatchQueue.global().async { self.reachedSignal.wait(); continuation.resume() }
    }
  }
  func release() { releaseSignal.signal() }
}

/// Saves run on their own queue so a slow write never blocks edits. Each write must
/// acknowledge exactly what it contained, and close/discard must fence writes in flight.
/// Oracle: the bytes on disk, read back independently of the owner.
@Suite(.serialized) struct PersistenceSchedulingTests {
  let increment = #"{"intents":[{"type":"increment","path":["hits"],"by":3}]}"#

  func fixture() throws -> URL {
    let repository = #filePath.components(separatedBy: "/apps/apple/")[0]
    let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString + ".slop")
    try FileManager.default.copyItem(atPath: repository + "/tests/fixtures/checklist/document", toPath: root.path)
    let spec = try JSONSerialization.jsonObject(with: Data(contentsOf: URL(fileURLWithPath: repository + "/crates/hitslop-core/fixtures/checklist.json"))) as! [String: Any]
    for (file, key) in [("state.schema.json", "schema"), ("initial.json", "initial")] {
      try JSONSerialization.data(withJSONObject: spec[key]!).write(to: root.appendingPathComponent(file))
    }
    return root
  }

  /// The saved value, read without the owner (snapshot mode takes no lock).
  func savedHits(_ root: URL) throws -> Int? {
    let package = try SlopPackage(rootURL: root)
    let core = try NativeStore.open(root: root.path, mode: .snapshot)
      .document(
        schemaKey: package.schemaKey, initialJson: String(decoding: Data(contentsOf: package.initialURL), as: UTF8.self),
        themeDefaultsJson: package.themeDefaults)
    let frame = try JSONSerialization.jsonObject(with: Data(core.state().utf8)) as! [String: Any]
    return (frame["value"] as? [String: Any])?["hits"] as? Int
  }

  func edit(_ owner: DocumentOwner) async throws {
    _ = try await owner.apply(batch: increment)
  }

  // An accepted increment survives an in-flight committed save even if discard
  // replaces the owner before its socket reply. The CLI must not promise safe replay.
  @Test @MainActor func socketMutationAfterDiscardReportsUnknownWithoutReplay() async throws {
    let root = try fixture()
    defer { try? FileManager.default.removeItem(at: root) }
    let package = try SlopPackage(rootURL: root)
    let owner = try DocumentOwner(package: package)
    let gate = StorageGate("append:committed")
    defer { gate.release() }
    owner.testingPhase = gate.hook
    let mutations = Locked(0)
    let mutationCode = Locked<SocketReplyCode?>(nil)
    let server = try SocketServer { request, _ in
      if request.method == .batch { mutations.modify { $0 += 1 } }
      let reply = await owner.request(request)
      if request.method == .batch { mutationCode.modify { $0 = try? decodeReply(reply).code } }
      return reply
    }
    defer { server.stop() }
    try JSONSerialization.data(withJSONObject: [
      "socket": server.path, "documentPath": package.rootURL.path,
    ]).write(to: package.rootURL.appendingPathComponent("state/host.lock"))
    let command = Task { @MainActor () -> String in
      do {
        _ = try await command("apply", url: root,
          operation: Data(#"{"type":"increment","path":["hits"],"by":3}"#.utf8))
        return "Unexpected successful reply"
      } catch { return error.localizedDescription }
    }
    await gate.reached()
    let discarding = Task { try await owner.discardPending() }
    // Discard rejects the socket's flush before waiting for the gated storage queue.
    let message = await command.value
    gate.release()
    try await discarding.value
    owner.testingPhase = nil
    #expect(mutationCode.value == .failed)
    #expect(message.contains("Outcome unknown. Run slop get before issuing another edit."))
    #expect(!message.contains("Not applied."))
    #expect(mutations.value == 1)
    #expect(try savedHits(root) == 3)
    try await owner.close()
  }

  /// True when `work` finishes before the deadline. The deadline only bounds a failure;
  /// a passing run completes as soon as `work` does.
  func finishes(within seconds: Double, _ work: @escaping @Sendable () async throws -> Void) async -> Bool {
    final class Once: @unchecked Sendable {
      let lock = NSLock()
      var continuation: CheckedContinuation<Bool, Never>?
      func resume(_ value: Bool) { lock.withLock { continuation?.resume(returning: value); continuation = nil } }
    }
    let once = Once()
    return await withCheckedContinuation { continuation in
      once.continuation = continuation
      Task { try? await work(); once.resume(true) }
      Task { try? await Task.sleep(for: .seconds(seconds)); once.resume(false) }
    }
  }

  // Failure: autosave waited for a pause in editing, so a document edited continuously
  // never saved, and a crash lost everything since the last pause.
  @Test func continuousEditingStillAutosaves() async throws {
    let root = try fixture()
    defer { try? FileManager.default.removeItem(at: root) }
    let owner = try DocumentOwner(package: SlopPackage(rootURL: root))
    let start = ContinuousClock.now
    while (try savedHits(root) ?? 0) == 0, ContinuousClock.now - start < .seconds(3) {
      try await edit(owner)
      try await Task.sleep(for: .milliseconds(50))
    }
    // A completed write can leave newer edits pending, so `.saving` does not mean
    // autosave failed. Check the durable bytes before close can flush them.
    #expect((try savedHits(root) ?? 0) > 0)
    try await owner.close()
  }

  // Failure: a slow SQLite write blocked every edit, and a flush that joined an in-flight
  // write could be acknowledged by it although its edits were not in those bytes.
  @Test func editsProceedWhileASaveIsInFlightAndFlushCoversThem() async throws {
    let root = try fixture()
    defer { try? FileManager.default.removeItem(at: root) }
    let owner = try DocumentOwner(package: SlopPackage(rootURL: root))
    try await edit(owner)
    let gate = StorageGate("append:uncommitted")
    owner.testingPhase = gate.hook
    let first = Task { try await owner.flush() }
    await gate.reached()
    let proceeded = await finishes(within: 2) { try await edit(owner) }
    let second = Task { try await owner.flush() }
    gate.release()
    try await first.value
    try await second.value
    owner.testingPhase = nil
    #expect(proceeded)
    #expect(try savedHits(root) == 6)
    try await owner.close()
  }

  // Failure: a write that completes after discard must not leave the owner believing a
  // write is still in flight, or later saves never start.
  @Test func discardDuringAnInFlightWriteKeepsLaterSavesWorking() async throws {
    let root = try fixture()
    defer { try? FileManager.default.removeItem(at: root) }
    let owner = try DocumentOwner(package: SlopPackage(rootURL: root))
    try await edit(owner)
    let gate = StorageGate("append:committed")
    owner.testingPhase = gate.hook
    let saving = Task { try await owner.flush() }
    await gate.reached()
    let discarding = Task { try await owner.discardPending() }
    gate.release()
    _ = try? await saving.value
    try await discarding.value
    owner.testingPhase = nil
    try await edit(owner)
    #expect(await finishes(within: 2) { try await owner.flush() })
    #expect(try savedHits(root) == 6)
    try await owner.close()
  }

  // Failure: close released the writer lock or accepted edits before its final write
  // committed, letting another writer in or losing the late edit.
  @Test func closeRefusesEditsAndHoldsTheLockUntilItsFinalWrite() async throws {
    let root = try fixture()
    defer { try? FileManager.default.removeItem(at: root) }
    let owner = try DocumentOwner(package: SlopPackage(rootURL: root))
    try await edit(owner)
    let gate = StorageGate("append:uncommitted")
    owner.testingPhase = gate.hook
    let closing = Task { try await owner.close() }
    await gate.reached()
    await #expect(throws: (any Error).self) { try await edit(owner) }
    #expect(throws: DocumentLocked.self) { _ = try WriterLock.acquire(root) }
    gate.release()
    try await closing.value
    try WriterLock.acquire(root).release()
    #expect(try savedHits(root) == 3)
  }

  // Failure: an undo changed the document without saving it. Oracle: the saved bytes,
  // read back without the owner.
  @Test func undoSavesLikeAnEdit() async throws {
    let root = try fixture()
    defer { try? FileManager.default.removeItem(at: root) }
    let owner = try DocumentOwner(package: SlopPackage(rootURL: root))
    let states = Locked<[UndoAvailability]>([])
    owner.onUndoState = { state in states.modify { $0.append(state) } }
    _ = try await owner.apply(batch: increment)
    _ = try await owner.apply(batch: increment, origin: .page)
    try await owner.flush()
    #expect(try savedHits(root) == 6)
    _ = try await owner.undo()
    try await owner.flush()
    #expect(try savedHits(root) == 3)
    _ = try await owner.undo()
    try await owner.flush()
    #expect(try savedHits(root) == 0, "the agent's edit is undone too")
    #expect(states.value == [UndoAvailability(canUndo: true, canRedo: false), UndoAvailability(canUndo: true, canRedo: true),
      UndoAvailability(canUndo: false, canRedo: true)])
    try await owner.close()
  }

  // Failure: an agent's edit to a closed document lived only in the CLI process's undo,
  // so the next window could not undo it. Oracle: the saved bytes.
  @Test func aClosedAgentEditIsUndoneFromTheNextSession() async throws {
    let root = try fixture()
    defer { try? FileManager.default.removeItem(at: root) }
    let agent = try DocumentOwner(package: SlopPackage(rootURL: root))
    _ = try await agent.apply(batch: increment)
    try await agent.close()
    let owner = try DocumentOwner(package: SlopPackage(rootURL: root))
    let states = Locked<[UndoAvailability]>([])
    owner.onUndoState = { state in states.modify { $0.append(state) } }
    owner.publishUndoState()
    #expect(try savedHits(root) == 3)
    _ = try await owner.undo()
    try await owner.flush()
    #expect(try savedHits(root) == 0)
    _ = try await owner.undo(redo: true)
    try await owner.flush()
    #expect(try savedHits(root) == 3)
    #expect(states.value.first == UndoAvailability(canUndo: true, canRedo: false))
    try await owner.close()
  }

  // Failure: a document kept every edit it ever saw. Oracle: closing a session that
  // edited a large document leaves its saved state smaller, holding only that session.
  @Test func closingALargeEditedDocumentTrimsItsHistory() async throws {
    let root = try fixture()
    defer { try? FileManager.default.removeItem(at: root) }
    func stored() throws -> UInt64 {
      let meta = try NativeStore.open(root: root.path, mode: .snapshot).metadata()
      return meta.checkpointBytes + meta.updateBytes
    }
    var seed: UInt64 = 7
    func noise() -> String {
      String((0..<32 * 1024).map { _ in
        seed = seed &* 6364136223846793005 &+ 1442695040888963407
        return Character(UnicodeScalar(97 + UInt8((seed >> 59) % 26)))
      })
    }
    var beforeClose: UInt64 = 0
    for _ in 0..<2 {
      let owner = try DocumentOwner(package: SlopPackage(rootURL: root))
      for row in 0..<160 {
        // A row inserted and removed: history the live value no longer holds.
        let id = String(format: "%032x", row + 16)
        let batch: [String: Any] = ["intents": [
          ["type": "insert", "path": ["rows"], "id": id, "value": ["text": noise(), "done": false]],
          ["type": "remove", "path": ["rows"], "id": id],
        ]]
        _ = try await owner.apply(batch: String(decoding: try JSONSerialization.data(withJSONObject: batch), as: UTF8.self))
      }
      try await owner.flush()
      beforeClose = try stored()
      try await owner.close()
    }
    let afterClose: UInt64
    do { afterClose = try stored() } catch { Issue.record("\(error)"); return }
    #expect(afterClose < beforeClose / 2, "\(afterClose) of \(beforeClose) bytes")
  }

  // Failure: another process holding the database (a backup during Duplicate) must be a
  // definite, retryable failure, never mistaken for a lost reply or a conflict. The
  // store's own tests hold a real competing connection.
  @Test func busyDatabaseIsARetryableFailure() async throws {
    let root = try fixture()
    defer { try? FileManager.default.removeItem(at: root) }
    let owner = try DocumentOwner(package: SlopPackage(rootURL: root))
    try await edit(owner)
    owner.testingPhase = { phase in if phase == "append:uncommitted" { throw CoreError.Busy } }
    await #expect(throws: SaveFailure.busy) { try await owner.flush() }
    owner.testingPhase = nil
    try await owner.flush()
    #expect(try savedHits(root) == 3)
    try await owner.close()
  }
}


extension PersistenceSchedulingTests {
  // Failure: discard fences an active save, then a failed restore leaves writing
  // latched forever. Oracle: another edit becomes durable and close releases ownership.
  @Test func failedDiscardDuringWriteKeepsSavingUsable() async throws {
    let root = try fixture()
    defer { try? FileManager.default.removeItem(at: root) }
    let owner = try DocumentOwner(package: SlopPackage(rootURL: root))
    try await edit(owner)
    let gate = StorageGate("append:uncommitted")
    owner.testingPhase = { phase in
      gate.hook(phase)
      if phase == "load" { throw failure("injected restore failure") }
    }
    let saving = Task { try await owner.flush() }
    await gate.reached()
    let discarding = Task { try await owner.discardPending() }
    // Discard rejects the old save waiter before waiting for the persistence queue.
    _ = await saving.result
    gate.release()
    await #expect(throws: (any Error).self) { try await discarding.value }
    owner.testingPhase = nil
    try await edit(owner)
    let completed = await finishes(within: 2) { try await owner.flush() }
    #expect(completed)
    if completed { #expect(try savedHits(root) == 6) }
    // Also makes failure cleanup finite on the pre-fix implementation.
    try await owner.discardPending()
    try await owner.close()
  }

  @Test func discardDoesNotPublishSaveFailure() async throws {
    final class Statuses: @unchecked Sendable {
      let lock = NSLock()
      var values: [DocumentSaveStatus] = []
      func append(_ value: DocumentSaveStatus) { lock.withLock { values.append(value) } }
      var failed: Bool { lock.withLock { values.contains { if case .failed = $0 { true } else { false } } } }
    }
    let root = try fixture()
    defer { try? FileManager.default.removeItem(at: root) }
    let owner = try DocumentOwner(package: SlopPackage(rootURL: root))
    let statuses = Statuses()
    owner.onSaveStatus = { status in statuses.append(status) }
    try await edit(owner)
    try await owner.discardPending()
    #expect(!statuses.failed)
    try await owner.close()
  }
}


extension PersistenceSchedulingTests {
  @Test func failedCloseRetainsOwnershipAndAllowsRetry() async throws {
    let root = try fixture()
    defer { try? FileManager.default.removeItem(at: root) }
    let owner = try DocumentOwner(package: SlopPackage(rootURL: root))
    try await edit(owner)
    owner.testingPhase = { phase in
      if phase == "close" { throw failure("injected close failure") }
    }
    await #expect(throws: (any Error).self) { try await owner.close() }
    #expect(throws: DocumentLocked.self) { _ = try WriterLock.acquire(root) }
    owner.testingPhase = nil
    try await edit(owner)
    try await owner.close()
    try WriterLock.acquire(root).release()
    #expect(try savedHits(root) == 6)
  }
}


extension PersistenceSchedulingTests {
  // Failure: a flush admitted while discard restored saved bytes waited for the old
  // publication sequence, which the restored core (sequence 0) never reaches, so the
  // page hung. Oracle: the flush settles promptly as replaced, and saving still works.
  @Test func flushDuringDiscardRestoreSettlesAsReplaced() async throws {
    final class Reply: @unchecked Sendable {
      let lock = NSLock()
      var result: Result<PageOutcome, Error>?
      var continuation: CheckedContinuation<Void, Never>?
      func set(_ value: Result<PageOutcome, Error>) {
        lock.withLock { result = value; continuation?.resume(); continuation = nil }
      }
      func wait() async {
        await withCheckedContinuation { next in
          lock.withLock { if result == nil { continuation = next } else { next.resume() } }
        }
      }
    }
    let root = try fixture()
    defer { try? FileManager.default.removeItem(at: root) }
    let owner = try DocumentOwner(package: SlopPackage(rootURL: root))
    owner.attach(view: "page")
    try await edit(owner)
    try await edit(owner)
    let gate = StorageGate("load")
    owner.testingPhase = { gate.hook($0) }
    let discarding = Task { try await owner.discardPending() }
    await gate.reached()
    let reply = Reply()
    owner.enqueuePage(.flush, view: "page") { reply.set($0) }
    _ = try await owner.state()
    gate.release()
    try await discarding.value
    owner.testingPhase = nil
    let settled = await finishes(within: 2) { await reply.wait() }
    #expect(settled)
    if settled, case .failure(let error)? = reply.lock.withLock({ reply.result }) {
      #expect(error is OwnerReplaced)
    } else {
      Issue.record("flush during discard did not settle as replaced")
    }
    try await edit(owner)
    try await owner.flush()
    #expect(try savedHits(root) == 3)
    try await owner.close()
  }
}
