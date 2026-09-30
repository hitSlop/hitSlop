import AppKit
import Darwin
import Foundation
import Testing
import HitSlopCore
import HitSlopRuntime
import WebKit

@testable import HitSlopHost

@Suite(.serialized) struct BenchmarkTests {
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
  func currentRuntimeWindows() async throws {
    _ = NSApplication.shared
    var records: [[String: Any]] = []
    // S-A attribution: HITSLOP_BENCH_LABEL names the variant; HITSLOP_BENCH_NODOM=1 mounts
    // nothing, isolating owner, bridge and SDK cost from the Checklist's DOM.
    let label = ProcessInfo.processInfo.environment["HITSLOP_BENCH_LABEL"] ?? ""
    let noDOM = ProcessInfo.processInfo.environment["HITSLOP_BENCH_NODOM"] == "1"
    func writeEvidence(failure: String? = nil) throws {
      let root = String(#filePath.components(separatedBy: "/apps/apple/")[0])
      let out = URL(fileURLWithPath: root + "/.hitslop/v1-evidence")
      try FileManager.default.createDirectory(at: out, withIntermediateDirectories: true)
      try JSONSerialization.data(
        withJSONObject: [ "loro": "1.16.2",
          "method":
            "Restored frameless window controllers with hover panels, in the Host test harness (not the catalog application). One sequential run per cell, fully rendered rows, host plus identified WebContent physical footprints; excludes GPU/network processes. Creation plus opening, warm machine. Public ABI-2 checkbox acceptance, framework rendering and durable drain. Debug helper/test bundle, not an optimized app. Absolute memory only; no matched-control percentage claim.",
          "results": records, "failure": failure as Any? ?? NSNull(),
          "variant": ["label": label, "noDOM": noDOM,
            "autosaveMS": ProcessInfo.processInfo.environment["HITSLOP_AUTOSAVE_MS"] ?? "150"],
        ], options: [.prettyPrinted, .sortedKeys]
      ).write(to: out.appendingPathComponent("native-owner-windows\(label.isEmpty ? "" : "-" + label).json"))
    }
    let folder = FileManager.default.temporaryDirectory.appendingPathComponent(
      "hsl-bench-" + UUID().uuidString)
    try FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true)
    defer { try? FileManager.default.removeItem(at: folder) }
    let environment = ProcessInfo.processInfo.environment
    let rowCounts = environment["HITSLOP_BENCH_ROWS"]?.split(separator: ",").compactMap { Int($0) } ?? [1000, 5000]
    let windowCounts = environment["HITSLOP_BENCH_WINDOWS"]?.split(separator: ",").compactMap { Int($0) } ?? [1, 10, 20]
    for rows in rowCounts {
      for count in windowCounts {
        var windows: [SlopDocumentWindowController] = []
        let start = Date()
        for index in 0..<count {
          let root = folder.appendingPathComponent("\(rows)-\(count)-\(index).slop")
          try FileManager.default.copyItem(
            at: URL(fileURLWithPath:String(#filePath.components(separatedBy:"/apps/apple/")[0])+"/generated/v1/native-fixtures/quick-checklist.slop"), to: root)
          try JSONSerialization.data(withJSONObject: [
            "title": "Measurement",
            "tasks": (0..<rows).map { ["text": "Task \($0)", "done": false, "archived": false] as [String: Any] },
          ]).write(to: root.appendingPathComponent("initial.json"))
          // Capture public ctx for measurement while retaining the actual authored view.
          let app = root.appendingPathComponent("assets/app.js")
          try FileManager.default.moveItem(at: app, to: root.appendingPathComponent("assets/benchmark-authored.js"))
          try Data("""
            import authored from './benchmark-authored.js';
            export default { mount(ctx, target) {
              const mountStart = performance.now();
              const view = \(noDOM ? "{ rendered() {} }" : "authored.mount(ctx, target)");
              globalThis.benchmarkMountMS = performance.now() - mountStart;
              globalThis.benchmarkBootMS = mountStart;
              globalThis.benchmarkDocument = ctx.document;
              globalThis.benchmarkRendered = () => view?.rendered?.();
              return view;
            } };
            """.utf8).write(to: app)
          windows.append(try await SlopDocumentWindowController.open(packageURL: root))
        }
        for window in windows {
          window.showWindow(nil)
          let readySeconds = Double(ProcessInfo.processInfo.environment["HITSLOP_BENCH_READY_S"] ?? "") ?? 15
          do { try await window.session.waitUntilReady(timeout: .milliseconds(Int(readySeconds * 1000))) } catch {
            let message = "Benchmark \(rows) rows × \(count) windows: \(error.localizedDescription)"
            try writeEvidence(failure: message)
            throw SlopPackageError.invalid(message)
          }
          await window.waitForPresentation()
        }
        let openMS = Date().timeIntervalSince(start) * 1000
        let first = windows[0]
        let timings = try await first.session.webView.callAsyncJavaScript(
          """
          const doc = globalThis.benchmarkDocument, id = doc.current.tasks[0].$id;
          const acceptance = [], rendered = [], ipc = [], notify = [];
          // Attribution: native round trip vs SDK listeners (incl. framework work they trigger).
          const handler = globalThis.webkit.messageHandlers.owner;
          const post = handler.postMessage.bind(handler);
          let ipcMS = 0;
          handler.postMessage = async (m) => { const t = performance.now(); try { return await post(m); } finally { ipcMS += performance.now() - t; } };
          let notifyMS = 0, last = 0;
          doc.subscribe(() => { last = performance.now(); });
          // Push path: arrival, synchronous receive/notify, and when the main thread is next free.
          const events = globalThis.__hitslop, publication = events.publish;
          const phases = [];
          let editStart = 0;
          events.publish = (p) => {
            const arrived = performance.now();
            const result = publication(p);
            const syncDone = performance.now();
            Promise.resolve(result).then(() => {
              const settled = performance.now();
              const channel = new MessageChannel();
              channel.port1.onmessage = () => phases.push({ arrive: arrived - editStart, sync: syncDone - arrived, settle: settled - arrived, free: performance.now() - arrived });
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
          // Same split as scripts/v1/bench-webkit.ts: click → DOM mutation (JS), forced layout, rest.
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
          const median = a => a.sort((x,y) => x-y)[a.length >> 1];
          const scroller = document.querySelector(".checklist-scroller");
          const clickSplit = { js: median(split.js), layout: median(split.layout), rest: median(split.rest),
            viewport: [innerWidth, innerHeight], scrollerClient: scroller?.clientHeight, scrollerContent: scroller?.scrollHeight };
          handler.postMessage = post;
          events.publication = publication;
          const med = k => { const a = phases.map(x => x[k]).sort((x,y) => x-y); return { p50: a[a.length>>1], p95: a[Math.floor(a.length*0.95)] }; };
          const phaseSummary = { arrive: med("arrive"), sync: med("sync"), settle: med("settle"), free: med("free"), count: phases.length };
          const p95 = a => a.slice().sort((x,y) => x-y)[94];
          const start = performance.now(); await doc.flush();
          return { mountMS: globalThis.benchmarkMountMS, bootMS: globalThis.benchmarkBootMS, acceptance: acceptance.sort((a,b) => a-b), rendered: rendered.sort((a,b) => a-b), drainMS: performance.now()-start, ipcP95: p95(ipc), phases: phaseSummary, idle: idle.sort((a,b) => a-b), frame: frame.sort((a,b) => a-b), titleFrame: titleFrame.sort((a,b) => a-b), rowTextFrame: rowTextFrame.sort((a,b) => a-b), clickSplit };
          """, arguments: [:], in: nil, contentWorld: .page) as! [String: Any]
        let acceptance = timings["acceptance"] as! [Double], rendered = timings["rendered"] as! [Double]
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
        let database = first.packageURL.appendingPathComponent("state/document.sqlite")
        let bytes =
          (try FileManager.default.attributesOfItem(atPath: database.path)[.size] as! NSNumber)
          .intValue
        for window in windows { try await window.session.finish(); window.window?.orderOut(nil) }
        windows.removeAll()
        try await Task.sleep(for: .milliseconds(500))
        records.append([
          "rows": rows, "windows": count, "open_ms": openMS, "acceptance_p95_ms": acceptance[94], "rendered_p95_ms": rendered[94],
          "drain_ms": timings["drainMS"]!, "ipc_p95_ms": timings["ipcP95"] ?? NSNull(), "push_phases_ms": timings["phases"] ?? NSNull(), "mount_sync_ms": timings["mountMS"] ?? NSNull(),
          "idle_acceptance_p50_ms": (timings["idle"] as? [Double])?[20] ?? NSNull(), "idle_acceptance_p95_ms": (timings["idle"] as? [Double])?[38] ?? NSNull(),
          "frame_after_edit_p50_ms": (timings["frame"] as? [Double])?[20] ?? NSNull(), "frame_after_edit_p95_ms": (timings["frame"] as? [Double])?[38] ?? NSNull(),
          "frame_after_title_edit_p50_ms": (timings["titleFrame"] as? [Double])?[10] ?? NSNull(),
          "frame_after_row_text_edit_p50_ms": (timings["rowTextFrame"] as? [Double])?[10] ?? NSNull(),
          "click_split_ms": timings["clickSplit"] ?? NSNull(), "page_ms_before_mount": timings["bootMS"] ?? NSNull(),
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
