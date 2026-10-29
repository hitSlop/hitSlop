import Foundation
import Synchronization
import TinysCore
import TinysCoreBinding
import WebKit

/// Serves `tiny://app`: the page the shell owns, the bundled shell, and the document's app
/// assets from its file, whole or as a byte range. Reads run off the main thread; WebKit's
/// task callbacks run on the main actor, and a task WebKit stopped is never answered.
@MainActor final class SchemeHandler: NSObject, WKURLSchemeHandler {
  /// Bundled page shell files are immutable, so every WebView shares one in-memory copy.
  nonisolated private static let shellFiles = Mutex<[String: Data]>([:])
  nonisolated private static func shellFile(_ file: URL) throws -> Data {
    if let data = shellFiles.withLock({ $0[file.path] }) { return data }
    let data = try Data(contentsOf: file)
    shellFiles.withLock { $0[file.path] = data }
    return data
  }
  /// The page shell owns every page; a document supplies only its app's assets.
  #if os(iOS)
    nonisolated private static let viewport =
      "width=device-width,initial-scale=1,maximum-scale=1,user-scalable=no,viewport-fit=cover"
  #else
    nonisolated private static let viewport = "width=device-width,initial-scale=1,viewport-fit=cover"
  #endif
  nonisolated private static let visiblePage = Data(
    "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"\(viewport)\"><title>tinys</title></head><body><script type=\"module\" src=\"/__shell__/boot.js\"></script></body></html>"
      .utf8)
  /// Scripts only from the document and the shell; WebAssembly may compile (MilkDrop presets
  /// compile their equations at runtime). Inline, `blob:` and `data:` scripts stay refused.
  nonisolated private static let contentSecurityPolicy = AppResourcePolicy.contentSecurityPolicy
  private static let reads = DispatchQueue(label: "tinys.scheme", qos: .userInitiated, attributes: .concurrent)
  let shell: URL
  /// The document's assets, through its owner's long-lived connection.
  private let assets: ResourceReader
  /// Tasks WebKit started and has not stopped. A token tells a stopped task's late read
  /// from a newer task at the same address.
  private var tasks: [ObjectIdentifier: (token: Int, task: any WKURLSchemeTask)] = [:]
  private var nextToken = 0
  init(assets: ResourceReader, shell: URL) {
    self.shell = shell
    self.assets = assets
  }
  func webView(_ webView: WKWebView, start task: any WKURLSchemeTask) {
    nextToken += 1
    let id = ObjectIdentifier(task)
    let token = nextToken
    tasks[id] = (token, task)
    let url = task.request.url
    let shell = shell
    let assets = assets
    let range = task.request.value(forHTTPHeaderField: "Range")
    let head = task.request.httpMethod == "HEAD"
    Self.reads.async { [weak self] in
      let result = Result { try Self.response(url, range: range, head: head, shell: shell, assets: assets) }
      Task { @MainActor in self?.finish(id, token: token, url: url, result) }
    }
  }
  func webView(_ webView: WKWebView, stop task: any WKURLSchemeTask) {
    tasks[ObjectIdentifier(task)] = nil
  }
  private struct Response: Sendable {
    var status = 200
    var body: Data
    var headers: [String: String]
  }
  private func finish(_ id: ObjectIdentifier, token: Int, url: URL?, _ result: Result<Response, Error>) {
    guard let entry = tasks[id], entry.token == token else { return }
    tasks[id] = nil
    switch result {
    case .success(let response):
      entry.task.didReceive(
        HTTPURLResponse(
          url: url!, statusCode: response.status, httpVersion: "HTTP/1.1", headerFields: response.headers)!)
      entry.task.didReceive(response.body)
      entry.task.didFinish()
    case .failure(let error):
      NSLog("tinys resource failed: %@ — %@", url?.absoluteString ?? "", error.localizedDescription)
      entry.task.didFailWithError(error)
    }
  }
  /// The response for a request. WebKit's media loader asks for byte ranges and fails
  /// without a 206 answer; an asset's range is read from the file without loading the rest.
  nonisolated private static func response(_ url: URL?, range: String?, head: Bool, shell: URL, assets: ResourceReader)
    throws
    -> Response
  {
    guard let url, url.host == "app" else { throw TinyFailure("Unknown resource origin") }
    let isShell = url.path.hasPrefix("/__shell__/")
    if !isShell && url.path == "/" {
      return try ranged(nil, type: "text/html; charset=utf-8", length: visiblePage.count, head: head) {
        visiblePage.subdata(in: $0)
      }
    }
    let isAttachment = url.path.hasPrefix("/attachments/")
    let prefix = isShell ? "/__shell__/" : isAttachment ? "/attachments/" : "/assets/"
    guard url.path.hasPrefix(prefix) else { throw TinyFailure("Resource not exposed") }
    // URL.path decodes escaped separators and dots; the core's asset-path rule refuses
    // them before anything else.
    let key = String(url.path.dropFirst(prefix.count))
    guard validAssetPath(path: key) else { throw TinyFailure("Unsafe resource path") }
    let type = contentType(path: key)
    if isShell {
      let base = shell.standardizedFileURL
      let file = base.appendingPathComponent(key).standardizedFileURL
      guard file.path.hasPrefix(base.path + "/") else { throw TinyFailure("Resource outside the shell") }
      return try ranged(range, type: type, length: shellFile(file).count, head: head) {
        try shellFile(file).subdata(in: $0)
      }
    }
    let route: ResourceRoute = isAttachment ? .attachment : .app
    guard let info = try assets.info(route: route, key: key) else { throw TinyFailure("Resource not found") }
    return try ranged(range, type: info.mediaType, length: Int(info.size), route: route, head: head) { bounds in
      guard
        let bytes = try assets.readRange(
          route: route, key: key, offset: UInt64(bounds.lowerBound), length: UInt64(bounds.count))
      else { throw TinyFailure("Resource not found") }
      return bytes
    }
  }
  nonisolated private static func ranged(
    _ range: String?, type: String, length: Int, route: ResourceRoute = .app, head: Bool,
    read: (Range<Int>) throws -> Data
  ) throws -> Response {
    let plan = resourceResponse(info: ResourceInfo(size: UInt64(length), mediaType: type), route: route, range: range)
    var headers = plan.headers
    if route == .app { headers["Content-Security-Policy"] = contentSecurityPolicy }
    let body: Data
    if !head, let span = plan.span {
      body = try read(Int(span.offset)..<Int(span.offset + span.length))
    } else {
      body = Data()
    }
    return Response(status: Int(plan.status), body: body, headers: headers)
  }
}
