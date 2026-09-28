import AppKit
import Darwin
import Foundation
#if RUST_CORE
import NativeOwner
typealias Session = RustOwner
#else
import SpikeCore
#endif
import WebKit

struct Startup: Encodable {
  let candidate: String
  let fixture: String
  let rows: Int
}
struct Request: Decodable {
  let method: String
#if RUST_CORE
  var batch: String?
  var textRequest: String?
  var draft: String?
#else
  var operations: [SpikeCore.Operation]?
#endif
  var generation: String?
  var updates: [String]?
  var bytes: String?
  var schemaKey: String?
  var error: String?
  var url: String?
  var payload: String?
}
func json<T: Encodable>(_ value: T) throws -> String {
  String(decoding: try JSONEncoder().encode(value), as: UTF8.self)
}
final class Handler: NSObject, WKScriptMessageHandlerWithReply {
  let queue = DispatchQueue(label: "placement.document")
  let store: Store
  let session: Session?
  var timer: DispatchWorkItem?
  var sockets: [String: URLSessionWebSocketTask] = [:]
  var ready = false
  var failure: String?
  var saveStatus: ((String?) -> Void)?
  @objc func retrySave() {
    queue.async { [self] in
      do { try session?.flush(); DispatchQueue.main.async { self.saveStatus?(nil) } }
      catch { DispatchQueue.main.async { self.saveStatus?(String(describing:error)) } }
    }
  }
  init(store: Store, session: Session?) {
    self.store = store
    self.session = session
  }
  func userContentController(
    _ controller: WKUserContentController, didReceive message: WKScriptMessage,
    replyHandler: @escaping (Any?, String?) -> Void
  ) {
    guard let text = message.body as? String, text.utf8.count <= 48 * 1024 * 1024 else {
      replyHandler(nil, "Invalid envelope")
      return
    }
    do {
      let request = try JSONDecoder().decode(Request.self, from: Data(text.utf8))
      if request.method == "relay" {
        guard let address = request.url, let url = URL(string: address), url.host == "127.0.0.1",
          url.scheme == "ws", let payload = request.payload, payload.utf8.count <= 256 * 1024
        else { throw SpikeError("Invalid local relay request") }
        let socket: URLSessionWebSocketTask
        if let existing = sockets[address] {
          socket = existing
        } else {
          socket = URLSession.shared.webSocketTask(with: url)
          socket.resume()
          sockets[address] = socket
        }
        Task {
          do {
            try await socket.send(.string(payload))
            let response = try await socket.receive()
            guard case .string(let value) = response else {
              throw SpikeError("Unexpected relay response")
            }
            replyHandler(try json(value), nil)
          } catch { replyHandler(nil, String(describing: error)) }
        }
        return
      }
      if request.method == "ready" {
        ready = true
        replyHandler("{}", nil)
        return
      }
      if request.method == "failed" {
        failure = request.error
        replyHandler("{}", nil)
        return
      }
      queue.async { [self] in
        do {
          let result: String
          switch request.method {
          case "load": result = try json(store.load())
          case "metadata": result = try json(store.metadata())
          case "append":
            guard let gen = request.generation, let values = request.updates else {
              throw SpikeError("Invalid append")
            }
            let bytes = try values.map { v -> Data in
              guard let b = Data(base64Encoded: v) else { throw SpikeError("Invalid bytes") }
              return b
            }
            result = try json(["generation": store.write(generation: gen, updates: bytes)])
          case "checkpoint":
            guard let gen = request.generation, let value = request.bytes,
              let bytes = Data(base64Encoded: value)
            else { throw SpikeError("Invalid checkpoint") }
            result = try json([
              "generation": store.write(
                generation: gen, checkpoint: bytes, schemaKey: request.schemaKey)
            ])
          case "current":
            guard let session else { throw SpikeError("Not native") }
#if RUST_CORE
            result = try session.core.snapshot()
#else
            result = try json(session.replica.read([]))
#endif
          case "apply", "command":
#if RUST_CORE
            guard let session, let batch = request.batch else { throw SpikeError("Invalid batch") }
            let started = Date()
            var publication = try JSONValue.decode(Data((request.method == "command" ? session.command(batch) : session.apply(batch)).utf8)).object
            publication["engineMS"] = .number(Date().timeIntervalSince(started) * 1000)
            result = try json(JSONValue.object(publication))
#else
            guard let session, let operations = request.operations, operations.count <= 1000 else {
              throw SpikeError("Invalid batch")
            }
            result = try json(session.apply(operations))
#endif
            if timer == nil {
              let work = DispatchWorkItem { [weak self] in
                self?.timer = nil
                do { try self?.session?.flush() } catch {
                  DispatchQueue.main.async { self?.saveStatus?(String(describing: error)) }
                }
              }
              timer = work
              queue.asyncAfter(deadline: .now() + 0.2, execute: work)
            }
#if RUST_CORE
          case "text":
            guard let session, let request = request.textRequest else { throw SpikeError("Invalid text") }
            result = try session.text(request)
            if timer == nil {
              let work = DispatchWorkItem { [weak self] in
                self?.timer = nil
                do { try self?.session?.flush() } catch { DispatchQueue.main.async { self?.saveStatus?(String(describing:error)) } }
              }
              timer = work; queue.asyncAfter(deadline: .now() + 0.2, execute: work)
            }
          case "releaseDraft":
            if let draft = request.draft { try session?.core.releaseDraft(draft: draft) }
            result = "{}"
          case "failSave": store.beforeCommit = { throw SpikeError("injected_save_failure") }; result = "{}"
          case "retrySave": store.beforeCommit = nil; try session?.flush(); result = "{}"
#endif
          case "flush":
            timer?.cancel()
            timer = nil
            do { try session?.flush(); DispatchQueue.main.async { self.saveStatus?(nil) } }
            catch { DispatchQueue.main.async { self.saveStatus?(String(describing:error)) }; throw error }
            result = "{}"
          case "saveCount": result = try json(store.commits)
          case "close":
            timer?.cancel()
            try session?.close()
            result = "{}"
          case "snapshot":
            guard let session else { throw SpikeError("Not native") }
#if RUST_CORE
            result = try json(session.core.checkpoint().base64EncodedString())
#else
            result = try json(session.replica.snapshot().base64EncodedString())
#endif
          default: throw SpikeError("Unknown method")
          }
          DispatchQueue.main.async { replyHandler(result, nil) }
        } catch { DispatchQueue.main.async { replyHandler(nil, String(describing: error)) } }
      }
    } catch { replyHandler(nil, String(describing: error)) }
  }
}
final class Scheme: NSObject, WKURLSchemeHandler {
  let root: URL
  var requests: [String] = []
  init(_ root: URL) { self.root = root }
  func webView(_ webView: WKWebView, start task: WKURLSchemeTask) {
    do {
      let relative =
        task.request.url!.path == "/" ? "index.html" : String(task.request.url!.path.dropFirst())
      guard !relative.split(separator: "/").contains("..") else {
        throw SpikeError("Unsafe resource")
      }
      requests.append(relative)
#if RUST_CORE
      guard !relative.contains("wasm"), !relative.contains("loro") else { throw SpikeError("Rust renderer requested an engine") }
#endif
      let data = try Data(contentsOf: root.appendingPathComponent(relative))
      let mime =
        relative.hasSuffix(".js")
        ? "text/javascript"
        : relative.hasSuffix(".wasm")
          ? "application/wasm" : relative.hasSuffix(".json") ? "application/json" : "text/html"
      task.didReceive(
        URLResponse(
          url: task.request.url!, mimeType: mime, expectedContentLength: data.count,
          textEncodingName: "utf-8"))
      task.didReceive(data)
      task.didFinish()
    } catch { task.didFailWithError(error) }
  }
  func webView(_ webView: WKWebView, stop task: WKURLSchemeTask) {}
}
func footprint(_ pid: Int32) -> UInt64? {
  var u = rusage_info_v4()
  let r = withUnsafeMutablePointer(to: &u) {
    $0.withMemoryRebound(to: rusage_info_t?.self, capacity: 1) {
      proc_pid_rusage(pid, RUSAGE_INFO_V4, $0)
    }
  }
  return r == 0 ? u.ri_phys_footprint : nil
}
@MainActor final class Run {
  var windows: [NSWindow] = [], webviews: [WKWebView] = [], handlers: [Handler] = [],
    schemes: [Scheme] = []
  func execute() async throws {
    let args = CommandLine.arguments
    guard args.count >= 8 else {
      throw SpikeError(
        "Usage: engine-placement DIST OUTPUT CANDIDATE ROWS WINDOWS FIXTURE [SAMPLES]")
    }
    let dist = URL(fileURLWithPath: args[1])
    let output = URL(fileURLWithPath: args[2])
    let candidate = args[3]
    let rows = Int(args[4])!
    let count = Int(args[5])!
    let fixture = args[6]
    let samples = Int(args[7])!
    let root = output.deletingPathExtension()
    try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
    let descriptor = try JSONValue.decode(
      Data(contentsOf: dist.appendingPathComponent("\(fixture).schema.json")))
    let schemaKey = String(
      decoding: try JSONSerialization.data(
        withJSONObject: JSONSerialization.jsonObject(
          with: Data(contentsOf: dist.appendingPathComponent("\(fixture).schema.json"))),
        options: [.sortedKeys, .withoutEscapingSlashes]), as: UTF8.self)
    let snapshot = try Data(contentsOf: dist.appendingPathComponent("\(fixture)-\(rows).snapshot"))
    // Database creation and identical seeds are outside the opening measurement.
    var stores: [Store] = []
    for i in 0..<count {
      let store = try Store(root: root.appendingPathComponent(String(i)))
      let m = try store.metadata()
      if m.checkpointBytes == 0 {
        _ = try store.write(generation: m.generation, checkpoint: snapshot, schemaKey: schemaKey)
      }
      stores.append(store)
    }
    let start = Date()
    var nativeEngineMS: [Double] = []
    for i in 0..<count {
      let t = Date()
#if RUST_CORE
      let session: Session? = try RustOwner(store: stores[i])
#else
      let session =
        try candidate.hasPrefix("native")
        ? Session(
          store: stores[i], schema: descriptor["root"], indexed: candidate != "native-naive") : nil
#endif
      nativeEngineMS.append(Date().timeIntervalSince(t) * 1000)
      let handler = Handler(store: stores[i], session: session)
      let scheme = Scheme(dist)
      let config = WKWebViewConfiguration()
      config.websiteDataStore = .nonPersistent()
      config.setURLSchemeHandler(scheme, forURLScheme: "spike")
      config.userContentController.addScriptMessageHandler(
        handler, contentWorld: .page, name: "spike")
      let script =
        "globalThis.spikeConfig="
        + (try json(Startup(candidate: candidate, fixture: fixture, rows: rows))) + ";"
      config.userContentController.addUserScript(
        WKUserScript(source: script, injectionTime: .atDocumentStart, forMainFrameOnly: true))
      let view = WKWebView(
        frame: NSRect(x: 0, y: 0, width: 620, height: 700), configuration: config)
      let window = NSWindow(
        contentRect: view.frame, styleMask: [.titled, .closable, .resizable], backing: .buffered,
        defer: false)
      window.isReleasedWhenClosed = false
      window.contentView = view
      window.title = "Engine placement — \(candidate)"
      let saveAccessory = NSTitlebarAccessoryViewController()
      let retry = NSButton(title:"Retry Save", target:handler, action:#selector(Handler.retrySave))
      retry.frame = NSRect(x:0,y:0,width:100,height:26)
      saveAccessory.view = retry; saveAccessory.layoutAttribute = .right
      saveAccessory.isHidden = true; window.addTitlebarAccessoryViewController(saveAccessory)
      handler.saveStatus = { [weak window, weak saveAccessory] error in
        saveAccessory?.isHidden = error == nil
        window?.title = error.map { "Save failed: " + $0 } ?? "Engine placement — \(candidate)"
      }
      window.setFrameOrigin(NSPoint(x: 80 + i * 16, y: 80 + i * 16))
      window.makeKeyAndOrderFront(nil)
      windows.append(window)
      webviews.append(view)
      handlers.append(handler)
      schemes.append(scheme)
      view.load(URLRequest(url: URL(string: "spike://app/")!))
    }
    NSApp.activate(ignoringOtherApps: true)
    let deadline = Date().addingTimeInterval(180)
    while handlers.contains(where: { !$0.ready }) {
      if let error = handlers.compactMap(\.failure).first { throw SpikeError(error) }
      guard Date() < deadline else { throw SpikeError("Startup timed out") }
      try await Task.sleep(for: .milliseconds(20))
    }
    let openMS = Date().timeIntervalSince(start) * 1000
    windows[0].makeKeyAndOrderFront(nil)
    let verification =
      try await webviews[0].callAsyncJavaScript(
        "return JSON.stringify(await spike.verify())", arguments: [:], in: nil, contentWorld: .page)
      as! String
    var measurement = ""
#if RUST_CORE
    if args.count > 8, args[8] == "integration" {
      var nativeChecks = try JSONValue.decode(Data((try await webviews[0].callAsyncJavaScript("return JSON.stringify(await spike.verifyNative())", arguments: [:], in:nil, contentWorld:.page) as! String).utf8)).object
      // Real AppKit events target WebKit's focused native responder.
      _ = try await webviews[0].callAsyncJavaScript("const e=document.querySelector('textarea');e.focus();e.setSelectionRange(3,3);", arguments:[:], in:nil, contentWorld:.page)
      for type in [NSEvent.EventType.keyDown, .keyUp] {
        let event = NSEvent.keyEvent(with:type, location:.zero, modifierFlags:[], timestamp:ProcessInfo.processInfo.systemUptime, windowNumber:windows[0].windowNumber, context:nil, characters:"K", charactersIgnoringModifiers:"K", isARepeat:false, keyCode:40)!
        NSApp.sendEvent(event)
      }
      let keyboard = try await webviews[0].callAsyncJavaScript("await new Promise(r=>requestAnimationFrame(r));await spike.adapter.drain();return spike.adapter.current().title", arguments:[:], in:nil, contentWorld:.page) as! String
      guard keyboard == "abcK" else { throw SpikeError("Native keyboard failed: \(keyboard)") }
      nativeChecks["appKitKeyboard"] = .bool(true)
      guard let history = windows[0].firstResponder?.undoManager, history.canUndo else { throw SpikeError("No native undo history") }
      history.undo()
      let undone = try await webviews[0].callAsyncJavaScript("await new Promise(r=>requestAnimationFrame(r));await spike.adapter.drain();return spike.adapter.current().title", arguments:[:], in:nil, contentWorld:.page) as! String
      guard undone == "abc" else { throw SpikeError("Native undo failed: \(undone)") }
      history.redo()
      let redone = try await webviews[0].callAsyncJavaScript("await new Promise(r=>requestAnimationFrame(r));await spike.adapter.drain();return spike.adapter.current().title", arguments:[:], in:nil, contentWorld:.page) as! String
      guard redone == "abcK" else { throw SpikeError("Native redo failed: \(redone)") }
      nativeChecks["appKitUndoRedo"] = .bool(true)
      for forced in [false, true] {
        if forced {
          let accepted = try await webviews[0].callAsyncJavaScript("spike.adapter.delay(500);const e=document.querySelector('textarea');e.value='abcKZ';e.setSelectionRange(5,5);e.dispatchEvent(new InputEvent('input',{bubbles:true}));return (await spike.call('current')).value.title", arguments:[:], in:nil, contentWorld:.page) as! String
          guard accepted == "abcKZ" else {throw SpikeError("Forced remount was not after native acceptance")}
        } else {
          _ = try await webviews[0].callAsyncJavaScript("await spike.adapter.drain();await spike.adapter.flush()", arguments:[:], in:nil, contentWorld:.page)
        }
        let configuration = webviews[0].configuration
        weak var oldRenderer = webviews[0]
        windows[0].makeFirstResponder(nil)
        history.removeAllActions()
        webviews[0].stopLoading();windows[0].contentView=nil
        handlers[0].ready=false
        webviews[0] = WKWebView(frame:NSRect(x:0,y:0,width:620,height:700), configuration:configuration)
        let handler = handlers[0]
        try await withCheckedThrowingContinuation { (continuation:CheckedContinuation<Void,Error>) in
          handler.queue.async {
            do {try handler.session?.core.detachRenderer();continuation.resume()} catch {continuation.resume(throwing:error)}
          }
        }
        windows[0].contentView=webviews[0]
        webviews[0].load(URLRequest(url:URL(string:"spike://app/")!))
        let remountDeadline=Date().addingTimeInterval(30)
        while !handlers[0].ready || oldRenderer != nil {
          if let error=handlers[0].failure {throw SpikeError(error)}
          guard Date()<remountDeadline else {throw SpikeError("Renderer remount timed out: ready=\(handlers[0].ready), oldAlive=\(oldRenderer != nil), forced=\(forced)")}
          try await Task.sleep(for:.milliseconds(20))
        }
        let remounted = try await webviews[0].callAsyncJavaScript("return spike.adapter.current().title", arguments:[:], in:nil, contentWorld:.page) as! String
        guard remounted == (forced ? "abcKZ" : "abcK") else {throw SpikeError("Remount lost accepted work")}
      }
      nativeChecks["rendererDestroyedAndRemounted"] = .bool(true)
      nativeChecks["forcedRemountPreservesAcceptedEditBeforeReply"] = .bool(true)
      measurement = try json(JSONValue.object(nativeChecks))
    }
#endif
    if measurement.isEmpty {
    if args.count > 8 {
      measurement =
        try await webviews[0].callAsyncJavaScript(
          "return JSON.stringify(await spike.transport(url))", arguments: ["url": args[8]], in: nil,
          contentWorld: .page) as! String
    } else {
      measurement =
        try await webviews[0].callAsyncJavaScript(
          "return JSON.stringify(await spike.run(samples))", arguments: ["samples": samples],
          in: nil, contentWorld: .page) as! String
    }
    }
    let pids = Set(
      webviews.compactMap { ($0.value(forKey: "_webProcessIdentifier") as? NSNumber)?.int32Value })
    let memory = ([getpid()] + pids).compactMap(footprint).reduce(0, +)
    var record = try JSONValue.decode(Data(measurement.utf8)).object
    record["resourceRequests"] = .array(schemes.flatMap { $0.requests }.map(JSONValue.string))
    record["candidate"] = .string(candidate)
    record["fixture"] = .string(fixture)
    record["windows"] = .number(Double(count))
    record["openMS"] = .number(openMS)
    record["nativeEngineMS"] = .array(nativeEngineMS.map(JSONValue.number))
    record["hostAndContentMiB"] = .number(Double(memory) / 1_048_576)
    // Attribution only: the gate still uses the combined total above.
    record["hostMiB"] = .number(Double(footprint(getpid()) ?? 0) / 1_048_576)
    record["contentMiB"] = .number(Double(pids.compactMap(footprint).reduce(0, +)) / 1_048_576)
    record["verification"] = try JSONValue.decode(Data(verification.utf8))
    record["storageBytesWritten"] = .number(Double(stores.reduce(0) { $0 + $1.writtenBytes }))
    record["storageCommits"] = .number(Double(stores.reduce(0) { $0 + $1.commits }))
    record["processesMeasured"] = .number(Double(pids.count + 1))
    for (i, view) in webviews.enumerated() {
      _ = try await view.callAsyncJavaScript(
        "await spike.close()", arguments: [:], in: nil, contentWorld: .page)
      view.configuration.userContentController.removeScriptMessageHandler(
        forName: "spike", contentWorld: .page)
      windows[i].close()
      stores[i].close()
    }
    try JSONValue.object(record).encoded().write(to: output)
    print("RESULT \(output.path)")
  }
}
#if !RUST_CORE
if CommandLine.arguments.count == 4, CommandLine.arguments[1] == "probe" {
  do {
    try probe(
      URL(fileURLWithPath: CommandLine.arguments[2]), URL(fileURLWithPath: CommandLine.arguments[3])
    )
    exit(0)
  } catch {
    fputs("PROBE FAILED: \(error)\n", stderr)
    exit(1)
  }
}
#endif
let app = NSApplication.shared
app.setActivationPolicy(.accessory)
Task { @MainActor in
  do {
    let run = Run()
    try await run.execute()
    exit(0)
  } catch {
    fputs("SPIKE FAILED: \(error)\n", stderr)
    exit(1)
  }
}
app.run()
