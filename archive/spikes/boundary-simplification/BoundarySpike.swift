import Foundation
import Darwin
import SQLite3
import Testing
import HitSlopCore
import HitSlopCoreBinding
@testable import HitSlopDocument

@Suite(.serialized) struct BoundarySpike {
  let schema = #"{"kind":"object","properties":{"title":{"kind":"text"},"rows":{"kind":"list","item":{"kind":"object","properties":{"text":{"kind":"text"},"done":{"kind":"boolean"}}}},"hits":{"kind":"counter"}}}"#
  let increment = #"{"intents":[{"type":"increment","path":["hits"],"by":1}]}"#
  func fixture(rows: Int = 0, text: Int = 3) throws -> URL {
    let repository = #filePath.components(separatedBy: "/apps/apple/")[0]
    let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString + ".slop")
    try FileManager.default.copyItem(atPath: repository + "/tests/fixtures/checklist/document", toPath: root.path)
    try Data(schema.utf8).write(to: root.appendingPathComponent("state.schema.json"))
    let initial: [String: Any] = ["title": String(repeating: "x", count: text), "hits": 0,
      "rows": (0..<rows).map { ["$id": String(format: "%032d", $0 + 1), "text": "Row \($0)", "done": false] as [String: Any] }]
    try JSONSerialization.data(withJSONObject: initial).write(to: root.appendingPathComponent("initial.json"))
    return root
  }
  func state(_ owner: DocumentOwner) async throws -> [String: Any] {
    try JSONSerialization.jsonObject(with: Data(try await owner.state().utf8)) as! [String: Any]
  }
  func value(_ owner: DocumentOwner) async throws -> [String: Any] { try await state(owner)["value"] as! [String: Any] }
  func bytes(_ root: URL) throws -> Int { try FileManager.default.attributesOfItem(atPath: root.appendingPathComponent("state/document.sqlite").path)[.size] as! Int }
  func sql(_ root: URL, _ query: String) throws -> Int64 {
    var db: OpaquePointer?, s: OpaquePointer?
    guard sqlite3_open_v2(root.appendingPathComponent("state/document.sqlite").path, &db, SQLITE_OPEN_READONLY, nil) == SQLITE_OK else { throw failure("open") }
    defer { sqlite3_finalize(s); sqlite3_close(db) }
    guard sqlite3_prepare_v2(db, query, -1, &s, nil) == SQLITE_OK, sqlite3_step(s) == SQLITE_ROW else { throw failure("query \(query)") }
    return sqlite3_column_int64(s, 0)
  }
  func record(_ name: String, _ value: Any) throws {
    let repository = #filePath.components(separatedBy: "/apps/apple/")[0]
    let path = URL(fileURLWithPath: repository + "/.hitslop/boundary-\(name).json")
    try JSONSerialization.data(withJSONObject: value, options: [.prettyPrinted, .sortedKeys]).write(to: path)
  }
  func ms(_ start: UInt64) -> Double { Double(DispatchTime.now().uptimeNanoseconds - start) / 1e6 }
  func summary(_ values: [Double]) -> [String: Any] {
    let sorted = values.sorted()
    return ["samples": values, "p50": sorted[sorted.count / 2], "p95": sorted[min(sorted.count - 1, Int(Double(sorted.count) * 0.95))], "max": sorted.last!]
  }

  @Test func repeatedLostAcknowledgementsThenLaterEdits() async throws {
    let root = try fixture(); defer { try? FileManager.default.removeItem(at: root) }
    let owner = try DocumentOwner(package: SlopPackage(rootURL: root))
    _ = try await owner.apply(batch: increment)
    owner.storage.testingPhase = { phase in
      if phase.hasSuffix(":committed") || phase == "metadata" { throw failure("injected lost reply/read") }
    }
    for _ in 0..<8 { await #expect(throws: (any Error).self) { try await owner.flush() } }
    owner.storage.testingPhase = nil
    _ = try await owner.apply(batch: increment)
    try await owner.flush()
    try await owner.close()
    let reopened = try DocumentOwner(package: SlopPackage(rootURL: root))
    #expect(try await value(reopened)["hits"] as? Int == 2)
    try await reopened.close()
    try record("retry", ["bytes": bytes(root), "lostReplies": 8])
  }

  @Test func replicasSurviveDuplicateDeliveryAndRestart() async throws {
    let a = try fixture(), b = try fixture()
    defer { try? FileManager.default.removeItem(at: a); try? FileManager.default.removeItem(at: b) }
    var left = try DocumentOwner(package: SlopPackage(rootURL: a))
    let seed = try await left.spikeCheckpoint(), base = try await left.spikeVersion()
    try await left.close()
    // Copy the durable seed: both replicas must share history, not just initial JSON.
    try FileManager.default.copyItem(at: a.appendingPathComponent("state"), to: b.appendingPathComponent("state"))
    left = try DocumentOwner(package: SlopPackage(rootURL: a))
    var right = try DocumentOwner(package: SlopPackage(rootURL: b))
    _ = try await left.apply(batch: #"{"intents":[{"type":"set","path":["title"],"value":"Lxxx"},{"type":"insert","path":["rows"],"id":"00000000000000000000000000000001","value":{"text":"left","done":false}},{"type":"increment","path":["hits"],"by":2}]}"#)
    _ = try await right.apply(batch: #"{"intents":[{"type":"set","path":["title"],"value":"xxxR"},{"type":"insert","path":["rows"],"id":"00000000000000000000000000000002","value":{"text":"right","done":false}},{"type":"increment","path":["hits"],"by":3}]}"#)
    let u1 = try await left.spikeExport(base), u2 = try await right.spikeExport(base)
    let dependentBase = try await left.spikeVersion()
    _ = try await left.apply(batch: increment)
    let dependent = try await left.spikeExport(dependentBase)
    await #expect(throws: (any Error).self) { try await right.spikeImport(dependent) }
    try await left.spikeImport(u2)
    try await left.spikeImport(u2)
    try await left.flush(); try await left.close()
    try await right.flush(); try await right.close()
    left = try DocumentOwner(package: SlopPackage(rootURL: a))
    right = try DocumentOwner(package: SlopPackage(rootURL: b))
    try await right.spikeImport(u1); try await right.spikeImport(dependent); try await right.spikeImport(u1)
    let updates = try await left.spikeExport(base)
    try await right.spikeImport(updates)
    let oracle = try NativeDocument.open(schemaJson: schema, checkpoint: seed, updates: [u1, u2, dependent])
    let expected = try JSONSerialization.jsonObject(with: Data(oracle.snapshot().utf8)) as! NSDictionary
    for owner in [left, right] {
      let actual = try await state(owner)
      #expect(NSDictionary(dictionary: actual["value"] as! [String: Any]) == (expected["value"] as! NSDictionary))
      #expect((actual["issues"] as! NSArray) == (expected["issues"] as! NSArray))
      #expect((actual["value"] as! [String: Any])["hits"] as? Int == 6)
      try await owner.close()
    }
    try record("replicas", ["passed": true, "restarts": 2, "duplicateDeliveries": 2, "missingDependenciesRejected": true])
  }

  @Test func workloadMeasurements() async throws {
    var results: [[String: Any]] = []
    for (rows, text) in [(1000, 3), (5000, 3), (0, 10000), (0, 100000)] {
      let root = try fixture(rows: rows, text: text); defer { try? FileManager.default.removeItem(at: root) }
      var start = DispatchTime.now().uptimeNanoseconds
      var owner = try DocumentOwner(package: SlopPackage(rootURL: root))
      let open = ms(start)
      var edits: [Double] = [], saves: [Double] = [], exports: [Double] = []
      var snapshotBytes = 0
      for i in 0..<30 {
        start = DispatchTime.now().uptimeNanoseconds
        let batch = rows > 0 ? increment : "{\"intents\":[{\"type\":\"set\",\"path\":[\"title\"],\"value\":\"\(String(repeating: "x", count: text))\(i)\"}]}"
        _ = try await owner.apply(batch: batch); edits.append(ms(start))
        start = DispatchTime.now().uptimeNanoseconds
        snapshotBytes = try await owner.spikeCheckpoint().count; exports.append(ms(start))
        start = DispatchTime.now().uptimeNanoseconds
        try await owner.flush(); saves.append(ms(start))
      }
      start = DispatchTime.now().uptimeNanoseconds
      try await owner.close(); let close = ms(start)
      let written = owner.storage.spikeWrittenBytes, diskMS = owner.storage.spikeWriteMS
      start = DispatchTime.now().uptimeNanoseconds
      owner = try DocumentOwner(package: SlopPackage(rootURL: root)); let reopen = ms(start)
      try await owner.close()
      var usage = rusage(); getrusage(RUSAGE_SELF, &usage)
      results.append(["rows": rows, "text": text, "openMS": open, "reopenMS": reopen, "closeMS": close, "editMS": summary(edits), "flushMS": summary(saves), "snapshotExportMS": summary(exports), "sqliteWriteMS": summary(diskMS), "writtenPayloadBytes": written, "snapshotBytes": snapshotBytes, "databaseBytes": try bytes(root), "processPeakRSSBytes": usage.ru_maxrss])
    }
    try record("workloads", results)
  }

  @Test func historyGrowth() async throws {
    let root = try fixture(); defer { try? FileManager.default.removeItem(at: root) }
    let owner = try DocumentOwner(package: SlopPackage(rootURL: root))
    var checkpoints: [[String: Any]] = []
    for i in 1...10000 {
      _ = try await owner.apply(batch: increment)
      if i % 100 == 0 { try await owner.flush() }
      if [100, 1000, 10000].contains(i) {
        checkpoints.append(["edits": i, "snapshotBytes": try await owner.spikeCheckpoint().count, "databaseBytes": try bytes(root), "writtenPayloadBytes": owner.storage.spikeWrittenBytes])
      }
    }
    try await owner.close()
    try record("history", checkpoints)
  }

  @Test(.enabled(if: ProcessInfo.processInfo.environment["SPIKE_STORAGE_BYTES"] != nil))
  func retryAtScaledStorageLimit() async throws {
    let root = try fixture(); defer { try? FileManager.default.removeItem(at: root) }
    let owner = try DocumentOwner(package: SlopPackage(rootURL: root))
    var rng: UInt64 = 12345
    let title = String((0..<24000).map { _ -> Character in
      rng ^= rng << 13; rng ^= rng >> 7; rng ^= rng << 17
      return Character(UnicodeScalar(65 + Int(rng % 26))!)
    })
    let batch = String(decoding: try JSONSerialization.data(withJSONObject: ["intents": [["type": "set", "path": ["title"], "value": title]]]), as: UTF8.self)
    _ = try await owner.apply(batch: batch)
    owner.storage.testingPhase = { if $0.hasSuffix(":committed") { throw failure("lost acknowledgement") } }
    for _ in 0..<20 { try? await owner.flush() }
    owner.storage.testingPhase = nil
    _ = try await owner.apply(batch: increment)
    try await owner.flush(); try await owner.close()
    let reopened = try DocumentOwner(package: SlopPackage(rootURL: root))
    #expect(try await value(reopened)["title"] as? String == title)
    #expect(try await value(reopened)["hits"] as? Int == 1)
    try await reopened.close()
    try record("scaled-limit", ["limit": Storage.maximumBytes, "databaseBytes": try bytes(root), "writtenPayloadBytes": owner.storage.spikeWrittenBytes])
  }

  @Test func slowSaveEditLatency() async throws {
    let root = try fixture(); defer { try? FileManager.default.removeItem(at: root) }
    let owner = try DocumentOwner(package: SlopPackage(rootURL: root))
    _ = try await owner.apply(batch: increment)
    let gate = StorageGate("SPIKE_PHASE:uncommitted")
    owner.storage.testingPhase = { phase in
      gate.hook(phase)
    }
    let save = Task { try await owner.flush() }
    await gate.reached()
    DispatchQueue.global().asyncAfter(deadline: .now() + .milliseconds(250)) { gate.release() }
    let start = DispatchTime.now().uptimeNanoseconds
    _ = try await owner.apply(batch: increment)
    let delay = ms(start)
    try await save.value
    owner.storage.testingPhase = nil
    try await owner.flush(); try await owner.close()
    try record("slow-save", ["editMS": delay, "injectedSaveMS": 250])
  }
}
