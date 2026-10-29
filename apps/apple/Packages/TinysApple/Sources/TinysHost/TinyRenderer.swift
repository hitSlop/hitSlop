import Foundation
import TinysCore
import TinysCoreBinding
import TinysDocument
import TinysWidgets

/// Previews, icons, PNG/PDF exports and widget images, rendered from a saved source by the
/// app's own views in the restricted Rust child. An open document's source is acquired
/// first (`withCaptureSnapshot`); a closed one is read in place, without a writer lease.
@MainActor public enum TinyRenderer {
  static let captureQueue = DispatchQueue(
    label: "com.hitslop.capture-render", qos: .utility, attributes: .concurrent)
  static let nativeRuntime: Result<CaptureRuntime, Error> = Result {
    guard let evaluator = DocumentOwner.evaluatorPath else { throw TinyFailure("Capture renderer is missing") }
    return try CaptureRuntime(evaluatorPath: evaluator)
  }

  public static func previewPNGData(url: URL) async throws -> Data {
    try await capture(url: url, output: .preview)
  }
  static func previewPNGData(session: DocumentSession) async throws -> Data {
    try await session.withCaptureSnapshot { try await capture(url: $0, output: .preview) }
  }
  public static func exportPNGData(session: DocumentSession) async throws -> Data {
    guard session.file.views.export else { throw TinyFailure("This tiny does not support PNG or PDF export") }
    return try await session.withCaptureSnapshot { try await capture(url: $0, output: .export(format: .png)) }
  }
  public static func exportPDFData(session: DocumentSession) async throws -> Data {
    guard session.file.views.export else { throw TinyFailure("This tiny does not support PNG or PDF export") }
    return try await session.withCaptureSnapshot { try await capture(url: $0, output: .export(format: .pdf)) }
  }

  public static func iconPNGData(url: URL) async throws -> Data? {
    let file = try await TinyPreparation.run(on: captureQueue) { try TinyFile(url: url) }
    guard file.views.icon else { return nil }
    return try await image(url: url, output: .icon)
  }
  public static func iconPNGData(session: DocumentSession) async throws -> Data? {
    guard session.file.views.icon else { return nil }
    return try await session.withCaptureSnapshot { try await iconPNGData(url: $0) }
  }

  /// One view of the saved document at `url`.
  static func capture(url: URL, output: CaptureOutput) async throws -> Data {
    guard let bytes = try await image(url: url, output: output)
    else { throw TinyFailure("This tiny has no native capture view; rebuild its template") }
    return bytes
  }

  private static func image(url: URL, output: CaptureOutput) async throws -> Data? {
    switch try await captureNative(url: url, outputs: [output])[0].result {
    case .image(let bytes): return bytes.map { Data($0) }
    case .failed(let failure): throw CoreError.Failure(failure: failure)
    case .widgets: throw TinyFailure("Unexpected capture result")
    }
  }

  /// Every output in one restricted Rust child, from the same saved source.
  static func captureNative(url: URL, outputs: [CaptureOutput]) async throws -> [CaptureResult] {
    let runtime = try nativeRuntime.get()
    let results = try await TinyPreparation.run(on: captureQueue) {
      try runtime.renderSaved(path: url.path, outputs: outputs)
    }
    try Task.checkCancellation()
    return results
  }

  /// Renders artwork from a source already acquired by a copy operation.
  static func artwork(url: URL, telemetry: TinyTelemetry) async -> TinyRenderedArtwork? {
    do {
      return artwork(from: try await captureNative(url: url, outputs: [.preview, .icon]), telemetry: telemetry)
    } catch {
      reportArtworkFailure(error, reason: .preview, telemetry: telemetry)
      return nil
    }
  }

  private static func artwork(from results: [CaptureResult], telemetry: TinyTelemetry) -> TinyRenderedArtwork {
    var preview: Data?
    var icon: Data?
    for capture in results where capture.output == .preview || capture.output == .icon {
      switch capture.result {
      case .image(let bytes):
        if capture.output == .preview { preview = bytes.map { Data($0) } }
        if capture.output == .icon { icon = bytes.map { Data($0) } }
      case .failed(let failure):
        reportArtworkFailure(
          CoreError.Failure(failure: failure), reason: capture.output == .icon ? .icon : .preview,
          telemetry: telemetry)
      case .widgets: break
      }
    }
    return TinyRenderedArtwork(preview: preview, icon: icon)
  }

  /// The artwork a closing window writes into its document: its preview and, when the
  /// app draws one, its icon. A capture that fails is reported and left out.
  public static func artwork(
    session: DocumentSession, telemetry: TinyTelemetry, trace: TinyCloseTrace? = nil, widgetURL: URL? = nil,
    renderArtwork: Bool = true
  ) async
    -> TinyRenderedArtwork
  {
    let capturedAt = Date()
    let widgetIdentity = widgetURL.flatMap { try? WidgetCache.identity($0) }
    if !renderArtwork {
      if let widgetURL, let widgetIdentity {
        await TinyWidgetPublisher.shared.capture(
          renderer: session, url: widgetURL, identity: widgetIdentity, capturedAt: capturedAt)
      }
      return TinyRenderedArtwork(preview: nil, icon: nil)
    }
    do {
      return try await session.withCaptureSnapshot(trace: trace) { source in
        if let widgetURL, let widgetIdentity, let options = session.file.views.widget,
          TinyWidgetPublisher.shared.enabled
        {
          var artwork = TinyRenderedArtwork(preview: nil, icon: nil)
          // The publisher captures its lifetime before any asynchronous rendering.
          // A widget failure must not discard successful preview/icon output.
          await TinyWidgetPublisher.shared.capture(
            url: widgetURL, identity: widgetIdentity, capturedAt: capturedAt, options: options
          ) {
            let results: [CaptureResult]
            do { results = try await captureNative(url: source, outputs: [.preview, .icon, .widgets]) } catch {
              reportArtworkFailure(error, reason: .preview, telemetry: telemetry)
              throw error
            }
            artwork = self.artwork(from: results, telemetry: telemetry)
            return try nativeWidgets(results[2].result)
          }
          return artwork
        }
        return await artwork(url: source, telemetry: telemetry) ?? TinyRenderedArtwork(preview: nil, icon: nil)
      }
    } catch {
      reportArtworkFailure(error, reason: .preview, telemetry: telemetry)
      return TinyRenderedArtwork(preview: nil, icon: nil)
    }
  }

  private static func reportArtworkFailure(
    _ error: Error, reason: TinyFailureContext.Reason, telemetry: TinyTelemetry
  ) {
    guard !TinyFailureContext.isCancellation(error) else { return }
    telemetry.send(.failed(.artwork, .init(reason: reason)))
  }

  /// One saved source, one restricted Rust child, three widget images.
  public static func widgetImages(session: DocumentSession) async throws -> [WidgetImage] {
    try await widgetCapture(session: session).images
  }
  public static func widgetImages(url: URL) async throws -> [WidgetImage] {
    try await widgetCapture(url: url).images
  }
  typealias WidgetCapture = DocumentWidgetCapture
  static func widgetSourceVersion(url: URL) async throws -> String {
    let runtime = try nativeRuntime.get()
    return try await TinyPreparation.run(on: captureQueue) {
      try runtime.savedWidgetVersion(path: url.path)
    }
  }
  static func widgetCapture(session: DocumentSession) async throws -> WidgetCapture {
    try await session.withCaptureSnapshot { try await widgetCapture(url: $0) }
  }
  static func widgetCapture(url: URL) async throws -> WidgetCapture {
    try nativeWidgets(try await captureNative(url: url, outputs: [.widgets])[0].result)
  }
  private static func nativeWidgets(_ result: RenderedOutput) throws -> WidgetCapture {
    try DocumentWidgetCapture(result)
  }
}

extension WidgetImageFamily {
  init(_ family: WidgetFamily) {
    switch family {
    case .small: self = .small
    case .medium: self = .medium
    case .large: self = .large
    }
  }
}
