import AppKit
import WebKit

final class Scheme: NSObject, WKURLSchemeHandler {
  let root: URL
  init(_ root: URL) { self.root = root }
  func webView(_ view: WKWebView, start task: WKURLSchemeTask) {
    do {
      guard let url = task.request.url, url.host == "app" else { throw Failure("Invalid resource") }
      let relative = url.path == "/" ? "index.html" : String(url.path.dropFirst())
      guard !relative.split(separator: "/").contains("..") else { throw Failure("Invalid path") }
      let data = try Data(contentsOf: root.appendingPathComponent(relative))
      let mime = relative.hasSuffix(".js") ? "text/javascript" : relative.hasSuffix(".json") ? "application/json" : "text/html"
      task.didReceive(URLResponse(url: url, mimeType: mime, expectedContentLength: data.count, textEncodingName: "utf-8"))
      task.didReceive(data); task.didFinish()
    } catch { task.didFailWithError(error) }
  }
  func webView(_ view: WKWebView, stop task: WKURLSchemeTask) {}
}
final class Handler: NSObject, WKScriptMessageHandlerWithReply {
  let slot: Slot, id: String, store: Store
  var ready = false, failure: String?, saveFailure: Error?
  var timer: DispatchWorkItem?
  var saveMS: [Double] = []
  init(slot: Slot, id: String, store: Store) { self.slot = slot; self.id = id; self.store = store }
  func save() throws {
    timer?.cancel(); timer = nil
    saveMS.append(try measure { try store.save(slot.call("current", id)) })
    saveFailure = nil
  }
  func userContentController(_ controller: WKUserContentController, didReceive message: WKScriptMessage,
    replyHandler: @escaping @MainActor @Sendable (Any?, String?) -> Void) {
    guard let text = message.body as? String, text.utf8.count < 1_048_576,
      let request = try? decode(text) as? [String: Any], let method = request["method"] as? String else {
      replyHandler(nil,"Invalid request"); return
    }
    if method == "ready" { ready = true; replyHandler("{}",nil); return }
    if method == "failed" { failure = request["error"] as? String; replyHandler("{}",nil); return }
    slot.queue.async { [self] in
      do {
        if let saveFailure { throw saveFailure }
        let response: String
        switch method {
        case "current": response = try slot.call("current", id)
        case "apply":
          let payload = try encode(request["operations"]!)
          let engineMS = try measure { _ = try slot.call("apply", id, payload) }
          let t = clockMS(), snapshot = try slot.call("current", id)
          let serializationMS = clockMS() - t
          response = "{\"engineMS\":\(engineMS),\"serializationMS\":\(serializationMS),\"state\":\(snapshot)}"
          if timer == nil {
            let work = DispatchWorkItem { [weak self] in
              guard let self else { return }
              do { try self.save() } catch { self.saveFailure = error }
            }
            timer = work; slot.queue.asyncAfter(deadline: .now() + .milliseconds(200), execute: work)
          }
        case "saveCount": response = String(store.commits)
        case "flush", "close": try save(); response = "null"
        default: throw Failure("Unknown request")
        }
        Task { @MainActor in replyHandler(response,nil) }
      } catch { Task { @MainActor in replyHandler(nil,String(describing:error)) } }
    }
  }
}
@MainActor func web(_ config: [String: Any], source: String, root: URL) async throws -> [String: Any] {
  let topology = Topology(rawValue: config["topology"] as! String)!, count = config["documents"] as! Int
  let dist = URL(fileURLWithPath: config["dist"] as! String), rows = config["rows"] as! Int
  let seed = try String(contentsOfFile: config["seed"] as! String, encoding: .utf8)
  var windows: [NSWindow] = [], views: [WKWebView] = [], handlers: [Handler] = [], schemes: [Scheme] = []
  let engines = Engines(topology, source: source)
  var stores: [Store] = []
  for i in 0..<count { let store = try Store(root.appendingPathComponent("view-\(i).sqlite")); try store.save(seed); stores.append(store) }
  let t = clockMS()
  for i in 0..<count {
    _ = try engines.add("d\(i)",json:seed)
    let h = Handler(slot: engines.slots[i], id: "d\(i)", store: stores[i]), scheme = Scheme(dist)
    let c = WKWebViewConfiguration(); c.websiteDataStore = .nonPersistent()
    c.setURLSchemeHandler(scheme,forURLScheme:"spike")
    c.userContentController.addScriptMessageHandler(h,contentWorld:.page,name:"spike")
    let startup: [String: Any] = ["candidate":"jsc-\(topology.rawValue)","fixture":"checklist","rows":rows]
    c.userContentController.addUserScript(WKUserScript(source:"globalThis.spikeConfig=\(try encode(startup));",injectionTime:.atDocumentStart,forMainFrameOnly:true))
    let view = WKWebView(frame:NSRect(x:0,y:0,width:620,height:700),configuration:c)
    let window = NSWindow(contentRect:view.frame,styleMask:[.titled,.closable,.resizable],backing:.buffered,defer:false)
    window.isReleasedWhenClosed = false; window.contentView = view; window.title = "Document authority — \(topology.rawValue)"
    window.setFrameOrigin(NSPoint(x:80+i*16,y:80+i*16)); window.makeKeyAndOrderFront(nil)
    windows.append(window); views.append(view); handlers.append(h); schemes.append(scheme)
    view.load(URLRequest(url:URL(string:"spike://app/")!))
  }
  defer {
    for i in views.indices { views[i].configuration.userContentController.removeScriptMessageHandler(forName:"spike",contentWorld:.page); windows[i].close() }
    try? engines.close(); stores.forEach { $0.close() }
  }
  NSApp.activate(ignoringOtherApps:true)
  let deadline = clockMS() + 120_000
  while handlers.contains(where:{ !$0.ready }) {
    if let failure = handlers.compactMap(\.failure).first { throw Failure(failure) }
    if clockMS() > deadline { throw Failure("WebKit startup timeout") }
    try await Task.sleep(for:.milliseconds(20))
  }
  let openMS = clockMS() - t
  windows[0].makeKeyAndOrderFront(nil)
  let verified = try await views[0].callAsyncJavaScript("return JSON.stringify(await spike.verify())",arguments:[:],in:nil,contentWorld:.page) as! String
  let result = try await views[0].callAsyncJavaScript("return JSON.stringify(await spike.run(samples))",arguments:["samples":config["samples"] as? Int ?? 20],in:nil,contentWorld:.page) as! String
  var record = try decode(result) as! [String:Any]
  let pids = Set(views.compactMap { ($0.value(forKey:"_webProcessIdentifier") as? NSNumber)?.int32Value })
  let host = memory(), content = pids.map { memory($0)["MiB"] ?? 0 }.reduce(0,+)
  record["hostAndContentMiB"] = (host["MiB"] ?? 0) + content
  record["hostMemory"] = host; record["processesMeasured"] = pids.count+1
  record["openMS"] = openMS; record["verification"] = try decode(verified)
  record["saveMS"] = handlers.flatMap(\.saveMS)
  record["storageCommits"] = stores.map(\.commits).reduce(0,+)
  record["storageBytesWritten"] = stores.map(\.bytes).reduce(0,+)
  for v in views { _ = try await v.callAsyncJavaScript("await spike.close()",arguments:[:],in:nil,contentWorld:.page) }
  return record
}
