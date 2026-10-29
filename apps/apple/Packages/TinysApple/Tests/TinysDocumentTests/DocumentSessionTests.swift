import AppKit
import Foundation
import Security
import Testing
import TinysCore
import TinysCoreBinding
import TinysTestSupport
import WebKit

@testable import TinysDocument

@Suite(.serialized) struct DocumentSessionTests {
  // Rust tests cover publishing a full owner's live state. This crosses the real page
  // barrier too: unsent text must reach that owner without saving the full source first.
  @Test @MainActor func aFullSharedSessionHandsOffUnsentPageText() async throws {
    let stage = try Fixtures.stage()
    try Fixtures.updateApp(stage) {
      $0["document"] = ["kind": "object", "properties": ["title": ["kind": "text"], "padding": ["kind": "string"]]]
      $0["initial"] = ["title": "Saved title", "padding": ""]
    }
    try Fixtures.writeApp(
      """
      export default { mount(ctx, target) {
        const input = document.createElement('textarea');
        target.append(input);
        const binding = ctx.bind.text(input, ctx.document.fields.title);
        return { unmount() { binding.destroy(); } };
      }};
      """, to: stage)
    let original = try Fixtures.document(stage: stage)
    let directory = try Fixtures.folder()
    defer {
      try? FileManager.default.removeItem(at: original)
      try? FileManager.default.removeItem(at: directory)
    }
    let library = try NativeLibrary.open(root: directory.appendingPathComponent("library").path, account: "test")
    let entry = try library.adopt(source: original.path, title: "Full document")
    let root = URL(fileURLWithPath: entry.path)
    let session = try await DocumentSession.open(url: root)
    session.load()
    try await session.waitUntilReady()
    let destination = directory.appendingPathComponent("Recovered.tiny")
    do {
      var full = false
      for _ in 0..<64 {
        var bytes = Data(count: 1024 * 1024)
        let status = bytes.withUnsafeMutableBytes { SecRandomCopyBytes(kSecRandomDefault, $0.count, $0.baseAddress!) }
        #expect(status == errSecSuccess)
        let batch = try JSONSerialization.data(withJSONObject: [
          "intents": [["type": "set", "path": ["padding"], "value": bytes.base64EncodedString()]]
        ])
        _ = try await session.owner.apply(batch: String(decoding: batch, as: UTF8.self))
        do { try await session.owner.flush() } catch let failure as OwnerFailure where failure.kind == .full {
          full = true
        }
      }
      try #require(full, "real retained history must fill the store")
      do {
        try await session.owner.flush()
        Issue.record("The retained history must still refuse saving before handoff")
      } catch let failure as OwnerFailure { #expect(failure.kind == .full) }
      _ = try await session.webView.callAsyncJavaScript(
        "const input = document.querySelector('textarea'); input.value = 'Unsent recovery text'; input.dispatchEvent(new Event('input', {bubbles:true})); return true",
        arguments: [:], in: nil, contentWorld: .page)
      try await session.handOff(to: destination)
      #expect(!Fixtures.isLocked(root))
      let copy = try DocumentOwner(url: destination)
      let state = try Fixtures.object(await copy.state())
      #expect((state["value"] as? [String: Any])?["title"] as? String == "Unsent recovery text")
      try await copy.close()
    } catch {
      // The unfixed session barrier fails before handoff; release its real full owner
      // directly so the regression run does not leave a writer behind.
      try? await session.owner.handOff(to: directory.appendingPathComponent("cleanup.tiny"), artwork: nil)
      throw error
    }
  }

  @Test @MainActor func environmentIsAvailableBeforeMountAndTracksTheWebViewWidth() async throws {
    let stage = try Fixtures.stage()
    try Fixtures.writeApp(
      """
      export default { mount(ctx) {
        globalThis.environmentAtMount = ctx.environment.current;
        globalThis.attributesAtMount = {...document.documentElement.dataset};
        globalThis.environmentNow = ctx.environment.current;
        const unsubscribe = ctx.environment.subscribe(() => { globalThis.environmentNow = ctx.environment.current; });
        return { unmount: unsubscribe };
      }};
      """, to: stage)
    let root = try Fixtures.document(stage: stage)
    defer { try? FileManager.default.removeItem(at: root) }
    let session = try await DocumentSession.open(url: root)
    session.load()
    do {
      try await session.waitUntilReady()
      let initial = try await session.webView.callAsyncJavaScript(
        "return environmentAtMount.platform === 'macos' && environmentAtMount.device === 'desktop' && attributesAtMount.tinyPlatform === 'macos' && attributesAtMount.tinyDevice === 'desktop' && attributesAtMount.tinyLayout === environmentAtMount.layout",
        arguments: [:], in: nil, contentWorld: .page)
      #expect(initial as? Bool == true)
      for width in [639, 640] {
        session.webView.setFrameSize(NSSize(width: width, height: 700))
        let layout = width < 640 ? "compact" : "regular"
        #expect(
          await eventually(timeout: .seconds(5)) {
            (try? await session.webView.callAsyncJavaScript(
              "return environmentNow.platform === 'macos' && environmentNow.device === 'desktop' && environmentNow.layout === layout && document.documentElement.dataset.tinyLayout === layout",
              arguments: ["layout": layout], in: nil, contentWorld: .page)) as? Bool == true
          })
      }
      try await session.close()
    } catch {
      try? await session.close()
      throw error
    }
  }

  // A handoff used to discard prepareClose's draft reply. Cancellation or a failed
  // destination must leave the same renderer editable and its writer lease intact.
  @Test @MainActor func handoffRetainsDraftsAndOwnershipUntilRecoverySucceeds() async throws {
    let root = try Fixtures.document()
    let directory = try Fixtures.folder()
    defer {
      try? FileManager.default.removeItem(at: root)
      try? FileManager.default.removeItem(at: directory)
    }
    let session = try await DocumentSession.open(url: root)
    session.load()
    try await session.waitUntilReady()
    _ = try await session.webView.callAsyncJavaScript(
      """
      const original = globalThis.__tiny.prepareClose;
      globalThis.__tiny.prepareClose = async () => {
        await original();
        return {unresolvedDrafts:[{path:['title'],text:'Keep this entire draft 🐈'}]};
      };
      return true;
      """, arguments: [:], in: nil, contentWorld: .page)
    let destination = directory.appendingPathComponent("Recovered.tiny")
    do {
      await #expect(throws: (any Error).self) { try await session.handOff(to: destination) }
      #expect(session.acceptsPageWork && Fixtures.isLocked(root))
      #expect(!FileManager.default.fileExists(atPath: destination.path))
      await #expect(throws: CancellationError.self) {
        try await session.handOff(
          to: destination,
          resolveDrafts: { drafts in
            #expect(drafts.map(\.text) == ["Keep this entire draft 🐈"])
            throw CancellationError()
          })
      }
      #expect(session.acceptsPageWork && Fixtures.isLocked(root))
      let occupied = directory.appendingPathComponent("Occupied.tiny")
      try Data("Do not replace".utf8).write(to: occupied)
      await #expect(throws: (any Error).self) {
        try await session.handOff(to: occupied, resolveDrafts: { _ in })
      }
      #expect(try String(contentsOf: occupied, encoding: .utf8) == "Do not replace")
      #expect(session.acceptsPageWork && Fixtures.isLocked(root))
      let editable = try await session.webView.callAsyncJavaScript(
        "return !document.body.inert", arguments: [:], in: nil, contentWorld: .page)
      #expect(editable as? Bool == true)
      try await session.handOff(
        to: destination,
        resolveDrafts: { drafts in
          #expect(drafts.map(\.text) == ["Keep this entire draft 🐈"])
        })
      #expect(!Fixtures.isLocked(root))
      #expect(FileManager.default.fileExists(atPath: destination.path))
    } catch {
      try? await session.close()
      throw error
    }
  }

  @Test @MainActor func shellImportFailureReachesNativeSession() async throws {
    let root = try Fixtures.document()
    defer { try? FileManager.default.removeItem(at: root) }
    let session = try await DocumentSession.open(url: root)
    session.load()
    try await session.waitUntilReady()
    let events = SessionEvents()
    var reported: Error?
    events.failure = { reported = $0 }
    session.delegate = events
    // Execute the real boot module from the page URL: its relative index.js import
    // is absent there. The caught resource failure must reach the native session.
    let boot = try String(contentsOf: DocumentSession.pageShell().appendingPathComponent("boot.js"), encoding: .utf8)
    _ = try await session.webView.callAsyncJavaScript(boot, arguments: [:], in: nil, contentWorld: .page)
    #expect(reported != nil)
    try await session.close()
  }

  // The production shell must open and execute document operations under the page CSP.
  @Test @MainActor func shellStartupAndOperationsRespectCSP() async throws {
    let root = try Fixtures.document()
    defer { try? FileManager.default.removeItem(at: root) }
    let session = try await DocumentSession.open(url: root)
    session.webView.configuration.userContentController.addUserScript(
      WKUserScript(
        source: """
          globalThis.cspViolations = [];
          addEventListener('securitypolicyviolation', event => cspViolations.push(event.violatedDirective));
          """, injectionTime: .atDocumentStart, forMainFrameOnly: true))
    session.load()
    do {
      try await session.waitUntilReady()
      _ = try await session.webView.callAsyncJavaScript(
        "await globalThis.__tiny.flush(); return true", arguments: [:], in: nil, contentWorld: .page)
      let violations =
        try await session.webView.callAsyncJavaScript(
          "return cspViolations", arguments: [:], in: nil, contentWorld: .page) as? [String]
      #expect(violations == [])
      try await session.close()
    } catch {
      try? await session.close()
      throw error
    }
  }

  // Exercise the real WebKit boundary: invalid messages are definite refusals and
  // neither a string nor an object request can bypass Rust's shape checks.
  @Test @MainActor func bridgeRejectsOversizedPayloadsAndUnknownFields() async throws {
    let root = try Fixtures.document()
    defer { try? FileManager.default.removeItem(at: root) }
    let session = try await DocumentSession.open(url: root)
    session.load()
    do {
      try await session.waitUntilReady()
      let refused = try await session.webView.callAsyncJavaScript(
        """
          const requests = [
            {method: 'config', extra: 'unexpected'},
            {method: 'attachments.list'},
            {method: 'window.resize', width: {nested: 1}, height: 300},
            {method: 'attachments.put', bytes: 'A'.repeat(15 * 1024 * 1024)},
          ];
          const post = async request => JSON.parse(await webkit.messageHandlers.tinys.postMessage(request));
          const replies = await Promise.all(requests.map(request => post(JSON.stringify(request))));
          replies.push(await post({method: 'config'}));
          return replies.every(reply => reply.ok === false && reply.code === 'rejected');
        """, arguments: [:], in: nil, contentWorld: .page)
      #expect(refused as? Bool == true)
      try await session.close()
    } catch {
      try? await session.close()
      throw error
    }
  }

  // A theme can change after config is read but before the app finishes mounting.
  @Test @MainActor func themeChangesDuringMountReachTheReadyPage() async throws {
    let stage = try Fixtures.stage()
    try Fixtures.writeApp(
      """
      export default { async mount() {
        globalThis.mountStarted = true;
        await new Promise(resolve => globalThis.finishMount = resolve);
        return {};
      }};
      """, to: stage)
    let root = try Fixtures.document(stage: stage)
    defer { try? FileManager.default.removeItem(at: root) }
    let session = try await DocumentSession.open(url: root)
    session.load()
    do {
      #expect(
        await eventually(timeout: .seconds(5)) {
          (try? await session.webView.callAsyncJavaScript(
            "return globalThis.mountStarted === true", arguments: [:], in: nil, contentWorld: .page)) as? Bool == true
        })
      _ = try await session.owner.apply(batch: ##"{"intents":[{"type":"setTheme","values":{"accent":"#123456"}}]}"##)
      _ = try await session.webView.callAsyncJavaScript(
        "finishMount(); return true", arguments: [:], in: nil, contentWorld: .page)
      try await session.waitUntilReady()
      func accent() async throws -> String? {
        try await session.webView.callAsyncJavaScript(
          "return document.documentElement.style.getPropertyValue('--tiny-accent')", arguments: [:], in: nil,
          contentWorld: .page) as? String
      }
      for _ in 0..<100 {
        if try await accent() == "#123456" { break }
        try await Task.sleep(for: .milliseconds(10))
      }
      #expect(try await accent() == "#123456")
      _ = try await session.owner.apply(batch: #"{"intents":[{"type":"setTheme","values":{"accent":null}}]}"#)
      let expected = try await session.owner.loadTheme().state.effective["accent"]
      for _ in 0..<100 {
        if try await accent() == expected { break }
        try await Task.sleep(for: .milliseconds(10))
      }
      #expect(try await accent() == expected)
      try await session.close()
    } catch {
      try? await session.close()
      throw error
    }
  }

  // The theme panel's changes are edits: applied in the order made, settled on the page
  // before a flush returns, copied into a stable capture source, reported to the
  // window, and saved by close.
  @Test @MainActor func panelThemeChangesSettleBeforeFlushAndSaveOnClose() async throws {
    let stage = try Fixtures.stage()
    try Fixtures.writeApp("export default { mount() { return {}; } };", to: stage)
    let root = try Fixtures.document(stage: stage)
    defer { try? FileManager.default.removeItem(at: root) }
    let session = try await DocumentSession.open(url: root)
    let events = SessionEvents()
    var reported: [String?] = []
    events.theme = { reported.append($0.effective["accent"]) }
    session.delegate = events
    session.load()
    func accent() async throws -> String? {
      try await session.webView.callAsyncJavaScript(
        "return document.documentElement.style.getPropertyValue('--tiny-accent')", arguments: [:], in: nil,
        contentWorld: .page) as? String
    }
    func saved() async throws -> String? {
      let snapshot = try DocumentOwner(url: root, mode: .snapshot)
      defer { Task { try? await snapshot.close() } }
      return try await snapshot.loadTheme().state.effective["accent"]
    }
    do {
      try await session.waitUntilReady()
      #expect(session.acceptsPageWork)
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

      try await session.withCaptureSnapshot { source in
        session.changeTheme(.set(["accent": "#abcabc"]))
        try await session.flush()
        #expect(try await accent() == "#abcabc", "the editor remains live during snapshot rendering")
        let snapshot = try DocumentOwner(url: source, mode: .snapshot)
        let effective = try await snapshot.loadTheme().state.effective
        #expect(effective["accent"] == "#335577")
        try await snapshot.close()
      }

      session.changeTheme(.set(["accent": "#fedcba"]))
      try await session.close()
      #expect(try await saved() == "#fedcba")
    } catch {
      try? await session.close()
      throw error
    }
  }

  // Decoding and normalization must never turn a request into anything but an app asset
  // or a shell file: the document's other contents are not resources.
  @Test @MainActor func schemeRejectsEncodedTraversalAndServesOnlyAssets() async throws {
    let stage = try Fixtures.stage()
    let assetURL = try Fixtures.addAsset(
      stage, bytes: Data("assets/test.js".utf8), ext: "js", mediaType: "text/javascript")
    let root = try Fixtures.document(stage: stage)
    let shell = try Fixtures.folder()
    defer {
      try? FileManager.default.removeItem(at: root)
      try? FileManager.default.removeItem(at: shell)
    }
    try FileManager.default.createDirectory(at: shell.appendingPathComponent("loro"), withIntermediateDirectories: true)
    for path in ["loro/test.js", "secret.js"] {
      try Data(("shell/" + path).utf8).write(to: shell.appendingPathComponent(path))
    }
    let snapshot = try DocumentOwner(url: root, mode: .snapshot)
    let handler = SchemeHandler(assets: snapshot.assets, shell: shell)
    let view = WKWebView()
    for path in [
      "app", "document.tiny", "assets%2F..%2Fapp", "assets/..%2Fapp.js", "assets/%2e%2e/app.js", "assets//test.js",
      "assets/%2e/test.js", "assets/missing.js", "__shell__/loro%2F..%2Fsecret.js", "__shell__/loro//test.js",
      "__shell__/../secret.js",
    ] {
      let task = SchemeTask(URL(string: "tiny://app/" + path)!)
      handler.webView(view, start: task)
      await task.completion()
      #expect(task.error != nil, "Accepted unsafe resource: \(path)")
      #expect(task.data.isEmpty)
    }
    for (path, expected) in [
      (String(assetURL.dropFirst()) + "?v=1", "assets/test.js"), ("__shell__/loro/test.js", "shell/loro/test.js"),
    ] {
      let task = SchemeTask(URL(string: "tiny://app/" + path)!)
      handler.webView(view, start: task)
      await task.completion()
      #expect(task.error == nil)
      #expect(task.data == Data(expected.utf8))
    }
    // WebKit raises on a callback to a task it stopped; a stopped read is dropped.
    let stopped = SchemeTask(URL(string: "tiny://app" + assetURL)!)
    handler.webView(view, start: stopped)
    handler.webView(view, stop: stopped)
    let next = SchemeTask(URL(string: "tiny://app" + assetURL)!)
    handler.webView(view, start: next)
    await next.completion()
    for _ in 0..<10 { await Task.yield() }
    #expect(stopped.calls == 0)
    try await snapshot.close()
  }

  // Manifest sizing must govern actual bridge requests, not just native window chrome.
  @Test(arguments: [false, true]) @MainActor
  func resizeBridgeHonorsManifest(resizable: Bool) async throws {
    let stage = try Fixtures.stage()
    try Fixtures.updateApp(stage) {
      $0["window"] = ["kind": "standard", "width": 480, "height": 480, "resizable": resizable]
    }
    let root = try Fixtures.document(stage: stage)
    defer { try? FileManager.default.removeItem(at: root) }
    let session = try await DocumentSession.open(url: root)
    var resized = false
    let events = SessionEvents()
    events.resize = { size in
      resized = true
      return size
    }
    session.delegate = events
    session.load()
    do {
      try await session.waitUntilReady()
      let resize = """
        try {
          const size = JSON.parse(await webkit.messageHandlers.tinys.postMessage(JSON.stringify({method:'window.resize',width:600,height:500})));
          return size.width === 600 && size.height === 500;
        } catch { return false; }
        """
      let accepted = try await session.webView.callAsyncJavaScript(resize, arguments: [:], in: nil, contentWorld: .page)
      #expect(accepted as? Bool == resizable)
      #expect(resized == resizable)
      // Saved-source acquisition does not change live window eligibility.
      resized = false
      let duringCapture = try await session.withCapture {
        try await session.webView.callAsyncJavaScript(resize, arguments: [:], in: nil, contentWorld: .page)
      }
      #expect(duringCapture as? Bool == resizable)
      #expect(resized == resizable)
      try await session.close()
    } catch {
      try? await session.close()
      throw error
    }
  }

  // Camera and microphone reach only the interactive page of an owned document, in a visible
  // window, and only for what its app declared. WebKit exposes `navigator.mediaDevices` only
  // to an app whose Info.plist has the usage descriptions, which the test host lacks, and real
  // capture needs the person's consent, so this checks the decision against real session state.
  @Test @MainActor func mediaCaptureIsAdmittedOnlyForDeclaredDevicesOnTheInteractivePage() async throws {
    _ = NSApplication.shared
    let stage = try Fixtures.stage()
    try Fixtures.updateApp(stage) { $0["capabilities"] = ["camera"] }
    let root = try Fixtures.document(stage: stage)
    defer { try? FileManager.default.removeItem(at: root) }
    let session = try await DocumentSession.open(url: root)
    #expect(session.file.capabilities == [.camera])
    // WebKit asks this delegate method; a near-miss signature would never be called.
    #expect(
      session.responds(
        to: #selector(WKUIDelegate.webView(_:requestMediaCapturePermissionFor:initiatedByFrame:type:decisionHandler:))))
    let window = NSWindow(
      contentRect: NSRect(x: 0, y: 0, width: 400, height: 400), styleMask: [.borderless], backing: .buffered,
      defer: false)
    // Borderless: AppKit pulls a titled window back on screen when it is ordered front.
    window.moveOffScreen()
    window.contentView = session.webView
    defer {
      window.contentView = nil
      window.orderOut(nil)
    }
    let app = [(protocol: "tiny", host: "app")]
    func admits(
      _ type: WKMediaCaptureType, _ session: DocumentSession, main: Bool = true,
      origins: [(protocol: String, host: String)] = app
    ) -> Bool {
      session.admitsMediaCapture(type, isMainFrame: main, origins: origins, in: session.webView)
    }
    #expect(!admits(.camera, session), "an opening page captures nothing")
    session.load()
    do {
      try await session.waitUntilReady()
      #expect(!admits(.camera, session), "a hidden window captures nothing")
      window.orderFront(nil)
      #expect(admits(.camera, session))
      #expect(!admits(.microphone, session), "the app did not declare the microphone")
      #expect(!admits(.cameraAndMicrophone, session), "a combined request needs both declared")
      #expect(!admits(.camera, session, main: false), "embedded frames never capture")
      #expect(!admits(.camera, session, origins: app + [(protocol: "https", host: "example.com")]))
      #expect(await session.withCapture { admits(.camera, session) }, "saved-source capture leaves live media eligible")
      try await session.close()
    } catch {
      try? await session.close()
      throw error
    }
  }

  // Authored apps may embed HTTPS frames, so the bridge must refuse every frame but the main one.
  // A same-origin `tiny://app` frame shares the page's origin, leaving the main-frame check as
  // the only guard.
  @Test @MainActor func embeddedFrameCannotUseBridge() async throws {
    let stage = try Fixtures.stage()
    try Data("<!doctype html><title>frame</title>".utf8).write(
      to: stage.appendingPathComponent("assets/frame.html"))
    try Fixtures.updateApp(stage) {
      $0["window"] = ["kind": "standard", "width": 480, "height": 480, "resizable": true]
    }
    let root = try Fixtures.document(stage: stage)
    defer { try? FileManager.default.removeItem(at: root) }
    let session = try await DocumentSession.open(url: root)
    var resizes = 0
    let events = SessionEvents()
    events.resize = { size in
      resizes += 1
      return size
    }
    session.delegate = events
    session.load()
    do {
      try await session.waitUntilReady()
      let request = """
        const request = {method:'window.resize',width:600,height:500};
        const frame = document.createElement('iframe');
        frame.src = 'tiny://app/assets/frame.html';
        const loaded = new Promise(resolve => frame.addEventListener('load', resolve, {once: true}));
        document.body.append(frame);
        await Promise.race([loaded, new Promise(resolve => setTimeout(resolve, 5000))]);
        const bridge = frame.contentWindow?.webkit?.messageHandlers?.tinys;
        let framed = 'unreachable';
        if (bridge) {
          try { framed = JSON.parse(await bridge.postMessage(JSON.stringify(request))).ok ? 'accepted' : 'rejected'; } catch { framed = 'rejected'; }
        }
        let main = 'rejected';
        try { main = JSON.parse(await webkit.messageHandlers.tinys.postMessage(JSON.stringify(request))).ok ? 'accepted' : 'rejected'; } catch {}
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
  func didReceive(_ data: Data) {
    calls += 1
    self.data.append(data)
  }
  func didFinish() {
    calls += 1
    end()
  }
  func didFailWithError(_ error: Error) {
    calls += 1
    self.error = error
    end()
  }
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
