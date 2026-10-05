import Foundation
import HitSlopCore
import HitSlopCoreBinding
import Testing
import HitSlopTestSupport
@testable import HitSlopDocument

/// Saves run on their own queue so a slow write never blocks edits. Each write must
/// acknowledge exactly what it contained, and close/discard must fence writes in flight.
/// Oracle: the bytes on disk, read back independently of the owner. Faults are real: a held
/// database, a moved file, and another connection holding SQLite's write lock.
@Suite(.serialized) struct PersistenceSchedulingTests {
  let increment = #"{"intents":[{"type":"increment","path":["hits"],"by":3}]}"#

  func fixture() throws -> URL { try Fixtures.checklistDocument() }

  /// The saved value, read without the owner (snapshot mode takes no lock).
  func savedHits(_ root: URL) throws -> Int? {
    let core = try NativeStore.open(path: root.path, mode: .snapshot).document()
    let frame = try Fixtures.object(core.state())
    return (frame["value"] as? [String: Any])?["hits"] as? Int
  }
  /// The owner's live value.
  func liveHits(_ owner: DocumentOwner) async throws -> Int? {
    let frame = try Fixtures.object(await owner.state())
    return (frame["value"] as? [String: Any])?["hits"] as? Int
  }

  func edit(_ owner: DocumentOwner) async throws {
    _ = try await owner.apply(batch: increment)
  }

  // An accepted increment survives a save in flight even if discard replaces the owner
  // before its socket reply. The CLI must not promise safe replay.
  @Test @MainActor func socketMutationAfterDiscardReportsUnknownWithoutReplay() async throws {
    let root = try fixture()
    defer { try? FileManager.default.removeItem(at: root) }
    let owner = try DocumentOwner(url: root)
    let hold = try Fixtures.DatabaseHold(root)
    defer { hold.release() }
    let server = try owner.startServer(exporter: NativeExports { _, _, _, _ in throw OwnerError.rejected("No renderer") })
    defer { server.stop() }
    let reply = Task { @MainActor () -> (OutcomeCode?, String?) in
      let reply = try await command("batch", url: root, ["ops": #"[{"type":"increment","path":["hits"],"by":3}]"#])
      return (reply.code, reply.error)
    }
    // Accepted, and waiting for its save.
    #expect(try await eventually(timeout: .seconds(5)) { try await liveHits(owner) == 3 })
    let discarding = Task { try await owner.discardPending() }
    // Discard rejects the socket's flush before its reload waits for the held queue.
    let (code, error) = try await reply.value
    hold.release()
    try await discarding.value
    #expect(code == .unknownOutcome)
    #expect(error?.contains("could not be confirmed") == true)
    #expect(try savedHits(root) == 3)
    try await owner.close()
  }

  // Failure: autosave waited for a pause in editing, so a document edited continuously
  // never saved, and a crash lost everything since the last pause.
  @Test func continuousEditingStillAutosaves() async throws {
    let root = try fixture()
    defer { try? FileManager.default.removeItem(at: root) }
    let owner = try DocumentOwner(url: root)
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
    let owner = try DocumentOwner(url: root)
    try await edit(owner)
    let hold = try Fixtures.DatabaseHold(root)
    let first = Task { try await owner.flush() }
    try await edit(owner)
    let second = Task { try await owner.flush() }
    hold.release()
    try await first.value
    try await second.value
    #expect(try savedHits(root) == 6)
    try await owner.close()
  }

  // Failure: a write that completes after discard must not leave the owner believing a
  // write is still in flight, or later saves never start.
  @Test func discardDuringAnInFlightWriteKeepsLaterSavesWorking() async throws {
    let root = try fixture()
    defer { try? FileManager.default.removeItem(at: root) }
    let owner = try DocumentOwner(url: root)
    try await edit(owner)
    let hold = try Fixtures.DatabaseHold(root)
    let saving = Task { try await owner.flush() }
    _ = try await owner.state()
    let discarding = Task { try await owner.discardPending() }
    hold.release()
    _ = try? await saving.value
    try await discarding.value
    try await edit(owner)
    try await owner.flush()
    #expect(try savedHits(root) == 6)
    try await owner.close()
  }

  // Failure: close released the writer lock or accepted edits before its final write
  // committed, letting another writer in or losing the late edit.
  @Test func closeRefusesEditsAndHoldsTheLockUntilItsFinalWrite() async throws {
    let root = try fixture()
    defer { try? FileManager.default.removeItem(at: root) }
    let owner = try DocumentOwner(url: root)
    try await edit(owner)
    let hold = try Fixtures.DatabaseHold(root)
    let closing = Task { try await owner.close() }
    #expect(await eventually(timeout: .seconds(5)) { (try? await edit(owner)) == nil })
    #expect(Fixtures.isLocked(root))
    hold.release()
    try await closing.value
    #expect(!Fixtures.isLocked(root))
    #expect(try savedHits(root) ?? 0 >= 3)
  }

  // Failure: an undo changed the document without saving it. Oracle: the saved bytes,
  // read back without the owner.
  @Test func undoSavesLikeAnEdit() async throws {
    let root = try fixture()
    defer { try? FileManager.default.removeItem(at: root) }
    let owner = try DocumentOwner(url: root)
    let states = Locked<[UndoState]>([])
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
    #expect(states.value == [UndoState(canUndo: true, canRedo: false), UndoState(canUndo: true, canRedo: true),
      UndoState(canUndo: false, canRedo: true)])
    try await owner.close()
  }

  // Undo covers the open session only: an agent's edit saved while the document was
  // closed is where the next session starts. Oracle: the saved bytes.
  @Test func aReopenedDocumentStartsWithNothingToUndo() async throws {
    let root = try fixture()
    defer { try? FileManager.default.removeItem(at: root) }
    let agent = try DocumentOwner(url: root)
    _ = try await agent.apply(batch: increment)
    try await agent.close()
    let owner = try DocumentOwner(url: root)
    let states = Locked<[UndoState]>([])
    owner.onUndoState = { state in states.modify { $0.append(state) } }
    owner.publishUndoState()
    _ = try await owner.undo()
    try await owner.flush()
    #expect(try savedHits(root) == 3)
    #expect(!states.value.contains { $0.canUndo }, "the window never offers Undo")
    try await owner.close()
  }

  // Failure: a document kept every edit it ever saw. Oracle: closing a session that
  // edited a large document leaves its file smaller, holding no history.
  @Test func closingALargeEditedDocumentTrimsItsHistory() async throws {
    let root = try fixture()
    defer { try? FileManager.default.removeItem(at: root) }
    var seed: UInt64 = 7
    func noise() -> String {
      String((0..<32 * 1024).map { _ in
        seed = seed &* 6364136223846793005 &+ 1442695040888963407
        return Character(UnicodeScalar(97 + UInt8((seed >> 59) % 26)))
      })
    }
    var beforeClose: UInt64 = 0
    for _ in 0..<2 {
      let owner = try DocumentOwner(url: root)
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
      beforeClose = try Fixtures.size(root)
      try await owner.close()
    }
    let afterClose = try Fixtures.size(root)
    #expect(afterClose < beforeClose / 2, "\(afterClose) of \(beforeClose) bytes")
  }

  // Failure: another process holding the database (a backup during Duplicate) must be a
  // definite, retryable failure, never mistaken for a lost reply or a conflict.
  @Test func busyDatabaseIsARetryableFailure() async throws {
    let root = try fixture()
    defer { try? FileManager.default.removeItem(at: root) }
    let owner = try DocumentOwner(url: root)
    try await edit(owner)
    let hold = try Fixtures.DatabaseHold(root)
    await #expect(throws: SaveFailure.busy) { try await owner.flush() }
    hold.release()
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
    let moved = root.deletingLastPathComponent().appendingPathComponent("Moved.slop")
    defer { try? FileManager.default.removeItem(at: root); try? FileManager.default.removeItem(at: moved) }
    let owner = try DocumentOwner(url: root)
    try await edit(owner)
    let hold = try Fixtures.DatabaseHold(root)
    let saving = Task { try await owner.flush() }
    _ = try await owner.state()
    let discarding = Task { try await owner.discardPending() }
    // Discard rejects the old save waiter before waiting for the persistence queue.
    _ = await saving.result
    // The held write and the reload behind it both find the document moved.
    try FileManager.default.moveItem(at: root, to: moved)
    hold.release()
    await #expect(throws: (any Error).self) { try await discarding.value }
    try FileManager.default.moveItem(at: moved, to: root)
    try await edit(owner)
    try await owner.flush()
    #expect(try savedHits(root) == 6)
    try await owner.discardPending()
    try await owner.close()
  }

  // Failure: a discard whose reload failed (a moved file) threw without publishing, so
  // the coordinator, which leaves save failures to the save-failure sheet, showed nothing.
  // Oracle: the failed reload is published as a save failure.
  @Test func aFailedDiscardPublishesItsFailure() async throws {
    let root = try fixture()
    let moved = root.deletingLastPathComponent().appendingPathComponent("Moved.slop")
    defer { try? FileManager.default.removeItem(at: root); try? FileManager.default.removeItem(at: moved) }
    let owner = try DocumentOwner(url: root)
    let statuses = Locked<[DocumentSaveStatus]>([])
    owner.onSaveStatus = { status in statuses.modify { $0.append(status) } }
    try await edit(owner)
    try FileManager.default.moveItem(at: root, to: moved)
    await #expect(throws: SaveFailure.moved) { try await owner.discardPending() }
    #expect(statuses.value.contains(.failed(.moved)))
    try FileManager.default.moveItem(at: moved, to: root)
    try await owner.discardPending()
    try await owner.close()
  }

  @Test func discardDoesNotPublishSaveFailure() async throws {
    let root = try fixture()
    defer { try? FileManager.default.removeItem(at: root) }
    let owner = try DocumentOwner(url: root)
    let statuses = Locked<[DocumentSaveStatus]>([])
    owner.onSaveStatus = { status in statuses.modify { $0.append(status) } }
    try await edit(owner)
    try await owner.discardPending()
    #expect(!statuses.value.contains { if case .failed = $0 { true } else { false } })
    try await owner.close()
  }

  // Failure: a flush admitted while discard restored saved bytes waited for the old
  // publication sequence, which the restored core (sequence 0) never reaches, so the
  // page hung. Oracle: the flush settles at once as replaced, and saving still works.
  @Test func flushDuringDiscardRestoreSettlesAsReplaced() async throws {
    let root = try fixture()
    defer { try? FileManager.default.removeItem(at: root) }
    let owner = try DocumentOwner(url: root)
    owner.attach(view: "page")
    try await edit(owner)
    try await edit(owner)
    // The reload waits behind the held queue while discard is under way.
    let hold = try Fixtures.DatabaseHold(root)
    let discarding = Task { try await owner.discardPending() }
    while true {
      let refused: Bool = await withCheckedContinuation { continuation in
        owner.enqueuePage(.flush(PageFlushRequest()), view: "page") { result in
          if case .failure(let error) = result { continuation.resume(returning: error is OwnerReplaced) }
          else { continuation.resume(returning: false) }
        }
      }
      if refused { break }
      try await Task.sleep(for: .milliseconds(10))
    }
    hold.release()
    try await discarding.value
    try await edit(owner)
    try await owner.flush()
    // Autosave may have saved the first edits before discard began; either way, flush
    // saves exactly what the owner holds.
    #expect(try await savedHits(root) == liveHits(owner))
    try await owner.close()
  }
}
