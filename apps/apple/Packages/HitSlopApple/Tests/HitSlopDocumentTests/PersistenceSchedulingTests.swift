import Foundation
import SQLite3
import HitSlopCore
import HitSlopCoreBinding
import Testing
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
    try FileManager.default.copyItem(atPath: repository + "/tests/fixtures/4-1/document", toPath: root.path)
    let spec = try JSONSerialization.jsonObject(with: Data(contentsOf: URL(fileURLWithPath: repository + "/crates/hitslop-core/fixtures/checklist.json"))) as! [String: Any]
    for (file, key) in [("state.schema.json", "schema"), ("initial.json", "initial")] {
      try JSONSerialization.data(withJSONObject: spec[key]!).write(to: root.appendingPathComponent(file))
    }
    return root
  }

  /// The saved value, decoded from SQLite without the owner (snapshot mode takes no lock).
  func savedHits(_ root: URL) throws -> Int? {
    let storage = try Storage(root: root, mode: .snapshot)
    defer { storage.close() }
    let loaded = try storage.load()
    let core = try NativeDocument.open(schemaJson: loaded.schemaKey!, checkpoint: loaded.checkpoint!, updates: loaded.updates)
    let frame = try JSONSerialization.jsonObject(with: Data(core.snapshot().utf8)) as! [String: Any]
    return (frame["value"] as? [String: Any])?["hits"] as? Int
  }

  func edit(_ owner: DocumentOwner) async throws {
    _ = try await owner.apply(batch: increment)
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

  // Failure: a slow SQLite write blocked every edit, and a flush that joined an in-flight
  // write could be acknowledged by it although its edits were not in those bytes.
  @Test func editsProceedWhileASaveIsInFlightAndFlushCoversThem() async throws {
    let root = try fixture()
    defer { try? FileManager.default.removeItem(at: root) }
    let owner = try DocumentOwner(package: SlopPackage(rootURL: root))
    try await edit(owner)
    let gate = StorageGate("append:uncommitted")
    owner.storage.testingPhase = gate.hook
    let first = Task { try await owner.flush() }
    await gate.reached()
    let proceeded = await finishes(within: 2) { try await edit(owner) }
    let second = Task { try await owner.flush() }
    gate.release()
    try await first.value
    try await second.value
    owner.storage.testingPhase = nil
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
    owner.storage.testingPhase = gate.hook
    let saving = Task { try await owner.flush() }
    await gate.reached()
    let discarding = Task { try await owner.discardPending() }
    gate.release()
    _ = try? await saving.value
    try await discarding.value
    owner.storage.testingPhase = nil
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
    owner.storage.testingPhase = gate.hook
    let closing = Task { try await owner.close() }
    await gate.reached()
    await #expect(throws: (any Error).self) { try await edit(owner) }
    #expect(throws: DocumentWriterLock.Busy.self) { _ = try DocumentWriterLock(root: root) }
    gate.release()
    try await closing.value
    let lock = try DocumentWriterLock(root: root)
    lock.close()
    #expect(try savedHits(root) == 3)
  }

  // Failure: another process holding the database (a backup during Duplicate) must be a
  // definite, retryable failure, never mistaken for a lost reply or a conflict.
  @Test func busyDatabaseIsARetryableFailure() async throws {
    let root = try fixture()
    defer { try? FileManager.default.removeItem(at: root) }
    let owner = try DocumentOwner(package: SlopPackage(rootURL: root))
    try await edit(owner)
    var reader: OpaquePointer?
    #expect(sqlite3_open(root.appendingPathComponent("state/document.sqlite").path, &reader) == SQLITE_OK)
    #expect(sqlite3_exec(reader, "BEGIN EXCLUSIVE", nil, nil, nil) == SQLITE_OK)
    await #expect(throws: SaveFailure.busy) { try await owner.flush() }
    #expect(sqlite3_exec(reader, "COMMIT", nil, nil, nil) == SQLITE_OK)
    sqlite3_close_v2(reader)
    try await owner.flush()
    #expect(try savedHits(root) == 3)
    try await owner.close()
  }
}
