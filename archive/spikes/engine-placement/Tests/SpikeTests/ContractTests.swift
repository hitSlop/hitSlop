import Foundation
import Loro
import Testing

@testable import SpikeCore

// Observable failures: partial rejected edits, lost acknowledged saves, replay
// differences, and competing writers. These are independent native owner boundaries;
// existing Bun tests cannot prove the new Swift interpreter/storage path.
struct ContractTests {
  let directory = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
    .deletingLastPathComponent().deletingLastPathComponent().appendingPathComponent("dist")
  func replica(_ fixture: String = "checklist") throws -> Replica {
    let schema = try JSONValue.decode(
      Data(contentsOf: directory.appendingPathComponent("\(fixture).schema.json")))["root"]
    return try Replica(
      schema: schema,
      snapshot: Data(contentsOf: directory.appendingPathComponent("\(fixture)-1.snapshot")))
  }
  func store(_ bytes: Int = 32 * 1024 * 1024) throws -> (URL, Store) {
    let root = FileManager.default.temporaryDirectory.appendingPathComponent(
      "placement-test-" + UUID().uuidString)
    return (root, try Store(root: root, maximumBytes: bytes))
  }
  func seed(_ store: Store, _ r: Replica) throws {
    _ = try store.write(generation: "0", checkpoint: r.snapshot(), schemaKey: "fixture")
  }
  @Test func batchRejectsWithoutChangingStateOrVersion() throws {
    let r = try replica()
    let before = try r.read([])
    let version = r.version()
    #expect(throws: Error.self) {
      try r.apply([
        .splice(SpliceOp(path: [.string("title")], index: 0, deleteCount: 0, text: "bad")),
        .set(SetOp(path: [.string("title")], value: .bool(true))),
      ])
    }
    #expect(try r.read([]) == before)
    #expect(r.version() == version)
  }
  @Test func identitySurvivesMoveAndRestart() throws {
    let r = try replica()
    _ = try r.apply([
      .insert(
        InsertOp(
          path: [.string("rows")], index: 0, id: "inserted",
          value: .object(["text": .string("new"), "done": .bool(false)])))
    ])
    _ = try r.apply([
      .move(MoveOp(path: [.string("rows")], from: 0, to: 1)),
      .set(
        SetOp(
          path: [.string("rows"), .object(["id": .string("inserted")]), .string("done")],
          value: .bool(true))),
    ])
    let reopened = try Replica(schema: r.schema, snapshot: r.snapshot())
    #expect(try reopened.read([.string("rows")]).array.last?["$id"] == .string("inserted"))
    #expect(try reopened.read([.string("rows")]).array.last?["done"] == .bool(true))
  }
  @Test func failedSaveRetainsEditsAndOwnership() throws {
    let r = try replica()
    let (root, s) = try store()
    defer { try? FileManager.default.removeItem(at: root) }
    try seed(s, r)
    let session = try Session(store: s, schema: r.schema)
    _ = try session.apply([
      .splice(SpliceOp(path: [.string("title")], index: 0, deleteCount: 0, text: "saved "))
    ])
    s.beforeCommit = { throw SpikeError("disk failure") }
    #expect(throws: Error.self) { try session.close() }
    #expect(throws: Error.self) { try Store(root: root) }
    #expect(try session.replica.read([.string("title")]).string?.hasPrefix("saved ") == true)
    s.beforeCommit = nil
    try session.close()
    let next = try Store(root: root)
    defer { next.close() }
    #expect(
      try Session(store: next, schema: r.schema).replica.read([.string("title")]).string?.hasPrefix(
        "saved ") == true)
  }
  @Test func ambiguousCommitDoesNotDuplicateAnEdit() throws {
    let r = try replica()
    let (root, s) = try store()
    defer {
      s.close()
      try? FileManager.default.removeItem(at: root)
    }
    try seed(s, r)
    let session = try Session(store: s, schema: r.schema)
    _ = try session.apply([
      .splice(SpliceOp(path: [.string("title")], index: 0, deleteCount: 0, text: "once "))
    ])
    s.afterCommit = { throw SpikeError("reply lost") }
    try session.flush()
    s.afterCommit = nil
    try session.flush()
    #expect(try s.metadata().updateRows == 1)
    #expect(
      try Session(store: s, schema: r.schema).replica.read([.string("title")]).string
        == "once Engine placement")
  }
  @Test func checkpointFailurePreservesLastDurableState() throws {
    let r = try replica()
    let (root, s) = try store()
    defer {
      s.close()
      try? FileManager.default.removeItem(at: root)
    }
    try seed(s, r)
    let before = try s.load()
    s.beforeCommit = { throw SpikeError("checkpoint I/O failure") }
    #expect(throws: Error.self) {
      try s.write(generation: "1", checkpoint: Data([1, 2, 3]), schemaKey: "fixture")
    }
    #expect(try s.load().checkpoint == before.checkpoint)
    #expect(try s.load().generation == before.generation)
  }
  @Test func pendingDependenciesAndDuplicatesConverge() throws {
    let a = try replica()
    let b = try replica()
    let version = a.version()
    _ = try a.apply([
      .splice(SpliceOp(path: [.string("title")], index: 0, deleteCount: 0, text: "first"))
    ])
    let first = try a.updates(since: version)
    let v2 = a.version()
    _ = try a.apply([
      .splice(SpliceOp(path: [.string("title")], index: 5, deleteCount: 0, text: "second"))
    ])
    let second = try a.updates(since: v2)
    try b.receive(second)
    try b.receive(first)
    try b.receive(second)
    #expect(try a.read([]) == b.read([]))
    #expect(a.version() == b.version())
  }
  @Test func counterCheckpointPreservesAcceptedNumber() throws {
    let r = try replica("mixed")
    let (root, s) = try store()
    defer {
      s.close()
      try? FileManager.default.removeItem(at: root)
    }
    try seed(s, r)
    let session = try Session(store: s, schema: r.schema)
    for n in [1e16, -1e16, 1.0] {
      _ = try session.apply([.increment(IncrementOp(path: [.string("taps")], amount: n))])
    }
    let accepted = try session.replica.read([.string("taps")])
    try session.flush()
    #expect(try Session(store: s, schema: r.schema).replica.read([.string("taps")]) == accepted)
    #expect(try s.metadata().updateRows == 0)
  }
  @Test func unicodeSpliceUsesUTF16Positions() throws {
    let r = try replica()
    _ = try r.apply([
      .splice(SpliceOp(path: [.string("title")], index: 0, deleteCount: 16, text: "A🦊B"))
    ])
    _ = try r.apply([
      .splice(SpliceOp(path: [.string("title")], index: 1, deleteCount: 2, text: "é"))
    ])
    #expect(try r.read([.string("title")]) == .string("AéB"))
  }
  @Test func strictWritesRefuseAnomalousContainersWithoutRepair() throws {
    let r = try replica()
    try r.doc.getMap(id: "data").insert(
      key: "title", v: LoroValue.map(value: ["kind": .string(value: "Text")]))
    r.doc.commit()
    let before = r.version()
    #expect(try r.read([.string("title")]) == .string(""))
    #expect(throws: Error.self) {
      try r.apply([
        .splice(SpliceOp(path: [.string("title")], index: 0, deleteCount: 0, text: "no"))
      ])
    }
    #expect(r.version() == before)
  }
  @Test func capacityFailureDoesNotAdvanceGeneration() throws {
    let (root, s) = try store(10)
    defer {
      s.close()
      try? FileManager.default.removeItem(at: root)
    }
    #expect(throws: Error.self) {
      try s.write(generation: "0", checkpoint: Data(repeating: 0, count: 11))
    }
    #expect(try s.metadata().generation == "0")
  }
}
