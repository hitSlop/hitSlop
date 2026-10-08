import Foundation
import HitSlopCore
import HitSlopTestSupport
import Testing

@testable import HitSlopDocument

/// A measurement, not a CI timing assertion. The active conformance schema stores
/// synthetic serialized stroke geometry in row text; every edit uses the Rust owner.
@Suite(.serialized) struct DocumentGrowthTests {
  @Test(.enabled(if: ProcessInfo.processInfo.environment["HITSLOP_GROWTH"] == "1"))
  func documentGrowth() async throws {
    let env = ProcessInfo.processInfo.environment
    let days = Int(env["HITSLOP_GROWTH_DAYS"] ?? "30")!
    let folder = try Fixtures.folder()
    defer { try? FileManager.default.removeItem(at: folder) }
    let output = URL(
      fileURLWithPath: env["HITSLOP_GROWTH_OUTPUT"] ?? Fixtures.repository.path + "/docs/evidence/document-growth.json")
    var records: [[String: Any]] = []
    func report() throws {
      try FileManager.default.createDirectory(at: output.deletingLastPathComponent(), withIntermediateDirectories: true)
      try JSONSerialization.data(
        withJSONObject: [
          "coreBuild": DocumentOwner.coreBuildID,
          "baseCommit": env["HITSLOP_GROWTH_COMMIT"] ?? "unknown",
          "measuredAt": ISO8601DateFormatter().string(from: Date()),
          "candidate": "Working tree; Debug Swift harness with production Rust owner",
          "requestedDays": days, "complete": records.count == 2,
          "method":
            "Active checklist conformance fixture. Seed120029; 100 strokes per simulated day, 2048 points per stroke serialized as SVG path text, flush each insert, remove each group of25 and flush. Equal workloads compare daily close/reopen against one continuously open owner. Normal automatic checkpoint and retention policy unchanged; no forced compaction. Read-only saved snapshots verify exact values before close; no UI. State JSON is transient verification only. No extrapolated lifetime claim.",
          "results": records,
        ], options: [.prettyPrinted, .sortedKeys]
      ).write(to: output, options: .atomic)
    }
    for dailyClose in [true, false] {
      let name = dailyClose ? "daily-close" : "single-open"
      let root = try Fixtures.document(at: folder.appendingPathComponent(name + ".slop"))
      var owner = try DocumentOwner(url: root)
      var seed: UInt32 = 120029
      func coordinate() -> String {
        seed = 1_664_525 &* seed &+ 1_013_904_223
        return String(format: "%.2f", Double(seed % 100000) / 100)
      }
      var samples: [[String: Any]] = []
      var commits = 0
      var flushes = 0
      var saveMS = 0.0
      var generatedBytes = 0
      let started = Date()
      func flush() async throws {
        let start = Date()
        try await owner.flush()
        saveMS += Date().timeIntervalSince(start) * 1000
        flushes += 1
      }
      var failure: String?
      do {
        for day in 1...days {
          var ids: [String] = []
          for _ in 0..<100 {
            let stroke = "M" + (0..<2048).map { _ in coordinate() + "," + coordinate() }.joined(separator: "L")
            generatedBytes += stroke.utf8.count
            let reply = try await owner.apply(
              batch: Fixtures.json([
                "intents": [
                  [
                    "type": "insert", "path": ["rows"], "value": ["text": stroke, "done": false],
                  ]
                ]
              ]))
            commits += 1
            ids.append(try #require(reply.ids.first))
            try await flush()
            if ids.count == 25 {
              _ = try await owner.apply(
                batch: Fixtures.json([
                  "intents": ids.map {
                    ["type": "remove", "path": ["rows"], "id": $0] as [String: Any]
                  }
                ]))
              commits += 1
              ids.removeAll()
              try await flush()
            }
          }
          let expected = try Fixtures.json(Fixtures.object(await owner.state())["value"]!)
          let meta = try Fixtures.stored(root)
          // Opening a snapshot does not end the writer's session or trim its history.
          let saved = try DocumentOwner(url: root, mode: .snapshot)
          #expect(try Fixtures.json(Fixtures.object(await saved.state())["value"]!) == expected)
          try await saved.close()
          var sample: [String: Any] = [
            "day": day, "commits": commits, "generatedGeometryBytes": generatedBytes,
            "checkpointBytes": meta.checkpointBytes, "updateBytes": meta.updateBytes,
            "updateRows": meta.rows, "savedValueJSONBytes": expected.utf8.count,
            "savedValueVerified": true,
          ]
          if dailyClose {
            try await owner.close()
            let reopen = Date()
            owner = try DocumentOwner(url: root)
            sample["reopenMS"] = Date().timeIntervalSince(reopen) * 1000
            #expect(try Fixtures.json(Fixtures.object(await owner.state())["value"]!) == expected)
            let closed = try Fixtures.stored(root)
            sample["closedBytes"] = closed.checkpointBytes + closed.updateBytes
          }
          samples.append(sample)
          print(
            "Growth \(name): day \(day), \(meta.checkpointBytes + meta.updateBytes) saved bytes, \(generatedBytes) geometry bytes, \(commits) commits"
          )
        }
      } catch {
        failure = error.localizedDescription
        print("Growth \(name) stopped: \(error)")
      }
      do { try await owner.close() } catch { failure = failure ?? error.localizedDescription }
      let final = try Fixtures.stored(root)
      var record: [String: Any] = [
        "workload": name, "completedDays": samples.count, "commits": commits,
        "flushes": flushes, "meanFlushMS": saveMS / Double(max(flushes, 1)),
        "elapsedSeconds": Date().timeIntervalSince(started), "samples": samples,
        "finalClosedBytes": final.checkpointBytes + final.updateBytes,
        "maxSavedBytesBeforeClose": samples.map {
          ($0["checkpointBytes"] as? UInt64 ?? 0) + ($0["updateBytes"] as? UInt64 ?? 0)
        }.max() ?? 0,
      ]
      if let failure { record["stoppedError"] = failure }
      records.append(record)
      try report()
      #expect(failure == nil)
    }
  }
}
