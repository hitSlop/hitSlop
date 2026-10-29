import Foundation
import TinysCore
import TinysCoreBinding
@preconcurrency import WebKit

extension WKWebView {
  private static let hostDispatch = "return JSON.stringify(await globalThis.__tiny.dispatch(JSON.parse(request)))"

  /// The host and shell share a generated command vocabulary and one JavaScript entry. Both directions carry opaque JSON strings.
  @MainActor
  func callHost(_ request: HostRequest, completion: @escaping @MainActor (Result<Any, Error>) -> Void) {
    callAsyncJavaScript(
      Self.hostDispatch,
      arguments: ["request": encodeHostRequest(request: request)], in: nil, in: .page, completionHandler: completion)
  }
}
