import Foundation
import HitSlopCore
import HitSlopTestSupport
import Testing

@testable import HitSlopDocument

// The native helper, FFI and owner must run the stored command for both entry points.
@Test @MainActor func pageAndCLICommandsShareTheNativeOwner() async throws {
  let root = try Fixtures.native()
  defer { try? FileManager.default.removeItem(at: root) }
  let session = try await DocumentSession.open(url: root)
  session.load()
  do {
    try await session.waitUntilReady()
    let measured =
      try await session.webView.callAsyncJavaScript(
        """
        const post = async request => JSON.parse(await webkit.messageHandlers.hitslop.postMessage(JSON.stringify(request)));
        const times = [];
        for (let i = 0; i < 40; i++) {
          const start = performance.now();
          const reply = await post({method:'commands.run', name:'addTask', args:{text:'Native command ' + i}});
          if (!reply.ok || reply.ids.length !== 1) throw new Error(JSON.stringify(reply));
          times.push(performance.now() - start);
        }
        const invalid = await post({method:'commands.run', name:'addTask', args:{text:3}});
        if (invalid.ok || invalid.code !== 'rejected') throw new Error('Unchecked arguments');
        times.sort((a,b) => a-b);
        return [times[20], times[37]];
        """, arguments: [:], in: nil, contentWorld: .page) as? [Double]
    let times = try #require(measured)
    print("Native WebKit command latency, 40 fresh evaluations: p50=\(times[0])ms p95=\(times[1])ms")
    let cli = try await command("call", url: root, ["command": "addTask", "args": ["text": "CLI command"]])
    #expect(cli.ok && cli.ids?.count == 1)
    // The page receives the CLI command's publication without reopening.
    #expect(
      try await eventually(timeout: .seconds(5)) {
        try await session.webView.evaluateJavaScript(
          "[...document.querySelectorAll('[role=textbox]')].some(e => e.textContent === 'CLI command')") as? Bool
          == true
      })
    try await session.close()
    let value = try JSONSerialization.jsonObject(with: await commandState("get", url: root)) as! [String: Any]
    let tasks = (value["value"] as! [String: Any])["tasks"] as! [[String: Any]]
    #expect(tasks.filter { ($0["text"] as? String)?.hasPrefix("Native command ") == true }.count == 40)
    #expect(tasks.last?["text"] as? String == "CLI command")
  } catch {
    try? await session.close()
    throw error
  }
}
