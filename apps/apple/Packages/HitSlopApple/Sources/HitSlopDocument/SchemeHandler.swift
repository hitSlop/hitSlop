import Foundation
import HitSlopCore
import WebKit

final class SchemeHandler: NSObject, WKURLSchemeHandler {
  /// Bundled page shell files are immutable, so every WebView shares one in-memory copy.
  nonisolated(unsafe) private static var shellFiles: [String: Data] = [:]
  private static let shellFilesLock = NSLock()
  private static func shellFile(_ file: URL, within base: URL) throws -> Data {
    if let data = shellFilesLock.withLock({ shellFiles[file.path] }) { return data }
    let data = try SlopFile.read(file, within: base)
    shellFilesLock.withLock { shellFiles[file.path] = data }
    return data
  }
  /// The page shell owns every page; packages supply only assets and data files.
  private static let visiblePage =
    "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"><title>hitSlop</title><link rel=\"stylesheet\" href=\"/assets/app.css\"></head><body><script type=\"module\" src=\"/__shell__/boot.js\"></script></body></html>"
  let root: URL
  let shell: URL
  init(root: URL, shell: URL) {
    self.root = root
    self.shell = shell
  }
  func webView(_ webView: WKWebView, start task: WKURLSchemeTask) {
    do {
      guard let url = task.request.url, url.host == "app" else {
        throw failure("Unknown resource origin")
      }
      let isShell = url.path.hasPrefix("/__shell__/")
      if !isShell && url.path == "/" {
        respond(task, url: url, data: Data(Self.visiblePage.utf8), fileExtension: "html")
        return
      }
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
      if !isShell
        && !(resource == "state.schema.json" || resource == "initial.json"
          || resource.hasPrefix("assets/"))
      {
        throw failure("Resource not exposed")
      }
      let data = isShell ? try Self.shellFile(file, within: base) : try SlopFile.read(file, within: base)
      respond(task, url: url, data: data, fileExtension: file.pathExtension)
    } catch {
      NSLog(
        "hitSlop resource failed: %@ — %@", task.request.url?.absoluteString ?? "",
        error.localizedDescription)
      task.didFailWithError(error)
    }
  }
  private func respond(_ task: WKURLSchemeTask, url: URL, data: Data, fileExtension: String) {
    let mime =
      [
        "html": "text/html; charset=utf-8", "js": "text/javascript",
        "json": "application/json", "css": "text/css",
        "png": "image/png", "jpg": "image/jpeg", "jpeg": "image/jpeg", "svg": "image/svg+xml",
        "webp": "image/webp", "gif": "image/gif",
        "woff": "font/woff", "woff2": "font/woff2", "ttf": "font/ttf", "mp3": "audio/mpeg",
        "mp4": "video/mp4",
      ][fileExtension] ?? "application/octet-stream"
    let headers = [
      "Content-Type": mime, "Cache-Control": "no-store",
      "Content-Security-Policy":
        "default-src 'none'; script-src slop:; connect-src slop: https: blob:; media-src slop: https: blob:; style-src slop: 'unsafe-inline'; img-src slop: data: https: blob:; font-src slop: data:",
    ]
    task.didReceive(
      HTTPURLResponse(url: url, statusCode: 200, httpVersion: "HTTP/1.1", headerFields: headers)!)
    task.didReceive(data)
    task.didFinish()
  }
  func webView(_ webView: WKWebView, stop task: WKURLSchemeTask) {}
}
