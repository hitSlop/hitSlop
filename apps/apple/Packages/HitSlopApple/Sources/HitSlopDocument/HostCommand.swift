import Foundation
import HitSlopCore
import HitSlopCoreBinding
@preconcurrency import WebKit

extension WKWebView {
  /// The host and shell share a generated command vocabulary and one JavaScript entry. Both directions carry opaque JSON strings.
  @MainActor
  public func callHost(_ request: HostRequest) async throws -> Any? {
    try await callAsyncJavaScript(
      "return JSON.stringify(await globalThis.__slop.dispatch(JSON.parse(request)))",
      arguments: ["request": encodeHostRequest(request: request)], in: nil, contentWorld: .page)
  }

  @MainActor
  public func callHost(_ request: HostRequest, completion: @escaping @MainActor (Result<Any, Error>) -> Void) {
    callAsyncJavaScript(
      "return JSON.stringify(await globalThis.__slop.dispatch(JSON.parse(request)))",
      arguments: ["request": encodeHostRequest(request: request)], in: nil, in: .page, completionHandler: completion)
  }
}
