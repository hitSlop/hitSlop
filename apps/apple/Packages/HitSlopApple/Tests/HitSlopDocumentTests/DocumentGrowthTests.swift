import Foundation
import HitSlopCore
import HitSlopCoreBinding
import Testing
import HitSlopTestSupport
@testable import HitSlopDocument

/// A measurement, not a CI performance assertion. Every edit and save uses the production owner.
@Suite(.serialized) struct DocumentGrowthTests {
  private func json(_ value: Any) throws -> String {
    String(decoding: try JSONSerialization.data(withJSONObject: value, options: [.sortedKeys]), as: UTF8.self)
  }
  private func frame(_ owner: DocumentOwner) async throws -> [String: Any] {
    try JSONSerialization.jsonObject(with: Data(await owner.state().utf8)) as! [String: Any]
  }
  private func text(_ owner: DocumentOwner, request: String) async throws -> DocumentOwner.TextEdit {
    try await withCheckedThrowingContinuation { continuation in
      owner.enqueuePage(.text(request), view: "growth") { result in
        do {
          guard case .text(let edit) = try result.get() else { throw OwnerError.rejected("Missing text reply") }
          continuation.resume(returning: edit)
        } catch { continuation.resume(throwing: error) }
      }
    }
  }

  @Test(.enabled(if: ProcessInfo.processInfo.environment["HITSLOP_GROWTH_INPUT"] != nil))
  func documentGrowth() async throws {
    let env = ProcessInfo.processInfo.environment
    let input = URL(fileURLWithPath: env["HITSLOP_GROWTH_INPUT"]!)
    let days = Int(env["HITSLOP_GROWTH_DAYS"] ?? "365")!
    let repo = #filePath.components(separatedBy: "/apps/apple/")[0]
    let folder = try Fixtures.folder()
    defer { try? FileManager.default.removeItem(at: folder) }
    // `bun run bench:growth` names a dated evidence file.
    let output = URL(fileURLWithPath: env["HITSLOP_GROWTH_OUTPUT"] ?? repo + "/.hitslop/evidence/document-growth.json")
    var records: [[String: Any]] = []
    func report() throws {
      try FileManager.default.createDirectory(at: output.deletingLastPathComponent(), withIntermediateDirectories: true)
      try JSONSerialization.data(withJSONObject: [
        "coreBuild": DocumentOwner.coreBuildID, "build": "Debug Swift test harness, production Rust binding",
        "baseCommit": env["HITSLOP_GROWTH_COMMIT"] ?? "unknown", "measuredAt": ISO8601DateFormatter().string(from: Date()),
        "candidate": "Working tree measurement; harness and concurrent changes may be uncommitted",
        "requestedDays": days, "complete": records.count == 3,
        "releaseBlocked": records.contains { $0["fullAtDay"] != nil || $0["stoppedError"] != nil },
        "method": "Seed 120029. Synthetic heavy use; accelerated idle periods end with owner.flush, normal checkpoint thresholds unchanged. Each day is one session: the daily close trims history as the app does (a document over 4 MiB keeps none), and reopen checks exact values. Day samples are taken before the close; closedBytes after it. Bytes are measured, lifetimes beyond the run are estimates. No forced compaction or limit changes. UI/preview rendering is excluded.",
        "workloads": [
          "doodle-board": "100 strokes/day, 100 pointer samples/stroke through authored StrokeSamples and strokePath; one insert of the final geometry per stroke, as the app writes at stroke end, flush per stroke; clear every 25, flush clear. The stroke in progress is local, not a document commit.",
          "pixel-art": "20 repaints/day of all 256 cells, one 256-intent drag batch and flush per repaint; rotating palette prevents no-op writes.",
          "morning-pages": "5000 ASCII characters/day in 20-character text-binding requests, revise first 1000 characters in 20-character requests; flush every 10 requests (simulated typing pause), retain entries. Deterministic prose varies by day."
        ], "results": records,
      ], options: [.prettyPrinted, .sortedKeys]).write(to: output, options: .atomic)
    }
    for slug in ["doodle-board", "pixel-art", "morning-pages"] {
      let root = try Fixtures.document(
        from: URL(fileURLWithPath: repo + "/generated/templates/" + slug + ".slop"), at: folder.appendingPathComponent(slug + ".slop"))
      var owner = try DocumentOwner(url: root)
      owner.attach(view: "growth")
      let initialMeta = try owner.storageQueue.sync { try owner.store.metadata() }
      let initialBytes = Int64(initialMeta.checkpointBytes + initialMeta.updateBytes)
      var samples: [[String: Any]] = []
      var commits = 0, intents = 0, saves = 0, saveMS = 0.0, completedDays = 0
      var fullAt: Double?
      var stoppedError: String?
      var isClosed = false
      var finalMeta = initialMeta
      var progress = 0.0
      var lastSavedValue = try json(try await frame(owner)["value"]!)
      let started = Date()
      func apply(_ operations: [[String: Any]]) async throws -> DocumentOwner.Applied {
        let reply = try await owner.apply(batch: json(["intents": operations]))
        commits += 1; intents += operations.count
        return reply
      }
      func flush() async throws {
        let start = Date()
        try await owner.flush()
        saveMS += Date().timeIntervalSince(start) * 1000; saves += 1
      }
      for day in 1...days {
        do {
          if slug == "doodle-board" {
            let strokes = try JSONSerialization.jsonObject(with: Data(contentsOf: input.appendingPathComponent("strokes/\(day).json"))) as! [String]
            var ids: [String] = []
            for (index, stroke) in strokes.enumerated() {
              progress = Double(day - 1) + Double(index + 1) / 100
              let inserted = try await apply([["type": "insert", "path": ["strokes"], "value": ["geometry": stroke, "color": "#222222"]]])
              ids.append(inserted.ids[0])
              try await flush()
              if ids.count == 25 {
                _ = try await apply(ids.map { ["type": "remove", "path": ["strokes"], "id": $0] })
                try await flush(); ids.removeAll()
              }
            }
          } else if slug == "pixel-art" {
            let colors = ["#0f380f", "#306230", "#8bac0f", "#9bbc0f"]
            for cycle in 0..<20 {
              progress = Double(day - 1) + Double(cycle + 1) / 20
              _ = try await apply((0..<256).map { index in ["type": "set", "path": ["pixels", ["index": index]], "value": colors[(index + cycle + day) % 4]] })
              try await flush()
            }
          } else {
            let key = "growth-\(day)"
            _ = try await apply([["type": "set", "path": ["entries", key], "value": ["date": key, "text": "", "completedAt": ""]], ["type": "set", "path": ["currentKey"], "value": key]])
            var base = try await frame(owner)["version"] as! String, from = ""
            let prose = (0..<120).map { "Day \(day), thought \($0): I walked outside and wrote about the light. " }.joined()
            let full = String(prose.prefix(5000))
            for edit in 1...300 {
              progress = Double(day - 1) + Double(edit) / 300
              let to: String
              if edit <= 250 { to = String(full.prefix(edit * 20)) }
              else {
                let offset = (edit - 251) * 20
                to = String(from.prefix(offset)) + String(from.dropFirst(offset).prefix(20)).uppercased() + String(from.dropFirst(offset + 20))
              }
              let reply = try await text(owner, request: json(["base": base, "path": ["entries", key, "text"], "from": from, "to": to, "selectionStart": min(edit * 20, 5000), "selectionEnd": min(edit * 20, 5000)]))
              base = reply.authored; from = to; commits += 1; intents += 1
              if edit % 10 == 0 { try await flush() }
            }
          }
          try await flush()
          lastSavedValue = try json(try await frame(owner)["value"]!)
          let meta = try owner.storageQueue.sync { try owner.store.metadata() }
          finalMeta = meta
          let diskBytes = (try FileManager.default.attributesOfItem(atPath: root.path)[.size] as! NSNumber).int64Value
          samples.append(["day": day, "commits": commits, "intents": intents, "checkpointBytes": meta.checkpointBytes, "updateBytes": meta.updateBytes, "updateRows": meta.rows, "databaseBytes": diskBytes, "retainedValueJSONBytes": lastSavedValue.utf8.count, "reopenVerified": false])
          try await owner.close()
          isClosed = true
          let start = Date()
          owner = try DocumentOwner(url: root)
          isClosed = false
          let reopenMS = Date().timeIntervalSince(start) * 1000
          owner.attach(view: "growth")
          let closed = try owner.storageQueue.sync { try owner.store.metadata() }
          samples[samples.count - 1]["closedBytes"] = closed.checkpointBytes + closed.updateBytes
          #expect(try await json(frame(owner)["value"]!) == lastSavedValue)
          samples[samples.count - 1]["reopenMS"] = reopenMS
          samples[samples.count - 1]["reopenVerified"] = true
          completedDays = day
          print("Growth \(slug): day \(day), \(meta.checkpointBytes + meta.updateBytes) bytes, \(commits) commits")
        } catch SaveFailure.full {
          fullAt = progress
          // Retain the saved document; discard only the unsaved tail of this disposable workload.
          try await owner.discardPending()
          lastSavedValue = try json(try await frame(owner)["value"]!)
          finalMeta = try owner.storageQueue.sync { try owner.store.metadata() }
          print("Growth \(slug): FULL at day \(progress)")
          break
        } catch {
          stoppedError = error.localizedDescription
          print("Growth \(slug): stopped at day \(progress): \(error)")
          if !isClosed { try? await owner.close() }
          break
        }
      }
      if stoppedError == nil {
        try await owner.close()
        let reopened = try DocumentOwner(url: root)
        #expect(try await json(frame(reopened)["value"]!) == lastSavedValue)
        finalMeta = try reopened.storageQueue.sync { try reopened.store.metadata() }
        try await reopened.close()
      }
      if fullAt != nil || stoppedError != nil {
        let evidence = URL(fileURLWithPath: repo + "/.hitslop/evidence/document-growth/" + UUID().uuidString)
        try FileManager.default.createDirectory(at: evidence, withIntermediateDirectories: true)
        try FileManager.default.copyItem(at: root, to: evidence.appendingPathComponent(slug + ".slop"))
        print("Retained reproduction: \(evidence.path)/\(slug).slop")
      }
      let measuredBytes = Int64(finalMeta.checkpointBytes + finalMeta.updateBytes)
      var record: [String: Any] = ["template": slug, "completedDays": completedDays, "commits": commits, "intents": intents, "flushes": saves, "meanFlushMS": saveMS / Double(max(saves, 1)), "elapsedSeconds": Date().timeIntervalSince(started), "initialSavedBytes": initialBytes, "finalSavedBytes": measuredBytes, "netBytesPerAcceptedCommit": Double(measuredBytes - initialBytes) / Double(max(commits, 1)), "samples": samples]
      record["maxSavedBytes"] = samples.compactMap { sample -> UInt64? in
        guard let checkpoint = sample["checkpointBytes"] as? UInt64, let updates = sample["updateBytes"] as? UInt64 else { return nil }
        return checkpoint + updates
      }.max() ?? UInt64(measuredBytes)
      if let fullAt { record["fullAtDay"] = fullAt }
      else if stoppedError == nil { record["projectedDaysToLimitLinear"] = Double(Limits.storageBytes) / Double(max(measuredBytes, 1)) * Double(completedDays) }
      if let stoppedError { record["stoppedError"] = stoppedError; record["stoppedAtDay"] = progress }
      records.append(record)
      try report()
    }
  }
}
