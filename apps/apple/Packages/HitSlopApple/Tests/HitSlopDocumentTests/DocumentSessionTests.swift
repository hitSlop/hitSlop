import Foundation
import HitSlopCore
import Testing
import HitSlopTestSupport
import WebKit

@testable import HitSlopDocument

@Suite(.serialized) struct DocumentSessionTests {
  // The production shell must open and execute document operations under the page CSP.
  @Test @MainActor func shellStartupAndOperationsRespectCSP() async throws {
    let repository = String(#filePath.components(separatedBy: "/apps/apple/")[0])
    let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString + ".slop")
    try FileManager.default.copyItem(atPath: repository + "/tests/fixtures/checklist/document", toPath: root.path)
    defer { try? FileManager.default.removeItem(at: root) }
    let session = try await DocumentSession.open(packageURL: root)
    session.webView.configuration.userContentController.addUserScript(WKUserScript(source: """
      globalThis.cspViolations = [];
      addEventListener('securitypolicyviolation', event => cspViolations.push(event.violatedDirective));
      """, injectionTime: .atDocumentStart, forMainFrameOnly: true))
    session.load()
    do {
      try await session.waitUntilReady()
      _ = try await session.webView.callAsyncJavaScript("await globalThis.__slop.flush(); return true", arguments: [:], in: nil, contentWorld: .page)
      let violations = try await session.webView.callAsyncJavaScript("return cspViolations", arguments: [:], in: nil, contentWorld: .page) as? [String]
      #expect(violations == [])
      try await session.close()
    } catch { try? await session.close(); throw error }
  }

  // The native dispatch boundary must reject an oversized or unknown-field request
  // before base64 decoding or touching SQLite.
  @Test func bridgeRejectsOversizedPayloadsAndUnknownFields() {
    #expect((try? PageRequest(["method": "config"])) != nil)
    #expect((try? PageRequest(["method": "config", "extra": "unexpected"])) == nil)
    #expect((try? PageRequest(["method": "attachments.list"])) == nil)
    #expect((try? PageRequest(["method": "open"])) != nil)
    #expect((try? PageRequest(["method": "window.resize", "width": ["nested": 1], "height": 300])) == nil)
    let oversized = String(repeating: "A", count: 15 * 1024 * 1024)
    #expect((try? PageRequest(["method": "attachments.put", "bytes": oversized])) == nil)
  }

  // A theme can change after config is read but before the app finishes mounting.
  @Test @MainActor func themeChangesDuringMountReachTheReadyPage() async throws {
    let repository = String(#filePath.components(separatedBy: "/apps/apple/")[0])
    let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString + ".slop")
    try FileManager.default.copyItem(atPath: repository + "/tests/fixtures/checklist/document", toPath: root.path)
    defer { try? FileManager.default.removeItem(at: root) }
    try Data("""
      export default { async mount() {
        globalThis.mountStarted = true;
        await new Promise(resolve => globalThis.finishMount = resolve);
        return {};
      }};
      """.utf8).write(to: root.appendingPathComponent("assets/app.js"))
    let session = try await DocumentSession.open(packageURL: root)
    session.load()
    do {
      var mounted = false
      for _ in 0..<200 where !mounted {
        mounted = (try? await session.webView.callAsyncJavaScript("return globalThis.mountStarted === true", arguments: [:], in: nil, contentWorld: .page)) as? Bool == true
        if !mounted { try await Task.sleep(for: .milliseconds(25)) }
      }
      #expect(mounted)
      _ = try await session.owner.applyTheme(.set(valuesJson: ##"{"accent":"#123456"}"##))
      _ = try await session.webView.callAsyncJavaScript("finishMount(); return true", arguments: [:], in: nil, contentWorld: .page)
      try await session.waitUntilReady()
      func accent() async throws -> String? {
        try await session.webView.callAsyncJavaScript("return document.documentElement.style.getPropertyValue('--slop-accent')", arguments: [:], in: nil, contentWorld: .page) as? String
      }
      for _ in 0..<100 {
        if try await accent() == "#123456" { break }
        try await Task.sleep(for: .milliseconds(10))
      }
      #expect(try await accent() == "#123456")
      let reset = try await session.owner.applyTheme(.reset(token: "accent"))
      let expected = try JSONDecoder().decode([String: String].self, from: Data(reset.effective.utf8))["accent"]
      for _ in 0..<100 {
        if try await accent() == expected { break }
        try await Task.sleep(for: .milliseconds(10))
      }
      #expect(try await accent() == expected)
      try await session.close()
    } catch { try? await session.close(); throw error }
  }

  // The theme panel's changes are edits: applied in the order made, settled on the page
  // before a flush returns (so an export shows them), held back from a capture in
  // progress, reported to the window, and saved by close.
  @Test @MainActor func panelThemeChangesSettleBeforeFlushAndSaveOnClose() async throws {
    let repository = String(#filePath.components(separatedBy: "/apps/apple/")[0])
    let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString + ".slop")
    try FileManager.default.copyItem(atPath: repository + "/tests/fixtures/checklist/document", toPath: root.path)
    defer { try? FileManager.default.removeItem(at: root) }
    try Data("export default { mount() { return {}; } };".utf8).write(to: root.appendingPathComponent("assets/app.js"))
    let session = try await DocumentSession.open(packageURL: root)
    let events = SessionEvents()
    var reported: [String?] = []
    events.theme = { reported.append($0.effective["accent"]) }
    session.delegate = events
    session.load()
    func accent() async throws -> String? {
      try await session.webView.callAsyncJavaScript(
        "return document.documentElement.style.getPropertyValue('--slop-accent')", arguments: [:], in: nil,
        contentWorld: .page) as? String
    }
    func saved() async throws -> String? {
      let snapshot = try DocumentOwner(package: SlopPackage(rootURL: root), mode: .snapshot)
      defer { Task { try? await snapshot.close() } }
      return try JSONDecoder().decode([String: String].self, from: Data(try await snapshot.loadTheme().state.effective.utf8))["accent"]
    }
    do {
      try await session.waitUntilReady()
      #expect(session.canEditTheme)
      for step in 1...20 { session.changeTheme(.set(["accent": String(format: "#0000%02x", step)])) }
      try await session.flush()
      #expect(try await accent() == "#000014")
      #expect(try await saved() == "#000014")
      #expect(reported.last == "#000014")

      var refused: Error?
      session.changeTheme(.set(["accent": "red"])) { if case .failure(let error) = $0 { refused = error } }
      session.changeTheme(.resetAll)
      try await session.flush()
      #expect(refused != nil)
      #expect(try await accent() == "#335577")

      session.capturing = true
      session.changeTheme(.set(["accent": "#abcabc"]))
      try await Task.sleep(for: .milliseconds(150))
      #expect(try await accent() == "#335577", "a capture in progress is not restyled")
      session.capturing = false
      for _ in 0..<100 where try await accent() != "#abcabc" { try await Task.sleep(for: .milliseconds(10)) }
      #expect(try await accent() == "#abcabc")

      session.changeTheme(.set(["accent": "#fedcba"]))
      try await session.close()
      #expect(try await saved() == "#fedcba")
    } catch { try? await session.close(); throw error }
  }

  @Test @MainActor func socketRejectsMalformedEnvelopesBeforeDispatch() async throws {
    let handled = Locked(false)
    let server = try SocketServer { _, _ in
      handled.modify { $0 = true }
      return SocketReply(ok: true, epoch: "test").encoded()
    }
    defer { server.stop() }
    let path = server.path
    for payload in [
      "not json", "{}",
      #"{"method":"export","documentPath":"/tmp/a.slop","format":"pdf","output":"/tmp/a.pdf"}"#,
    ] {
      let data = try await Task.detached {
        try SocketClient.call(path: path, request: Data(payload.utf8))
      }.value
      let reply = try #require(try JSONSerialization.jsonObject(with: data) as? [String: Any])
      #expect(reply["ok"] as? Bool == false)
      #expect(reply["error"] as? String == "Invalid socket request")
    }
    #expect(!handled.value)
    // Past the request limit, only an attachment upload is parsed at all.
    let oversized = try await Task.detached {
      try SocketClient.call(path: path, request: Data(repeating: 65, count: 1_048_577))
    }.value
    #expect(String(decoding: oversized, as: UTF8.self).contains("Invalid socket request"))
    #expect(!handled.value)
  }
  @Test @MainActor func socketBoundsConcurrentClientsWithoutBlockingMainActor() async throws {
    let replies = Locked<[CheckedContinuation<Data, Never>]>([])
    let server = try SocketServer { _, _ in
      await withCheckedContinuation { continuation in replies.modify { $0.append(continuation) } }
    }
    defer { server.stop() }
    let path = server.path
    let request = Data(#"{"method":"get","documentPath":"/tmp/a.slop"}"#.utf8)
    // The synchronous client must not occupy Swift's cooperative executor.
    func call() async throws -> Data {
      try await withCheckedThrowingContinuation { continuation in
        DispatchQueue.global().async {
          continuation.resume(with: Result { try SocketClient.call(path: path, request: request) })
        }
      }
    }
    let tasks = (0..<16).map { _ in Task { try await call() } }
    for _ in 0..<200 where replies.value.count < 16 { try await Task.sleep(for: .milliseconds(10)) }
    #expect(replies.value.count == 16)
    await #expect(throws: (any Error).self) {
      _ = try await call()
    }
    for reply in replies.value { reply.resume(returning: SocketReply(ok: true, state: [:]).encoded()) }
    for task in tasks { _ = try await task.value }
  }

  @Test @MainActor func schemeRejectsSymlinkReplacedAfterOpen() async throws {
    let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    try FileManager.default.createDirectory(
      at: root.appendingPathComponent("assets"), withIntermediateDirectories: true)
    defer { try? FileManager.default.removeItem(at: root) }
    let asset = root.appendingPathComponent("assets/test.js")
    try Data("safe".utf8).write(to: asset)
    let handler = SchemeHandler(root: root, shell: root)
    try FileManager.default.removeItem(at: asset)
    try FileManager.default.createSymbolicLink(
      at: asset, withDestinationURL: root.appendingPathComponent("initial.json"))
    try Data("secret".utf8).write(to: root.appendingPathComponent("initial.json"))
    let task = SchemeTask(URL(string: "slop://app/assets/test.js")!)
    handler.webView(WKWebView(), start: task)
    await task.completion()
    #expect(task.error != nil)
    #expect(task.data.isEmpty)
  }

  // Decoding and normalization must never turn an authored asset into a private resource.
  @Test @MainActor func schemeRejectsEncodedTraversalAndPreservesAllowedResources() async throws {
    let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: root) }
    for directory in ["assets", "state", "shell/loro"] {
      try FileManager.default.createDirectory(at: root.appendingPathComponent(directory), withIntermediateDirectories: true)
    }
    for path in ["assets/test.js", "shell/loro/test.js", "shell/secret.js", "state/host.lock", "state/document.sqlite"] {
      try Data(path.utf8).write(to: root.appendingPathComponent(path))
    }
    let shell = root.appendingPathComponent("shell")
    let handler = SchemeHandler(root: root, shell: shell)
    let view = WKWebView()
    for path in [
      "assets%2F..%2Fstate%2Fhost.lock", "assets/..%2Fstate/document.sqlite",
      "assets/%2e%2e/state/host.lock", "assets//test.js", "assets/%2e/test.js",
      "__shell__/loro%2F..%2Fsecret.js", "__shell__/loro//test.js",
    ] {
      let task = SchemeTask(URL(string: "slop://app/" + path)!)
      handler.webView(view, start: task)
      await task.completion()
      #expect(task.error != nil, "Accepted unsafe resource: \(path)")
      #expect(task.data.isEmpty)
    }
    for (path, expected) in [("assets/test.js?v=1", "assets/test.js"), ("__shell__/loro/test.js", "shell/loro/test.js")] {
      let task = SchemeTask(URL(string: "slop://app/" + path)!)
      handler.webView(view, start: task)
      await task.completion()
      #expect(task.error == nil)
      #expect(task.data == Data(expected.utf8))
    }
    // WebKit raises on a callback to a task it stopped; a stopped read is dropped.
    let stopped = SchemeTask(URL(string: "slop://app/assets/test.js")!)
    handler.webView(view, start: stopped)
    handler.webView(view, stop: stopped)
    let next = SchemeTask(URL(string: "slop://app/assets/test.js")!)
    handler.webView(view, start: next)
    await next.completion()
    for _ in 0..<10 { await Task.yield() }
    #expect(stopped.calls == 0)

  }

  // Manifest sizing must govern actual bridge requests, not just native window chrome.
  @Test(arguments: [false, true]) @MainActor
  func resizeBridgeHonorsManifest(resizable: Bool) async throws {
    let repository = String(#filePath.components(separatedBy: "/apps/apple/")[0])
    let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString + ".slop")
    try FileManager.default.copyItem(atPath: repository + "/tests/fixtures/checklist/document", toPath: root.path)
    defer { try? FileManager.default.removeItem(at: root) }
    let manifestURL = root.appendingPathComponent("manifest.json")
    var manifest = try #require(JSONSerialization.jsonObject(with: Data(contentsOf: manifestURL)) as? [String: Any])
    manifest["presentation"] = ["width": 480, "height": 480, "resizable": resizable]
    try JSONSerialization.data(withJSONObject: manifest).write(to: manifestURL)
    let session = try await DocumentSession.open(packageURL: root)
    var resized = false
    let events = SessionEvents()
    events.resize = { size in resized = true; return size }
    session.delegate = events
    session.load()
    do {
      try await session.waitUntilReady()
      let resize = """
        try {
          const size = await webkit.messageHandlers.hitslop.postMessage({method:'window.resize',width:600,height:500});
          return size.width === 600 && size.height === 500;
        } catch { return false; }
        """
      let accepted = try await session.webView.callAsyncJavaScript(resize, arguments: [:], in: nil, contentWorld: .page)
      #expect(accepted as? Bool == resizable)
      #expect(resized == resizable)
      // A capture owns the view's size; the page cannot resize the window meanwhile.
      resized = false
      session.capturing = true
      let duringCapture = try await session.webView.callAsyncJavaScript(resize, arguments: [:], in: nil, contentWorld: .page)
      session.capturing = false
      #expect(duringCapture as? Bool == false)
      #expect(!resized)
      try await session.close()
    } catch {
      try? await session.close()
      throw error
    }
  }

  // Authored apps may embed HTTPS frames, so the bridge must refuse every frame but the main one.
  // A same-origin `slop://app` frame shares the page's origin, leaving the main-frame check as
  // the only guard.
  @Test @MainActor func embeddedFrameCannotUseBridge() async throws {
    let repository = String(#filePath.components(separatedBy: "/apps/apple/")[0])
    let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString + ".slop")
    try FileManager.default.copyItem(atPath: repository + "/tests/fixtures/checklist/document", toPath: root.path)
    defer { try? FileManager.default.removeItem(at: root) }
    try Data("<!doctype html><title>frame</title>".utf8).write(
      to: root.appendingPathComponent("assets/frame.html"))
    let manifestURL = root.appendingPathComponent("manifest.json")
    var manifest = try #require(JSONSerialization.jsonObject(with: Data(contentsOf: manifestURL)) as? [String: Any])
    manifest["presentation"] = ["width": 480, "height": 480, "resizable": true]
    try JSONSerialization.data(withJSONObject: manifest).write(to: manifestURL)
    let session = try await DocumentSession.open(packageURL: root)
    var resizes = 0
    let events = SessionEvents()
    events.resize = { size in resizes += 1; return size }
    session.delegate = events
    session.load()
    do {
      try await session.waitUntilReady()
      let request = """
        const request = {method:'window.resize',width:600,height:500};
        const frame = document.createElement('iframe');
        frame.src = 'slop://app/assets/frame.html';
        const loaded = new Promise(resolve => frame.addEventListener('load', resolve, {once: true}));
        document.body.append(frame);
        await Promise.race([loaded, new Promise(resolve => setTimeout(resolve, 5000))]);
        const bridge = frame.contentWindow?.webkit?.messageHandlers?.hitslop;
        let framed = 'unreachable';
        if (bridge) {
          try { framed = (await bridge.postMessage(request)).ok ? 'accepted' : 'rejected'; } catch { framed = 'rejected'; }
        }
        let main = 'rejected';
        try { main = (await webkit.messageHandlers.hitslop.postMessage(request)).ok ? 'accepted' : 'rejected'; } catch {}
        frame.remove();
        return {framed, main};
        """
      let result = try #require(
        try await session.webView.callAsyncJavaScript(request, arguments: [:], in: nil, contentWorld: .page)
          as? [String: String])
      #expect(result["main"] == "accepted")
      #expect(result["framed"] == "rejected")
      #expect(resizes == 1)
      try await session.close()
    } catch {
      try? await session.close()
      throw error
    }
  }

}

@MainActor private final class SchemeTask: NSObject, @preconcurrency WKURLSchemeTask {
  let request: URLRequest
  var data = Data()
  var error: Error?
  var calls = 0
  private var finished = false
  private var waiter: CheckedContinuation<Void, Never>?
  init(_ url: URL) { request = URLRequest(url: url) }
  func didReceive(_ response: URLResponse) { calls += 1 }
  func didReceive(_ data: Data) { calls += 1; self.data.append(data) }
  func didFinish() { calls += 1; end() }
  func didFailWithError(_ error: Error) { calls += 1; self.error = error; end() }
  private func end() {
    finished = true
    waiter?.resume()
    waiter = nil
  }
  /// Waits until the handler finished or failed this task.
  func completion() async {
    if finished { return }
    await withCheckedContinuation { waiter = $0 }
  }
}
