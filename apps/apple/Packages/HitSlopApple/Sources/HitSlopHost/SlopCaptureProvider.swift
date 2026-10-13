import Foundation
import HitSlopCore
import HitSlopCoreBinding
import HitSlopDocument

/// Providers consume an already saved source. Acquisition and publication stay with
/// the owner and caller; choosing a provider never retries through another engine.
@MainActor protocol SlopCaptureProviding {
  func render(url: URL, output: CaptureOutput) async throws -> Data
  func icon(url: URL) async throws -> Data?
  func widgets(url: URL) async throws -> SlopRenderer.WidgetCapture
}

extension SlopRenderer {
  static let nativeRuntime: Result<CaptureRuntime, Error> = Result {
    guard let evaluator = DocumentOwner.evaluatorPath else { throw SlopFailure("Capture renderer is missing") }
    return try CaptureRuntime(evaluatorPath: evaluator)
  }

  static func captureNative(url: URL, outputs: [CaptureOutput]) async throws -> [CaptureResult] {
    let runtime = try nativeRuntime.get()
    let results = try await SlopPreparation.run(on: captureQueue) {
      try runtime.renderSaved(path: url.path, outputs: outputs)
    }
    try Task.checkCancellation()
    return results
  }

  static func captureProvider(
    _ output: CaptureOutput, requested: CaptureProvider? = nil
  ) throws -> any SlopCaptureProviding {
    switch try selectCaptureProvider(output: output, requested: requested) {
    case .takumi: TakumiCaptureProvider()
    case .webKit: WebKitCaptureProvider()
    }
  }
}

@MainActor struct TakumiCaptureProvider: SlopCaptureProviding {
  private func view(url: URL, output: CaptureOutput) async throws -> Data? {
    let result = try await SlopRenderer.captureNative(url: url, outputs: [output])[0].result
    switch result {
    case .image(let bytes): return bytes.map { Data($0) }
    case .failed(let failure): throw CoreError.Failure(failure: failure)
    case .widgets: throw SlopFailure("Unexpected capture result")
    }
  }
  func render(url: URL, output: CaptureOutput) async throws -> Data {
    guard let bytes = try await view(url: url, output: output)
    else { throw SlopFailure("This slop has no native capture view; rebuild its template") }
    return bytes
  }
  func icon(url: URL) async throws -> Data? { try await view(url: url, output: .icon) }
  func widgets(url: URL) async throws -> SlopRenderer.WidgetCapture {
    try await SlopRenderer.captureNativeWidgets(url: url)
  }
}

/// Retained for explicit internal comparison and future host policy. Production
/// selection is Takumi; there is no authored override or automatic fallback.
@MainActor struct WebKitCaptureProvider: SlopCaptureProviding {
  func render(url: URL, output: CaptureOutput) async throws -> Data {
    try await SlopRenderer.withRenderSession(url: url) {
      try await SlopRenderer.captureWebView(session: $0, output: output)
    }
  }
  func icon(url: URL) async throws -> Data? {
    try await SlopRenderer.withRenderSession(url: url) { try await SlopRenderer.captureWebIcon(session: $0) }
  }
  func widgets(url: URL) async throws -> SlopRenderer.WidgetCapture {
    throw SlopFailure("WebKit does not support widget batches")
  }
}
