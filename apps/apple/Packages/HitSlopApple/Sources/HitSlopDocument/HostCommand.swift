import Foundation
import HitSlopCore
@preconcurrency import WebKit

extension WKWebView {
  /// The host and shell share a generated command vocabulary and one JavaScript entry.
  @MainActor
  public func callHost(_ request: HostRequest) async throws -> Any? {
    try await callAsyncJavaScript("return await globalThis.__slop.dispatch(request)",
      arguments: ["request": request.json], in: nil, contentWorld: .page)
  }

  @MainActor
  public func callHost(_ request: HostRequest, completion: @escaping @MainActor (Result<Any, Error>) -> Void) {
    callAsyncJavaScript("return await globalThis.__slop.dispatch(request)",
      arguments: ["request": request.json], in: nil, in: .page, completionHandler: completion)
  }
}
