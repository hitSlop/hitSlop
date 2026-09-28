import Foundation
import HitSlopCore
import WebKit

final class SchemeHandler: NSObject, WKURLSchemeHandler {
  /// Bundled runtime files are immutable, so every WebView shares one in-memory copy.
  nonisolated(unsafe) private static var runtimeFiles: [String: Data] = [:]
  private static let runtimeFilesLock = NSLock()
  private static func runtimeFile(_ file: URL, within base: URL) throws -> Data {
    if let data = runtimeFilesLock.withLock({ runtimeFiles[file.path] }) { return data }
    let data = try SlopFile.read(file, within: base)
    runtimeFilesLock.withLock { runtimeFiles[file.path] = data }
    return data
  }
  /// Prewarm sessions have no package root and serve only the runtime plus this page.
  private static let prewarmResources = [
    "/": "<html><head><script type=\"module\" src=\"/__prewarm__.js\"></script></head><body></body></html>",
    "/__prewarm__.js": """
      let outcome = "ready";
      try { await (await import("/__runtime__/index.js")).initialize(); }
      catch (error) { outcome = String(error); }
      webkit.messageHandlers.prewarm.postMessage(outcome);
      """,
  ]
  /// The runtime owns every page; packages supply only assets and data files.
  private static let visiblePage =
    "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"><title>hitSlop</title><link rel=\"stylesheet\" href=\"/assets/app.css\"></head><body><script type=\"module\" src=\"/__runtime__/boot.js\"></script></body></html>"
  private static let headlessPage =
    "<html><head><script type=\"module\" src=\"/__runtime__/headless.js\"></script></head><body></body></html>"
  let headless: Bool
  let root: URL?
  let runtime: URL
  init(root: URL?, runtime: URL, headless: Bool = false) {
    self.headless = headless
    self.root = root
    self.runtime = runtime
  }
  func webView(_ webView: WKWebView, start task: WKURLSchemeTask) {
    do {
      guard let url = task.request.url, url.host == "app" else {
        throw failure("Unknown resource origin")
      }
      let isRuntime = url.path.hasPrefix("/__runtime__/")
      guard isRuntime || root != nil else {
        guard let page = Self.prewarmResources[url.path] else { throw failure("Resource not exposed") }
        respond(task, url: url, data: Data(page.utf8), fileExtension: url.path == "/" ? "html" : "js")
        return
      }
      if !isRuntime && url.path == "/" {
        respond(task, url: url, data: Data((headless ? Self.headlessPage : Self.visiblePage).utf8), fileExtension: "html")
        return
      }
      let base = (isRuntime ? runtime : root!).standardizedFileURL
      let relative = isRuntime ? String(url.path.dropFirst("/__runtime__/".count)) : String(url.path.dropFirst())
      // URL.path decodes escaped separators and dots. Reject aliases before
      // normalization so assets/../state can never inherit asset permissions.
      guard SlopPackage.isSafeRelativePath(relative), !relative.hasSuffix("/")
      else { throw failure("Unsafe resource path") }
      let file = base.appendingPathComponent(relative).standardizedFileURL
      guard file.path.hasPrefix(base.path + "/") else { throw failure("Resource outside package") }
      let resource = String(file.path.dropFirst(base.path.count + 1))
      // Serve only authored resources, never state databases or discovery files.
      if !isRuntime
        && !(resource == "state.schema.json" || resource == "initial.json"
          || resource.hasPrefix("assets/"))
      {
        throw failure("Resource not exposed")
      }
      guard !headless || isRuntime || ["state.schema.json", "initial.json", "assets/theme.json"].contains(resource)
      else { throw failure("App resources unavailable to headless engine") }
      let data = isRuntime ? try Self.runtimeFile(file, within: base) : try SlopFile.read(file, within: base)
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
        "html": "text/html; charset=utf-8", "js": "text/javascript", "wasm": "application/wasm",
        "json": "application/json", "css": "text/css",
        "png": "image/png", "jpg": "image/jpeg", "jpeg": "image/jpeg", "svg": "image/svg+xml",
        "webp": "image/webp", "gif": "image/gif",
        "woff": "font/woff", "woff2": "font/woff2", "ttf": "font/ttf", "mp3": "audio/mpeg",
        "mp4": "video/mp4",
      ][fileExtension] ?? "application/octet-stream"
    let headers = [
      "Content-Type": mime, "Cache-Control": "no-store",
      "Content-Security-Policy":
        "default-src 'none'; script-src slop: 'wasm-unsafe-eval'; connect-src slop: https: blob:; media-src slop: https: blob:; style-src slop: 'unsafe-inline'; img-src slop: data: https: blob:; font-src slop: data:",
    ]
    task.didReceive(
      HTTPURLResponse(url: url, statusCode: 200, httpVersion: "HTTP/1.1", headerFields: headers)!)
    task.didReceive(data)
    task.didFinish()
  }
  func webView(_ webView: WKWebView, stop task: WKURLSchemeTask) {}
}
