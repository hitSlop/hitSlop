import Foundation
import HitSlopCore
import WebKit

/// Serves `slop://app`. Files are read off the main thread; WebKit's task callbacks run
/// on the main actor, and a task WebKit stopped is never answered.
@MainActor final class SchemeHandler: NSObject, WKURLSchemeHandler {
  /// Bundled page shell files are immutable, so every WebView shares one in-memory copy.
  nonisolated(unsafe) private static var shellFiles: [String: Data] = [:]
  nonisolated private static let shellFilesLock = NSLock()
  nonisolated private static func shellFile(_ file: URL, within base: URL) throws -> Data {
    if let data = shellFilesLock.withLock({ shellFiles[file.path] }) { return data }
    let data = try SlopFile.read(file, within: base)
    shellFilesLock.withLock { shellFiles[file.path] = data }
    return data
  }
  /// The page shell owns every page; packages supply only assets and data files.
  nonisolated private static let visiblePage = Data(
    "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"><title>hitSlop</title><link rel=\"stylesheet\" href=\"/assets/app.css\"></head><body><script type=\"module\" src=\"/__shell__/boot.js\"></script></body></html>".utf8)
  private static let mimeTypes = [
    "html": "text/html; charset=utf-8", "js": "text/javascript",
    "json": "application/json", "css": "text/css",
    "png": "image/png", "jpg": "image/jpeg", "jpeg": "image/jpeg", "svg": "image/svg+xml",
    "webp": "image/webp", "gif": "image/gif",
    "woff": "font/woff", "woff2": "font/woff2", "ttf": "font/ttf", "mp3": "audio/mpeg",
    "mp4": "video/mp4",
  ]
  private static let contentSecurityPolicy =
    "default-src 'none'; script-src slop:; connect-src slop: https: blob:; media-src slop: https: blob:; frame-src https:; style-src slop: 'unsafe-inline'; img-src slop: data: https: blob:; font-src slop: data:"
  private static let reads = DispatchQueue(label: "hitslop.scheme", qos: .userInitiated, attributes: .concurrent)
  let root: URL
  let shell: URL
  /// Tasks WebKit started and has not stopped. A token tells a stopped task's late read
  /// from a newer task at the same address.
  private var tasks: [ObjectIdentifier: (token: Int, task: any WKURLSchemeTask)] = [:]
  private var nextToken = 0
  init(root: URL, shell: URL) {
    self.root = root
    self.shell = shell
  }
  func webView(_ webView: WKWebView, start task: any WKURLSchemeTask) {
    nextToken += 1
    let id = ObjectIdentifier(task), token = nextToken
    tasks[id] = (token, task)
    let url = task.request.url, root = root, shell = shell
    Self.reads.async { [weak self] in
      let result = Result { try Self.resource(url, root: root, shell: shell) }
      Task { @MainActor in self?.finish(id, token: token, url: url, result) }
    }
  }
  func webView(_ webView: WKWebView, stop task: any WKURLSchemeTask) {
    tasks[ObjectIdentifier(task)] = nil
  }
  private func finish(_ id: ObjectIdentifier, token: Int, url: URL?, _ result: Result<(Data, String), Error>) {
    guard let entry = tasks[id], entry.token == token else { return }
    tasks[id] = nil
    switch result {
    case .success(let (data, fileExtension)):
      let headers = [
        "Content-Type": Self.mimeTypes[fileExtension] ?? "application/octet-stream",
        "Cache-Control": "no-store", "Content-Security-Policy": Self.contentSecurityPolicy,
      ]
      entry.task.didReceive(
        HTTPURLResponse(url: url!, statusCode: 200, httpVersion: "HTTP/1.1", headerFields: headers)!)
      entry.task.didReceive(data)
      entry.task.didFinish()
    case .failure(let error):
      NSLog("hitSlop resource failed: %@ — %@", url?.absoluteString ?? "", error.localizedDescription)
      entry.task.didFailWithError(error)
    }
  }
  /// The bytes and file extension for a request.
  nonisolated private static func resource(_ url: URL?, root: URL, shell: URL) throws -> (Data, String) {
    guard let url, url.host == "app" else { throw failure("Unknown resource origin") }
    let isShell = url.path.hasPrefix("/__shell__/")
    if !isShell && url.path == "/" { return (visiblePage, "html") }
    let base = (isShell ? shell : root).standardizedFileURL
    let relative = isShell ? String(url.path.dropFirst("/__shell__/".count)) : String(url.path.dropFirst())
    // URL.path decodes escaped separators and dots. Reject aliases before
    // normalization so assets/../state can never inherit asset permissions.
    guard SlopPackage.isSafeRelativePath(relative), !relative.hasSuffix("/")
    else { throw failure("Unsafe resource path") }
    let file = base.appendingPathComponent(relative).standardizedFileURL
    guard file.path.hasPrefix(base.path + "/") else { throw failure("Resource outside package") }
    let resource = String(file.path.dropFirst(base.path.count + 1))
    // Serve only authored resources, never state databases or discovery files.
    if !isShell && !(resource == "state.schema.json" || resource == "initial.json" || resource.hasPrefix("assets/")) {
      throw failure("Resource not exposed")
    }
    return (isShell ? try shellFile(file, within: base) : try SlopFile.read(file, within: base), file.pathExtension)
  }
}
