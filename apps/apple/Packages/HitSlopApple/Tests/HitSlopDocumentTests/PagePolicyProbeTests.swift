import AppKit
import Foundation
import Testing
import HitSlopTestSupport
import WebKit

@testable import HitSlopDocument

/// What the page policy and the `slop:` scheme allow, observed in a real document window:
/// workers, worklets, WebAssembly, cross-origin isolation and media seeking.
@Suite(.serialized) struct PagePolicyProbeTests {
  @Test @MainActor func pagePolicyRunsPackageCodeAndWebAssemblyNotInlineScripts() async throws {
    let stage = try Fixtures.stage()
    let assets = stage.appendingPathComponent("assets")
    let files: [String: Data] = [
      "worker.js": Data("postMessage('worker');".utf8),
      "module.js": Data("export {}; postMessage('module');".utf8),
      "module.mjs": Data("export {}; postMessage('mjs');".utf8),
      "worklet.js": Data(Self.workletSource.utf8),
      // (module (func (export "add") (param i32 i32) (result i32) local.get 0 local.get 1 i32.add))
      "add.wasm": Data([
        0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00,
        0x01, 0x07, 0x01, 0x60, 0x02, 0x7f, 0x7f, 0x01, 0x7f,
        0x03, 0x02, 0x01, 0x00,
        0x07, 0x07, 0x01, 0x03, 0x61, 0x64, 0x64, 0x00, 0x00,
        0x0a, 0x09, 0x01, 0x07, 0x00, 0x20, 0x00, 0x20, 0x01, 0x6a, 0x0b,
      ]),
      "tone.wav": Self.wav(seconds: 3),
    ]
    for (name, data) in files { try data.write(to: assets.appendingPathComponent(name)) }
    try Fixtures.writeApp("export default { mount() { return {}; } };", to: stage)
    let root = try Fixtures.document(stage: stage)
    defer { try? FileManager.default.removeItem(at: root) }

    let session = try await DocumentSession.open(url: root)
    session.webView.configuration.userContentController.addUserScript(WKUserScript(source: """
      globalThis.policyViolations = [];
      addEventListener('securitypolicyviolation', event => policyViolations.push(`${event.violatedDirective} ${event.blockedURI}`));
      """, injectionTime: .atDocumentStart, forMainFrameOnly: true))
    // WebKit starts media only in a page that is in a window.
    let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 400, height: 400), styleMask: [.titled], backing: .buffered, defer: false)
    window.contentView = session.webView
    window.orderFront(nil)
    defer { window.contentView = nil; window.orderOut(nil) }
    session.load()
    let results: [String: String]
    do {
      try await session.waitUntilReady()
      let json = try await session.webView.callAsyncJavaScript(
        Self.probe, arguments: ["workletSource": Self.workletSource], in: nil, contentWorld: .page) as? String
      results = try JSONDecoder().decode([String: String].self, from: Data((json ?? "{}").utf8))
      try await session.close()
    } catch { try? await session.close(); throw error }

    // Evidence capture: every outcome, including the ones not asserted below.
    if let output = ProcessInfo.processInfo.environment["HITSLOP_PROBE_OUT"] {
      try JSONSerialization.data(withJSONObject: results, options: [.prettyPrinted, .sortedKeys])
        .write(to: URL(fileURLWithPath: output))
    }
    // Workers and worklets shipped as app assets run.
    #expect(results["worker.asset"] == "ok:worker")
    #expect(results["worker.module.js"] == "ok:module")
    #expect(results["worker.module.mjs"] == "ok:mjs")
    #expect(results["worklet.asset"] == "ok")
    // JavaScript made from strings does not.
    for refused in ["worker.blob", "worker.data", "worklet.blob", "worklet.data"] {
      #expect(results[refused]?.hasPrefix("error") == true, "\(refused): \(results[refused] ?? "missing")")
    }
    // WebAssembly compiles, including from an asset streamed with its own type
    // (MilkDrop presets in Soma Amp compile their equations this way).
    #expect(results["wasm.instantiate"] == "5", "wasm.instantiate: \(results["wasm.instantiate"] ?? "missing")")
    #expect(results["wasm.module"] == "5", "wasm.module: \(results["wasm.module"] ?? "missing")")
    #expect(results["wasm.streaming"] == "5", "wasm.streaming: \(results["wasm.streaming"] ?? "missing")")
    // Media assets play and seek: WebKit's media loader needs byte ranges, read from the file.
    #expect(results["media.asset"]?.hasSuffix("seekedTo=1.50") == true, "media.asset: \(results["media.asset"] ?? "missing")")
  }

  // The range forms WebKit's media loader doesn't exercise above.
  @Test func byteRangesFollowHTTP() {
    #expect(ByteRange("bytes=0-1", length: 10) == .part(0..<2))
    #expect(ByteRange("bytes=4-", length: 10) == .part(4..<10))
    #expect(ByteRange("bytes=-3", length: 10) == .part(7..<10))
    #expect(ByteRange("bytes=-30", length: 10) == .part(0..<10))
    #expect(ByteRange("bytes=8-99", length: 10) == .part(8..<10))
    #expect(ByteRange("bytes=10-", length: 10) == .unsatisfiable)
    #expect(ByteRange("bytes=5-4", length: 10) == .unsatisfiable)
    #expect(ByteRange("bytes=-0", length: 10) == .unsatisfiable)
    #expect(ByteRange("bytes=0-1,4-5", length: 10) == .whole)
    #expect(ByteRange("items=0-1", length: 10) == .whole)
    #expect(ByteRange("bytes=a-b", length: 10) == .whole)
    // The largest endpoints a request can name never overflow.
    #expect(ByteRange("bytes=0-\(Int.max)", length: 10) == .part(0..<10))
    #expect(ByteRange("bytes=\(Int.max)-", length: 10) == .unsatisfiable)
    #expect(ByteRange("bytes=-\(Int.max)", length: 10) == .part(0..<10))
  }

  private static let workletSource =
    "registerProcessor('probe', class extends AudioWorkletProcessor { process() { return false; } });"

  /// Each probe settles within three seconds and records `ok…`, a value, or `error: …`.
  private static let probe = """
    const results = {};
    const settle = async (name, run) => {
      const timeout = new Promise((_, reject) => setTimeout(() => reject(new Error('timeout')), 3000));
      try { results[name] = String(await Promise.race([run(), timeout])); }
      catch (error) { results[name] = 'error: ' + (error?.message || String(error)); }
    };
    const worker = (url, options) => new Promise((resolve, reject) => {
      const instance = new Worker(url, options);
      instance.onmessage = event => { resolve('ok:' + event.data); instance.terminate(); };
      instance.onerror = event => { event.preventDefault?.(); reject(new Error('worker failed: ' + (event.message || 'load'))); instance.terminate(); };
    });
    await settle('worker.asset', () => worker('/assets/worker.js'));
    await settle('worker.module.js', () => worker('/assets/module.js', { type: 'module' }));
    await settle('worker.module.mjs', () => worker('/assets/module.mjs', { type: 'module' }));
    await settle('worker.blob', () => worker(URL.createObjectURL(new Blob(["postMessage('blob')"], { type: 'text/javascript' }))));
    await settle('worker.data', () => worker("data:text/javascript,postMessage('data')"));

    const worklet = async url => { await new OfflineAudioContext(1, 128, 44100).audioWorklet.addModule(url); return 'ok'; };
    await settle('worklet.asset', () => worklet('/assets/worklet.js'));
    await settle('worklet.blob', () => worklet(URL.createObjectURL(new Blob([workletSource], { type: 'text/javascript' }))));
    await settle('worklet.data', () => worklet('data:text/javascript,' + encodeURIComponent(workletSource)));

    const response = await fetch('/assets/add.wasm');
    results['wasm.contentType'] = String(response.headers.get('content-type'));
    const bytes = await response.arrayBuffer();
    await settle('wasm.instantiate', async () => (await WebAssembly.instantiate(bytes)).instance.exports.add(2, 3));
    await settle('wasm.module', async () => new WebAssembly.Instance(new WebAssembly.Module(bytes)).exports.add(2, 3));
    await settle('wasm.streaming', async () => (await WebAssembly.instantiateStreaming(fetch('/assets/add.wasm'))).instance.exports.add(2, 3));

    results['isolation.crossOriginIsolated'] = String(globalThis.crossOriginIsolated);
    results['isolation.SharedArrayBuffer'] = typeof SharedArrayBuffer;

    const media = (name, source) => {
      const events = [];
      results[name + '.events'] = '';
      return settle(name, async () => {
        const src = await source();
        return new Promise((resolve, reject) => {
          const audio = new Audio();
          for (const type of ['loadstart', 'loadedmetadata', 'canplay', 'stalled', 'suspend', 'abort', 'error', 'seeking', 'seeked'])
            audio.addEventListener(type, () => { events.push(type); results[name + '.events'] = events.join(','); });
          audio.preload = 'auto';
          audio.onerror = () => reject(new Error('media error ' + audio.error?.code + ' ' + (audio.error?.message || '')));
          audio.onloadedmetadata = () => {
            const seekable = audio.seekable.length ? audio.seekable.end(0) : 0;
            audio.onseeked = () => resolve(`duration=${audio.duration.toFixed(2)} seekableEnd=${seekable.toFixed(2)} seekedTo=${audio.currentTime.toFixed(2)}`);
            audio.currentTime = 1.5;
          };
          audio.src = src;
          audio.load();
        });
      });
    };
    await media('media.asset', async () => '/assets/tone.wav');
    await media('media.blob', async () => URL.createObjectURL(await (await fetch('/assets/tone.wav')).blob()));
    await settle('media.decode', async () => {
      const buffer = await new OfflineAudioContext(1, 128, 8000).decodeAudioData(await (await fetch('/assets/tone.wav')).arrayBuffer());
      return `duration=${buffer.duration.toFixed(2)}`;
    });

    results.violations = policyViolations.join(' | ');
    return JSON.stringify(results);
    """

  /// A mono 16-bit PCM WAV at 8 kHz: a 440 Hz tone, so decoders have real samples.
  private static func wav(seconds: Int) -> Data {
    let rate = 8_000
    let samples = rate * seconds
    var data = Data()
    func append<T: FixedWidthInteger>(_ value: T) { withUnsafeBytes(of: value.littleEndian) { data.append(contentsOf: $0) } }
    data.append(contentsOf: Array("RIFF".utf8)); append(UInt32(36 + samples * 2))
    data.append(contentsOf: Array("WAVEfmt ".utf8)); append(UInt32(16)); append(UInt16(1)); append(UInt16(1))
    append(UInt32(rate)); append(UInt32(rate * 2)); append(UInt16(2)); append(UInt16(16))
    data.append(contentsOf: Array("data".utf8)); append(UInt32(samples * 2))
    for index in 0..<samples {
      append(Int16(sin(Double(index) * 2 * .pi * 440 / Double(rate)) * 8_000))
    }
    return data
  }
}
