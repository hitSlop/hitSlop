import Foundation
import HitSlopCore
import HitSlopCoreBinding
import WebKit

/// Serves `slop://app`: the page the shell owns, the bundled shell, and the document's app
/// assets from its file, whole or as a byte range. Reads run off the main thread; WebKit's
/// task callbacks run on the main actor, and a task WebKit stopped is never answered.
@MainActor final class SchemeHandler: NSObject, WKURLSchemeHandler {
  /// Bundled page shell files are immutable, so every WebView shares one in-memory copy.
  nonisolated(unsafe) private static var shellFiles: [String: Data] = [:]
  nonisolated private static let shellFilesLock = NSLock()
  nonisolated private static func shellFile(_ file: URL) throws -> Data {
    if let data = shellFilesLock.withLock({ shellFiles[file.path] }) { return data }
    let data = try Data(contentsOf: file)
    shellFilesLock.withLock { shellFiles[file.path] = data }
    return data
  }
  /// The page shell owns every page; a document supplies only its app's assets.
  nonisolated private static let visiblePage = Data(
    "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"><title>hitSlop</title><link rel=\"stylesheet\" href=\"/assets/app.css\"></head><body><script type=\"module\" src=\"/__shell__/boot.js\"></script></body></html>".utf8)
  /// Scripts only from the document and the shell; WebAssembly may compile (MilkDrop presets
  /// compile their equations at runtime). Inline, `blob:` and `data:` scripts stay refused.
  nonisolated private static let contentSecurityPolicy =
    "default-src 'none'; script-src slop: 'wasm-unsafe-eval'; connect-src slop: https: blob:; media-src slop: https: blob:; frame-src https:; style-src slop: 'unsafe-inline'; img-src slop: data: https: blob:; font-src slop: data:"
  private static let reads = DispatchQueue(label: "hitslop.scheme", qos: .userInitiated, attributes: .concurrent)
  let shell: URL
  /// The document's assets, through its owner's long-lived connection.
  private let assets: AssetReader
  /// Tasks WebKit started and has not stopped. A token tells a stopped task's late read
  /// from a newer task at the same address.
  private var tasks: [ObjectIdentifier: (token: Int, task: any WKURLSchemeTask)] = [:]
  private var nextToken = 0
  init(assets: AssetReader, shell: URL) {
    self.shell = shell
    self.assets = assets
  }
  func webView(_ webView: WKWebView, start task: any WKURLSchemeTask) {
    nextToken += 1
    let id = ObjectIdentifier(task), token = nextToken
    tasks[id] = (token, task)
    let url = task.request.url, shell = shell, assets = assets
    let range = task.request.value(forHTTPHeaderField: "Range")
    Self.reads.async { [weak self] in
      let result = Result { try Self.response(url, range: range, shell: shell, assets: assets) }
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
        HTTPURLResponse(url: url!, statusCode: response.status, httpVersion: "HTTP/1.1", headerFields: response.headers)!)
      entry.task.didReceive(response.body)
      entry.task.didFinish()
    case .failure(let error):
      NSLog("hitSlop resource failed: %@ — %@", url?.absoluteString ?? "", error.localizedDescription)
      entry.task.didFailWithError(error)
    }
  }
  /// The response for a request. WebKit's media loader asks for byte ranges and fails
  /// without a 206 answer; an asset's range is read from the file without loading the rest.
  nonisolated private static func response(_ url: URL?, range: String?, shell: URL, assets: AssetReader) throws -> Response {
    guard let url, url.host == "app" else { throw SlopFailure("Unknown resource origin") }
    let isShell = url.path.hasPrefix("/__shell__/")
    if !isShell && url.path == "/" { return whole(visiblePage, type: "text/html; charset=utf-8") }
    let prefix = isShell ? "/__shell__/" : "/assets/"
    guard url.path.hasPrefix(prefix) else { throw SlopFailure("Resource not exposed") }
    // URL.path decodes escaped separators and dots; the core's asset-path rule refuses
    // them before anything else.
    let key = String(url.path.dropFirst(prefix.count))
    guard validAssetPath(path: key) else { throw SlopFailure("Unsafe resource path") }
    let type = contentType(path: key)
    if isShell {
      let base = shell.standardizedFileURL
      let file = base.appendingPathComponent(key).standardizedFileURL
      guard file.path.hasPrefix(base.path + "/") else { throw SlopFailure("Resource outside the shell") }
      return try ranged(range, type: type, length: shellFile(file).count) { try shellFile(file).subdata(in: $0) }
    }
    guard let size = try assets.size(key: key) else { throw SlopFailure("Resource not found") }
    return try ranged(range, type: type, length: Int(size)) { bounds in
      guard let bytes = try assets.readRange(key: key, offset: UInt64(bounds.lowerBound), length: UInt64(bounds.count))
      else { throw SlopFailure("Resource not found") }
      return bytes
    }
  }
  nonisolated private static func headers(_ type: String, length: Int) -> [String: String] {
    ["Content-Type": type, "Cache-Control": "no-store", "Content-Security-Policy": contentSecurityPolicy,
     "Accept-Ranges": "bytes", "Content-Length": String(length)]
  }
  nonisolated private static func whole(_ data: Data, type: String) -> Response {
    Response(body: data, headers: headers(type, length: data.count))
  }
  nonisolated private static func ranged(_ range: String?, type: String, length: Int, read: (Range<Int>) throws -> Data) throws -> Response {
    switch range.map({ ByteRange($0, length: length) }) ?? .whole {
    case .whole:
      return whole(try read(0..<length), type: type)
    case .part(let bounds):
      let body = try read(bounds)
      var response = Response(status: 206, body: body, headers: headers(type, length: body.count))
      response.headers["Content-Range"] = "bytes \(bounds.lowerBound)-\(bounds.upperBound - 1)/\(length)"
      return response
    case .unsatisfiable:
      var response = Response(status: 416, body: Data(), headers: headers(type, length: 0))
      response.headers["Content-Range"] = "bytes */\(length)"
      return response
    }
  }
}

/// What one `Range: bytes=…` header asks for: `first-last`, `first-` or `-suffix`. A header
/// this does not parse, or one naming several ranges, gets the whole resource, which HTTP
/// allows; a range that names nothing that exists is unsatisfiable (416).
enum ByteRange: Equatable {
  case whole, part(Range<Int>), unsatisfiable

  init(_ header: String, length: Int) {
    let spec = header.trimmingCharacters(in: .whitespaces)
    guard spec.hasPrefix("bytes="), !spec.contains(",") else { self = .whole; return }
    let parts = spec.dropFirst(6).split(separator: "-", maxSplits: 1, omittingEmptySubsequences: false)
      .map { $0.trimmingCharacters(in: .whitespaces) }
    guard parts.count == 2 else { self = .whole; return }
    switch (Int(parts[0]), Int(parts[1])) {
    case let (first?, last) where first >= 0 && (parts[1].isEmpty || last != nil):
      if first >= length || (last.map { $0 < first } ?? false) { self = .unsatisfiable; return }
      // Clamped before the inclusive end becomes exclusive, so no endpoint overflows.
      self = .part(first..<(last.map { min($0, length - 1) + 1 } ?? length))
    case let (nil, suffix?) where parts[0].isEmpty && suffix >= 0:
      self = suffix == 0 || length == 0 ? .unsatisfiable : .part(max(0, length - suffix)..<length)
    default:
      self = .whole
    }
  }
}
