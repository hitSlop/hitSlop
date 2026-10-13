import Foundation
import HitSlopCore
import HitSlopCoreBinding
import HitSlopDocument

/// Providers consume an already saved source. Acquisition and publication stay with
/// the owner and caller; choosing a provider never retries through another engine.
@MainActor protocol SlopCaptureProviding {
  func render(url: URL, output: SlopRenderer.CaptureOutput) async throws -> Data
  func icon(url: URL) async throws -> Data?
  func widgets(url: URL) async throws -> SlopRenderer.WidgetCapture
}

extension SlopRenderer {
  static func captureProvider(
    _ capability: CaptureCapability, requested: CaptureProvider? = nil
  ) throws -> any SlopCaptureProviding {
    switch try selectCaptureProvider(capability: capability, requested: requested) {
    case .takumi: TakumiCaptureProvider()
    case .webKit: WebKitCaptureProvider()
    }
  }
}

@MainActor struct TakumiCaptureProvider: SlopCaptureProviding {
  private func view(url: URL, kind: ViewCaptureKind, format: CaptureFormat) async throws -> Data? {
    guard let evaluator = DocumentOwner.evaluatorPath else { throw SlopFailure("Capture renderer is missing") }
    let bytes = try await SlopPreparation.run(on: SlopRenderer.captureQueue) {
      try renderSavedView(path: url.path, evaluatorPath: evaluator, kind: kind, format: format)
    }
    try Task.checkCancellation()
    return bytes.map { Data($0) }
  }
  func render(url: URL, output: SlopRenderer.CaptureOutput) async throws -> Data {
    guard
      let bytes = try await view(
        url: url, kind: output == .previewPNG ? .preview : .export,
        format: output == .pdf ? .pdf : .png)
    else { throw SlopFailure("This slop has no native capture view; rebuild its template") }
    return bytes
  }
  func icon(url: URL) async throws -> Data? { try await view(url: url, kind: .icon, format: .png) }
  func widgets(url: URL) async throws -> SlopRenderer.WidgetCapture {
    try await SlopRenderer.captureNativeWidgets(url: url)
  }
}

/// Retained for explicit internal comparison and future host policy. Production
/// selection is Takumi; there is no authored override or automatic fallback.
@MainActor struct WebKitCaptureProvider: SlopCaptureProviding {
  func render(url: URL, output: SlopRenderer.CaptureOutput) async throws -> Data {
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
