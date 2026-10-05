import AppKit
import Foundation
import HitSlopCore
import HitSlopTestSupport
import Testing
@testable import HitSlopDocument
@testable import HitSlopHost

@Suite(.serialized) struct ThemeDragBenchmarkTests {
  @Test(.enabled(if: ProcessInfo.processInfo.environment["HITSLOP_BENCH_THEME"] == "1"))
  @MainActor func syntheticPaletteGesture() async throws {
    _ = NSApplication.shared
    var measurements: [[String: Any]] = []
    for rows in [10, 1000] { measurements.append(try await measure(rows: rows)) }
    let output = Fixtures.repository.appendingPathComponent("docs/evidence/theme-drag-2026-10-04.json")
    try JSONSerialization.data(withJSONObject: ["measurements": measurements], options: [.prettyPrinted, .sortedKeys]).write(to: output)
    print("Theme drag evidence: \(output.path)")
  }

  @MainActor private func measure(rows: Int) async throws -> [String: Any] {
    let folder = try Fixtures.folder()
    defer { try? FileManager.default.removeItem(at: folder) }
    let stage = try Fixtures.nativeStage()
    let updates = 60
    try Fixtures.updateApp(stage) {
      $0["initial"] = ["title": "Theme gesture measurement", "tasks": (0..<rows).map {
        ["text": "Task \($0)", "done": false, "archived": false] as [String: Any]
      }]
    }
    let document = try Fixtures.document(stage: stage, at: folder.appendingPathComponent("theme.slop"))
    let controller = try await SlopDocumentWindowController.open(url: document)
    controller.showWindow(nil)
    await controller.waitForPresentation()
    let session = controller.session
    do {
      try await session.waitUntilReady()
      let startColor = try #require(try await session.webView.callAsyncJavaScript("""
        globalThis.__themeDrag = [];
        const root = document.documentElement;
        new MutationObserver(() => {
          const value = getComputedStyle(root).getPropertyValue('--slop-accent').trim();
          if (globalThis.__themeDrag.at(-1)?.value !== value)
            globalThis.__themeDrag.push({ value, epochMS: performance.timeOrigin + performance.now() });
        }).observe(root, { attributes: true, attributeFilter: ['style'] });
        return getComputedStyle(root).getPropertyValue('--slop-accent').trim();
        """, arguments: [:], in: nil, contentWorld: .page) as? String)
      #expect(!startColor.isEmpty)

      var sent: [(color: String, epochMS: Double, uptime: TimeInterval)] = []
      var accepted: [[String: Any]] = [], failures: [String] = []
      let clock = ContinuousClock(), start = clock.now
      session.beginThemeGesture()
      for index in 0..<updates {
        try await clock.sleep(until: start.advanced(by: .milliseconds(index * 16)))
        let color = String(format: "#%02x%02x%02x", index * 4, 255 - index * 4, 128)
        let uptime = ProcessInfo.processInfo.systemUptime
        sent.append((color, Date().timeIntervalSince1970 * 1000, uptime))
        session.changeTheme(.set(["accent": color])) { result in
          switch result {
          case .success(let sequence):
            accepted.append(["index": index, "sequence": sequence,
              "latencyMS": (ProcessInfo.processInfo.systemUptime - uptime) * 1000])
          case .failure(let error): failures.append(error.localizedDescription)
          }
        }
      }
      session.endThemeGesture()
      try await session.flush()
      let deadline = clock.now.advanced(by: .seconds(5))
      while accepted.count + failures.count < updates && clock.now < deadline {
        try await Task.sleep(for: .milliseconds(5))
      }
      let finalColor = try #require(sent.last?.color)
      try await waitForColor(finalColor, session: session)
      let observed = try #require(try await session.webView.callAsyncJavaScript(
        "return globalThis.__themeDrag", arguments: [:], in: nil, contentWorld: .page) as? [[String: Any]])
      var samples: [[String: Any]] = []
      var next = 0
      for entry in observed {
        guard let color = entry["value"] as? String,
          let index = sent.firstIndex(where: { $0.color == color }), index >= next,
          let epochMS = (entry["epochMS"] as? NSNumber)?.doubleValue else { continue }
        samples.append(["index": index, "value": color, "coversUpdates": index - next + 1,
          "matchingUpdateMS": epochMS - sent[index].epochMS,
          "earliestCoveredUpdateMS": epochMS - sent[next].epochMS])
        next = index + 1
      }
      #expect(failures.isEmpty)
      #expect(accepted.count == updates)
      #expect(next == updates, "The page must observe the last submitted color")

      // Use the same page flush + owner Undo route as the native Edit menu.
      try await session.undo()
      try await session.flush()
      try await waitForColor(startColor, session: session)
      let theme = try await session.owner.loadTheme()
      let effective = try JSONDecoder().decode([String: String].self, from: Data(theme.state.effective.utf8))
      #expect(effective["accent"] == startColor, "One Undo restores the whole gesture")

      let evidence: [String: Any] = [
        "date": "2026-10-04", "build": "Debug native test bundle", "coreBuild": DocumentOwner.coreBuildID,
        "platform": ProcessInfo.processInfo.operatingSystemVersionString,
        "method": "Synthetic palette gesture through DocumentSession.beginThemeGesture/changeTheme/endThemeGesture, shared Rust owner, ordered publications, and a shown Checklist WebKit page. 60 unique accent colors scheduled at 16 ms intervals with ContinuousClock; no await of acceptance between submissions. Native latency uses monotonic uptime through the main-actor acceptance callback. Page latency matches computed CSS values from a MutationObserver using JavaScript performance epoch timestamps and native Date submission timestamps; this cross-clock measurement has approximately millisecond precision and includes observer/test overhead. Coalesced samples also report the earliest covered submission. Flush, final computed CSS, and one native Undo are verified. This is not a physical color-picker, IME, frame-presentation, or release-build measurement.",
        "rows": rows, "requestedUpdates": updates, "cadenceMS": 16,
        "acceptedUpdates": accepted.count, "observedColors": samples.count, "failures": failures,
        "submissionOffsetsMS": sent.map { ($0.uptime - sent[0].uptime) * 1000 },
        "acceptanceMS": summary(accepted.compactMap { $0["latencyMS"] as? Double }),
        "pageMatchingUpdateMS": summary(samples.compactMap { $0["matchingUpdateMS"] as? Double }),
        "pageEarliestCoveredUpdateMS": summary(samples.compactMap { $0["earliestCoveredUpdateMS"] as? Double }),
        "acceptanceSamples": accepted, "pageSamples": samples,
        "startColor": startColor, "finalColor": finalColor, "singleUndoRestoredStart": effective["accent"] == startColor,
      ]
      try await controller.closeDocument()
      return evidence
    } catch {
      try? await controller.closeDocument()
      throw error
    }
  }

  @MainActor private func waitForColor(_ expected: String, session: DocumentSession) async throws {
    let clock = ContinuousClock(), deadline = ContinuousClock.now.advanced(by: .seconds(5))
    var value: String?
    repeat {
      value = try await session.webView.callAsyncJavaScript(
        "return getComputedStyle(document.documentElement).getPropertyValue('--slop-accent').trim()",
        arguments: [:], in: nil, contentWorld: .page) as? String
      if value == expected { return }
      try await Task.sleep(for: .milliseconds(5))
    } while clock.now < deadline
    throw SlopFailure("Theme did not reach the page: expected \(expected), got \(value ?? "nil")")
  }

  private func summary(_ samples: [Double]) -> [String: Double] {
    let values = samples.sorted()
    guard !values.isEmpty else { return [:] }
    return ["p50": values[(values.count - 1) / 2],
      "p95": values[min(values.count - 1, Int(ceil(Double(values.count) * 0.95)) - 1)],
      "max": values.last!]
  }
}
