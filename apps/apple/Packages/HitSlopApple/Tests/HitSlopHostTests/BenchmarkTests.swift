import AppKit
import Darwin
import Foundation
import Testing
import HitSlopTestSupport
import HitSlopCore
@testable import HitSlopDocument
import WebKit

@testable import HitSlopHost

private final class PublicationTimes: @unchecked Sendable {
  private let lock = NSLock()
  private var values: [Int: Double] = [:]
  func record(_ publication: String) {
    let now = Date().timeIntervalSince1970 * 1000
    guard let frame = try? JSONSerialization.jsonObject(with: Data(publication.utf8)) as? [String: Any],
      let sequence = frame["sequence"] as? Int else { return }
    lock.withLock { values[sequence] = now }
  }
  func arrivalCosts(_ arrivals: [[String: Any]]) -> [Double] {
    lock.withLock {
      arrivals.compactMap { entry in
        guard let sequence = entry["sequence"] as? Int, let arrival = entry["epochMS"] as? Double,
          let commit = values[sequence] else { return nil }
        return arrival - commit
      }.sorted()
    }
  }
}

@Suite(.serialized) struct BenchmarkTests {
  @Test(.enabled(if: ProcessInfo.processInfo.environment["HITSLOP_BENCH_CAPTURE"] == "1")) @MainActor
  func previewCaptureCost() async throws {
    _ = NSApplication.shared
    let repository = Fixtures.repository.path
    let folder = try Fixtures.folder()
    defer { try? FileManager.default.removeItem(at: folder) }
    var records: [[String: Any]] = []
    for rows in [1000, 5000] {
      let stage = try Fixtures.nativeStage()
      try Fixtures.updateApp(stage) { $0["initial"] = [
        "title": "Capture measurement",
        "tasks": (0..<rows).map { ["text": "Task \($0)", "done": false, "archived": false] as [String: Any] },
      ] }
      let root = try Fixtures.document(stage: stage, at: folder.appendingPathComponent("\(rows).slop"))
      let session = try await DocumentSession.open(url: root)
      session.load()
      try await session.waitUntilReady()
      do {
        var total: [Double] = [], preparation: [[String: Any]] = []
        for sample in 0..<6 {
          let start = Date()
          _ = try await SlopRenderer.previewPNGData(session: session)
          let elapsed = Date().timeIntervalSince(start) * 1000
          if sample > 0 { total.append(elapsed) }
        }
        // Measure preparation in fresh saved renderers, separate from total capture cost.
        for sample in 0..<6 {
          let measurement = try await session.withCaptureSnapshot { source in
            try await SlopRenderer.withRenderSession(url: source) { renderer in
              return try await renderer.webView.callAsyncJavaScript("""
                const token = crypto.randomUUID(), start = performance.now();
                try {
                  const box = await globalThis.__slop.capture.begin(token, "preview");
                  return {preview_prepare_ms: performance.now() - start, height_css_px: box.height, dedicated: box.dedicated};
                } finally { await globalThis.__slop.capture.restore(token); }
                """, arguments: [:], in: nil, contentWorld: .page) as! [String: Any]
            }
          }
          if sample > 0 { preparation.append(measurement) }
        }
        let sorted = total.sorted()
        records.append(["rows": rows, "windows": 1, "preview_total_ms": total,
          "preview_total_median_ms": sorted[2], "preview_total_max_ms": sorted.last!,
          "preparations": preparation])
      } catch { try await session.close(); throw error }
      try await session.close()
    }
    let output = URL(fileURLWithPath: repository + "/.hitslop/evidence/preview-capture.json")
    try FileManager.default.createDirectory(at: output.deletingLastPathComponent(), withIntermediateDirectories: true)
    try JSONSerialization.data(withJSONObject: ["build": "Debug native test bundle", "warmup": 1, "samples": 5, "results": records], options: [.prettyPrinted, .sortedKeys]).write(to: output)
  }
  private func footprint(_ pid: Int32) -> UInt64? {
    var usage = rusage_info_v4()
    let result = withUnsafeMutablePointer(to: &usage) { ptr in
      ptr.withMemoryRebound(to: rusage_info_t?.self, capacity: 1) {
        proc_pid_rusage(pid, RUSAGE_INFO_V4, $0)
      }
    }
    return result == 0 ? usage.ri_phys_footprint : nil
  }
  @Test(.enabled(if: ProcessInfo.processInfo.environment["HITSLOP_BENCH"] == "1")) @MainActor
  func currentPageWindows() async throws {
    _ = NSApplication.shared
    var records: [[String: Any]] = []
    // S-A attribution: HITSLOP_BENCH_LABEL names the variant; HITSLOP_BENCH_NODOM=1 mounts
    // nothing, isolating owner, bridge and SDK cost from the Checklist's DOM.
    let label = ProcessInfo.processInfo.environment["HITSLOP_BENCH_LABEL"] ?? ""
    let noDOM = ProcessInfo.processInfo.environment["HITSLOP_BENCH_NODOM"] == "1"
    func writeEvidence(failure: String? = nil) throws {
      let root = Fixtures.repository.path
      let out = URL(fileURLWithPath: root + "/.hitslop/evidence")
      try FileManager.default.createDirectory(at: out, withIntermediateDirectories: true)
      try JSONSerialization.data(
        withJSONObject: [ "loro": "c00c9fa501f8",
          "method":
            "Frameless window controllers with hover panels in the Host test harness, not the catalog application. One sequential run per cell, fully rendered rows, host plus identified WebContent physical footprints; excludes GPU/network processes. Creation plus opening, warm machine. Checkbox acceptance, rendering and durable drain. Owner publication callback to JS arrival matched by sequence using epoch clocks (approximately millisecond precision); includes test timestamp/JSON decoding overhead. Only the first 100 edits enter publication phase samples; save-status pushes are excluded. Debug helper/test bundle, not an optimized app. Absolute memory only; no leak or matched-control percentage claim.",
          "results": records, "failure": failure as Any? ?? NSNull(),
          "variant": ["label": label, "noDOM": noDOM],
        ], options: [.prettyPrinted, .sortedKeys]
      ).write(to: out.appendingPathComponent("native-owner-windows\(label.isEmpty ? "" : "-" + label).json"))
    }
    let folder = try Fixtures.folder()
    defer { try? FileManager.default.removeItem(at: folder) }
    let environment = ProcessInfo.processInfo.environment
    let rowCounts = environment["HITSLOP_BENCH_ROWS"]?.split(separator: ",").compactMap { Int($0) } ?? [1000, 5000]
    let windowCounts = environment["HITSLOP_BENCH_WINDOWS"]?.split(separator: ",").compactMap { Int($0) } ?? [1, 10, 20]
    for rows in rowCounts {
      for count in windowCounts {
        var windows: [SlopDocumentWindowController] = []
        let start = Date()
        for index in 0..<count {
          let stage = try Fixtures.nativeStage()
          try Fixtures.updateApp(stage) { $0["initial"] = [
            "title": "Measurement",
            "tasks": (0..<rows).map { ["text": "Task \($0)", "done": false, "archived": false] as [String: Any] },
          ] }
          // Capture public ctx for measurement while retaining the actual authored view.
          let app = stage.appendingPathComponent("assets/app.js")
          try FileManager.default.moveItem(at: app, to: stage.appendingPathComponent("assets/benchmark-authored.js"))
          // Attribution only: CSS appended to the authored styles (HITSLOP_BENCH_CSS).
          let css = String(decoding: try JSONSerialization.data(withJSONObject: [environment["HITSLOP_BENCH_CSS"] ?? ""]), as: UTF8.self)
          try Fixtures.writeApp("""
            import authored from './benchmark-authored.js';
            export default { mount(ctx, target) {
              const css = \(css)[0];
              if (css) document.head.append(Object.assign(document.createElement('style'), { textContent: css }));
              const mountStart = performance.now();
              const view = \(noDOM ? "{ rendered() {} }" : "authored.mount(ctx, target)");
              globalThis.benchmarkMountMS = performance.now() - mountStart;
              globalThis.benchmarkBootMS = mountStart;
              globalThis.benchmarkDocument = ctx.document;
              globalThis.benchmarkRendered = () => view?.rendered?.();
              return view;
            } };
            """, to: stage)
          let root = try Fixtures.document(stage: stage, at: folder.appendingPathComponent("\(rows)-\(count)-\(index).slop"))
          windows.append(try await SlopDocumentWindowController.open(url: root))
        }
        for window in windows {
          window.showWindow(nil)
          let readySeconds = Double(ProcessInfo.processInfo.environment["HITSLOP_BENCH_READY_S"] ?? "") ?? 15
          do { try await window.session.waitUntilReady(timeout: .milliseconds(Int(readySeconds * 1000))) } catch {
            let message = "Benchmark \(rows) rows × \(count) windows: \(error.localizedDescription)"
            try writeEvidence(failure: message)
            throw SlopFailure(message)
          }
          await window.waitForPresentation()
        }
        let openMS = Date().timeIntervalSince(start) * 1000
        let first = windows[0]
        let publicationTimes = PublicationTimes()
        let originalPublication = first.session.owner.onPublication
        first.session.owner.onPublication = { publication in
          publicationTimes.record(publication)
          originalPublication?(publication)
        }
        let timings = try await first.session.webView.callAsyncJavaScript(
          """
          const doc = globalThis.benchmarkDocument, id = doc.current.tasks[0].$id;
          const acceptance = [], rendered = [], ipc = [], notify = [];
          // Attribution: native round trip vs SDK listeners (incl. framework work they trigger).
          const handler = globalThis.webkit.messageHandlers.hitslop;
          const post = handler.postMessage.bind(handler);
          let ipcMS = 0;
          handler.postMessage = async (m) => { const t = performance.now(); try { return await post(m); } finally { ipcMS += performance.now() - t; } };
          let notifyMS = 0, last = 0;
          doc.subscribe(() => { last = performance.now(); });
          // Push path: arrival, synchronous receive/notify, and when the main thread is next free.
          const events = globalThis.__slop, publication = events.publish;
          const phases = [], arrivals = [];
          let editStart = 0, measuring = true;
          events.publish = (p) => {
            const arrived = performance.now();
            const measured = measuring && p.some(item => item.type === "publication");
            const started = editStart;
            if (measured) for (const item of p) if (item.type === "publication")
              arrivals.push({sequence: item.publication.sequence, epochMS: performance.timeOrigin + arrived});
            const result = publication(p);
            const syncDone = performance.now();
            if (measured) Promise.resolve(result).then(() => {
              const settled = performance.now();
              const channel = new MessageChannel();
              channel.port1.onmessage = () => { phases.push({ arrive: arrived - started, sync: syncDone - arrived, settle: settled - arrived, free: performance.now() - arrived }); channel.port1.close(); channel.port2.close(); };
              channel.port2.postMessage(0);
            });
            return result;
          };
          for (let i = 0; i < 100; i++) {
            ipcMS = 0;
            const start = performance.now();
            editStart = start;
            await doc.fields.tasks.item(id).done.set(i % 2 === 0);
            acceptance.push(performance.now() - start);
            ipc.push(ipcMS);
            await globalThis.benchmarkRendered();
            rendered.push(performance.now() - start);
          }
          measuring = false;
          // Idle-start edits: each click begins after the previous frame finished rendering,
          // like a real click. frame = the rendering work the edit causes.
          const idle = [], frame = [];
          // Test windows may be occluded, where requestAnimationFrame never fires: use timers.
          const nextFrame = () => new Promise(r => setTimeout(r, 0));
          for (let i = 0; i < 40; i++) {
            await new Promise(r => setTimeout(r, 100));
            const start = performance.now();
            await doc.fields.tasks.item(id).done.set(i % 2 === 0);
            idle.push(performance.now() - start);
            const f = performance.now();
            await nextFrame();
            frame.push(performance.now() - f);
          }
          // Title edits change no row but still rebuild the filtered list: isolates list reconcile.
          const titleFrame = [];
          for (let i = 0; i < 20; i++) {
            await new Promise(r => setTimeout(r, 100));
            await doc.fields.title.set("Measurement " + i);
            const f = performance.now();
            await nextFrame();
            titleFrame.push(performance.now() - f);
          }
          const rowTextFrame = [];
          for (let i = 0; i < 20; i++) {
            await new Promise(r => setTimeout(r, 100));
            await doc.fields.tasks.item(id).text.set("Task edited " + i);
            const f = performance.now();
            await nextFrame();
            rowTextFrame.push(performance.now() - f);
          }
          // Same split as scripts/dev/bench-webkit.ts: click → DOM mutation (JS), forced layout, rest.
          const split = { js: [], layout: [], rest: [] };
          for (let i = 0; i < 20; i++) {
            await new Promise(r => setTimeout(r, 100));
            const row = document.querySelectorAll(".checklist-row")[2];
            const box = row?.querySelector("input[type=checkbox]");
            if (!box) break;
            const changed = new Promise(resolve => { const o = new MutationObserver(() => { o.disconnect(); resolve(); }); o.observe(row, { attributes: true, subtree: true, childList: true, characterData: true }); });
            const start = performance.now();
            box.click();
            await changed; await Promise.resolve(); await Promise.resolve();
            const js = performance.now();
            void document.documentElement.offsetHeight;
            const laid = performance.now();
            await new Promise(r => setTimeout(r, 0));
            split.js.push(js - start); split.layout.push(laid - js); split.rest.push(performance.now() - laid);
          }
          // Row text edit split: owner round trip + JS until the DOM changes, style, layout,
          // the rest; plus how many DOM records the edit produced across the whole list.
          const textSplit = { js: [], style: [], layout: [], rest: [], records: [] };
          for (let i = 0; i < 10; i++) {
            await new Promise(r => setTimeout(r, 100));
            const list = document.querySelector(".checklist-list");
            if (!list) break;
            let records = 0;
            const all = new MutationObserver(m => { records += m.length; });
            all.observe(list, { attributes: true, subtree: true, childList: true, characterData: true });
            const row = list.querySelector(".checklist-row");
            const changed = new Promise(resolve => { const o = new MutationObserver(() => { o.disconnect(); resolve(); }); o.observe(row, { attributes: true, subtree: true, childList: true, characterData: true }); });
            const start = performance.now();
            void doc.fields.tasks.item(id).text.set("Task split " + i);
            await changed; await Promise.resolve(); await Promise.resolve();
            const js = performance.now();
            void getComputedStyle(row).color;
            const styled = performance.now();
            void document.documentElement.offsetHeight;
            const laid = performance.now();
            await new Promise(r => setTimeout(r, 0));
            all.takeRecords().forEach(() => records++); all.disconnect();
            textSplit.js.push(js - start); textSplit.style.push(styled - js); textSplit.layout.push(laid - styled); textSplit.rest.push(performance.now() - laid); textSplit.records.push(records);
          }
          const median = a => a.sort((x,y) => x-y)[a.length >> 1];
          const rowTextSplit = Object.fromEntries(Object.entries(textSplit).map(([k, v]) => [k, median(v)]));
          const scroller = document.querySelector(".checklist-scroller");
          const clickSplit = { js: median(split.js), layout: median(split.layout), rest: median(split.rest),
            viewport: [innerWidth, innerHeight], scrollerClient: scroller?.clientHeight, scrollerContent: scroller?.scrollHeight };
          handler.postMessage = post;
          events.publish = publication;
          const med = k => { const a = phases.map(x => x[k]).sort((x,y) => x-y); return { p50: a[a.length>>1], p95: a[Math.floor(a.length*0.95)] }; };
          const phaseSummary = { arrive: med("arrive"), sync: med("sync"), settle: med("settle"), free: med("free"), count: phases.length };
          const p95 = a => a.slice().sort((x,y) => x-y)[94];
          const start = performance.now(); await doc.flush();
          return { mountMS: globalThis.benchmarkMountMS, bootMS: globalThis.benchmarkBootMS, acceptance: acceptance.sort((a,b) => a-b), rendered: rendered.sort((a,b) => a-b), drainMS: performance.now()-start, ipcP95: p95(ipc), phases: phaseSummary, arrivals, idle: idle.sort((a,b) => a-b), frame: frame.sort((a,b) => a-b), titleFrame: titleFrame.sort((a,b) => a-b), rowTextFrame: rowTextFrame.sort((a,b) => a-b), clickSplit, rowTextSplit };
          """, arguments: [:], in: nil, contentWorld: .page) as! [String: Any]
        let acceptance = timings["acceptance"] as! [Double], rendered = timings["rendered"] as! [Double]
        first.session.owner.onPublication = originalPublication
        let publicationCosts = publicationTimes.arrivalCosts(timings["arrivals"] as! [[String: Any]])
        #expect(publicationCosts.count == 100)
        let pids = Set(
          windows.compactMap { window -> Int32? in
            let key = "_webProcessIdentifier"
            guard window.session.webView.responds(to: NSSelectorFromString(key)) else { return nil }
            return (window.session.webView.value(forKey: key) as? NSNumber)?.int32Value
          })
        let samples = ([getpid()] + Array(pids)).compactMap { footprint($0) }
        let total = samples.reduce(UInt64(0), +)
        let hostBytes = footprint(getpid()) ?? 0
        let contentBytes = Array(pids).compactMap { footprint($0) }.reduce(UInt64(0), +)
        let database = first.url
        let bytes =
          (try FileManager.default.attributesOfItem(atPath: database.path)[.size] as! NSNumber)
          .intValue
        for window in windows { try await window.session.close(); window.window?.orderOut(nil) }
        windows.removeAll()
        try await Task.sleep(for: .milliseconds(500))
        records.append([
          "rows": rows, "windows": count, "open_ms": openMS, "acceptance_p95_ms": acceptance[94], "rendered_p95_ms": rendered[94],
          "drain_ms": timings["drainMS"]!, "ipc_p95_ms": timings["ipcP95"] ?? NSNull(), "push_phases_ms": timings["phases"] ?? NSNull(), "mount_sync_ms": timings["mountMS"] ?? NSNull(),
          "owner_to_js_publication_p50_ms": publicationCosts.isEmpty ? NSNull() : publicationCosts[publicationCosts.count / 2],
          "owner_to_js_publication_p95_ms": publicationCosts.isEmpty ? NSNull() : publicationCosts[Int(Double(publicationCosts.count) * 0.95)],
          "owner_to_js_publication_samples": publicationCosts.count,
          "idle_acceptance_p50_ms": (timings["idle"] as? [Double])?[20] ?? NSNull(), "idle_acceptance_p95_ms": (timings["idle"] as? [Double])?[38] ?? NSNull(),
          "frame_after_edit_p50_ms": (timings["frame"] as? [Double])?[20] ?? NSNull(), "frame_after_edit_p95_ms": (timings["frame"] as? [Double])?[38] ?? NSNull(),
          "frame_after_title_edit_p50_ms": (timings["titleFrame"] as? [Double])?[10] ?? NSNull(),
          "frame_after_row_text_edit_p50_ms": (timings["rowTextFrame"] as? [Double])?[10] ?? NSNull(),
          "click_split_ms": timings["clickSplit"] ?? NSNull(), "row_text_split_ms": timings["rowTextSplit"] ?? NSNull(), "page_ms_before_mount": timings["bootMS"] ?? NSNull(),
          "host_footprint_mib": Double(hostBytes) / 1_048_576,
          "webcontent_footprint_mib": Double(contentBytes) / 1_048_576,
          "host_plus_content_footprint_mib": Double(total) / 1_048_576,
          "processes_measured": samples.count, "database_bytes_after_100_edits": bytes,
          "host_footprint_after_close_mib": Double(footprint(getpid()) ?? 0) / 1_048_576,
        ])
        try writeEvidence()
      }
    }
    try writeEvidence()
  }
}
